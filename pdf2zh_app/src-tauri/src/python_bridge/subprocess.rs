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

        // Try entry point first (pdf2zh_next --json-stream)
        let child_result = Command::new("pdf2zh_next")
            .args(["--json-stream"])
            .current_dir(&work_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn();

        let mut child = match child_result {
            Ok(child) => child,
            Err(_) => {
                // Fallback: python -m pdf2zh_next --json-stream
                let python = find_python()?;
                Command::new(&python)
                    .args(["-m", "pdf2zh_next", "--json-stream"])
                    .current_dir(&work_dir)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                    .with_context(|| {
                        format!(
                            "Failed to spawn pdf2zh_next. Tried 'pdf2zh_next' and '{python} -m pdf2zh_next'"
                        )
                    })?
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

/// Find a suitable Python executable.
pub fn find_python() -> Result<String> {
    for candidate in ["python3", "python"] {
        if which::which(candidate).is_ok() {
            return Ok(candidate.to_string());
        }
    }
    anyhow::bail!(
        "Python 3 not found. Please install Python 3.10+ and ensure it's in your PATH."
    )
}
