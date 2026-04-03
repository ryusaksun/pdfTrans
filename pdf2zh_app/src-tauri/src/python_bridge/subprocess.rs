use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::protocol::{PythonCommand, PythonEvent};

/// Manages the Python pdf2zh_next subprocess.
pub struct PythonProcess {
    child: Child,
    stdin_tx: mpsc::Sender<PythonCommand>,
    event_rx: mpsc::Receiver<PythonEvent>,
    stderr_rx: mpsc::Receiver<String>,
}

impl PythonProcess {
    /// Spawn the pdf2zh_next subprocess with --json-stream flag.
    /// In a bundled .app, looks for the embedded Python first.
    pub async fn spawn() -> Result<Self> {
        let work_dir = dirs::download_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"));

        let extra_path = build_path();

        // Strategy 1: Use embedded Python inside .app bundle
        let mut child_opt: Option<Child> = None;
        if let Some(embedded) = find_embedded_python() {
            // embedded = .../Resources/python/bin/python3
            // PYTHONHOME = .../Resources/python  (2 levels up)
            let python_home = embedded.parent().unwrap().parent().unwrap();
            if let Ok(c) = Command::new(&embedded)
                .args(["-m", "pdf2zh_next", "--json-stream"])
                .current_dir(&work_dir)
                .env("PYTHONHOME", python_home)
                .env("PATH", &extra_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
            {
                child_opt = Some(c);
            }
        }

        // Strategy 2: Find pdf2zh_next in PATH / common locations
        if child_opt.is_none() {
            let candidates = find_pdf2zh_candidates();
            for cmd in &candidates {
                if let Ok(c) = Command::new(cmd)
                    .args(["--json-stream"])
                    .current_dir(&work_dir)
                    .env("PATH", &extra_path)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                {
                    child_opt = Some(c);
                    break;
                }
            }
        }

        // Strategy 3: python -m pdf2zh_next
        let mut child = match child_opt {
            Some(c) => c,
            None => {
                let python = find_python()?;
                Command::new(&python)
                    .args(["-m", "pdf2zh_next", "--json-stream"])
                    .current_dir(&work_dir)
                    .env("PATH", &extra_path)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                    .with_context(|| "Failed to spawn pdf2zh_next backend")?
            }
        };

        let stdin = child.stdin.take().expect("stdin should be piped");
        let stdout = child.stdout.take().expect("stdout should be piped");
        let stderr = child.stderr.take().expect("stderr should be piped");

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<PythonCommand>(32);
        let (event_tx, event_rx) = mpsc::channel::<PythonEvent>(256);
        let (stderr_tx, stderr_rx) = mpsc::channel::<String>(256);

        // Stdin writer task
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(cmd) = stdin_rx.recv().await {
                let line = match serde_json::to_string(&cmd) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to serialize command: {e}");
                        continue;
                    }
                };
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        // Stdout reader task
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<PythonEvent>(&line) {
                    Ok(event) => {
                        if event_tx.send(event).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        // Ignore unparseable lines
                    }
                }
            }
        });

        // Stderr reader task
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stderr_tx.send(line).await;
            }
        });

        Ok(Self {
            child,
            stdin_tx,
            event_rx,
            stderr_rx,
        })
    }

    /// Get a cloneable command sender (lock-free).
    pub fn command_sender(&self) -> mpsc::Sender<PythonCommand> {
        self.stdin_tx.clone()
    }

    /// Send a command to the Python process.
    pub async fn send(&self, cmd: PythonCommand) -> Result<()> {
        self.stdin_tx
            .send(cmd)
            .await
            .map_err(|_| anyhow::anyhow!("Python process stdin channel closed"))
    }

    /// Receive the next event from the Python process.
    pub async fn recv_event(&mut self) -> Option<PythonEvent> {
        self.event_rx.recv().await
    }

    /// Try to receive a stderr line without blocking.
    pub fn try_recv_stderr(&mut self) -> Option<String> {
        self.stderr_rx.try_recv().ok()
    }

    /// Drain all available stderr lines.
    pub fn drain_stderr(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while let Some(line) = self.try_recv_stderr() {
            lines.push(line);
        }
        lines
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Send shutdown command and wait for process to exit.
    pub async fn shutdown(&mut self) {
        let _ = self.send(PythonCommand::Shutdown).await;
        tokio::select! {
            _ = self.child.wait() => {}
            _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
                let _ = self.child.kill().await;
            }
        }
    }
}

/// Find the embedded Python inside the .app bundle (Contents/Resources/python/bin/python3).
fn find_embedded_python() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    // exe is at .app/Contents/MacOS/binary
    let resources = exe.parent()?.parent()?.join("Resources").join("python").join("bin").join("python3");
    if resources.exists() {
        Some(resources)
    } else {
        None
    }
}

/// Build an extended PATH that includes common user binary locations.
fn build_path() -> String {
    let home = dirs::home_dir().unwrap_or_default();
    let mut paths: Vec<String> = vec![
        home.join(".local/bin").to_string_lossy().to_string(),
        home.join(".cargo/bin").to_string_lossy().to_string(),
        "/usr/local/bin".to_string(),
        "/opt/homebrew/bin".to_string(),
        "/opt/homebrew/sbin".to_string(),
    ];
    // Include existing PATH
    if let Ok(existing) = std::env::var("PATH") {
        paths.push(existing);
    }
    paths.join(":")
}

/// Find candidate paths for the pdf2zh_next command.
fn find_pdf2zh_candidates() -> Vec<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut candidates = vec![
        home.join(".local/bin/pdf2zh_next").to_string_lossy().to_string(),
        home.join(".local/bin/pdf2zh").to_string_lossy().to_string(),
        "pdf2zh_next".to_string(),
        "pdf2zh".to_string(),
    ];
    // Check uv tool installs
    let uv_bin = home.join(".local/share/uv/tools/pdf2zh-next/bin/pdf2zh_next");
    if uv_bin.exists() {
        candidates.insert(0, uv_bin.to_string_lossy().to_string());
    }
    candidates
}

/// Find a suitable Python executable.
pub fn find_python() -> Result<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let candidates = [
        home.join(".local/bin/python3").to_string_lossy().to_string(),
        "/opt/homebrew/bin/python3".to_string(),
        "/usr/local/bin/python3".to_string(),
        "python3".to_string(),
        "python".to_string(),
    ];
    for candidate in &candidates {
        if which::which(candidate).is_ok() || std::path::Path::new(candidate).exists() {
            return Ok(candidate.to_string());
        }
    }
    anyhow::bail!(
        "Python 3 not found. Please install Python 3.10+ and ensure it's in your PATH."
    )
}
