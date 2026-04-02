use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, Mutex};

use crate::python_bridge::protocol::{EngineSchema, PythonCommand, PythonEvent};
use crate::python_bridge::PythonProcess;

/// Command sender — lock-free, cloneable.
pub struct CmdSender(pub Arc<Mutex<Option<mpsc::Sender<PythonCommand>>>>);

/// Schema data cached after the backend sends config_schema.
#[derive(Default)]
pub struct SchemaCache(pub Arc<Mutex<Option<ConfigSchemaData>>>);

#[derive(Debug, Clone, Serialize)]
pub struct ConfigSchemaData {
    pub engines: Vec<EngineSchema>,
    pub languages: indexmap::IndexMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackendStatus {
    pub running: bool,
    pub version: Option<String>,
}

/// Start the Python backend subprocess and begin forwarding events to the frontend.
#[tauri::command]
pub async fn start_backend(
    app: AppHandle,
    cmd_sender: State<'_, CmdSender>,
    schema_cache: State<'_, SchemaCache>,
) -> Result<BackendStatus, String> {
    // Spawn new process
    let mut proc = PythonProcess::spawn().await.map_err(|e| e.to_string())?;

    // Store the command sender (lock-free path for translate/cancel)
    let sender = proc.command_sender();
    *cmd_sender.0.lock().await = Some(sender);

    // Wait for ready + config_schema events
    let mut version = None;
    loop {
        match proc.recv_event().await {
            Some(PythonEvent::Ready { version: v, .. }) => {
                version = Some(v);
            }
            Some(PythonEvent::ConfigSchema {
                engines, languages, ..
            }) => {
                let data = ConfigSchemaData { engines, languages };
                let _ = app.emit("config-schema", &data);
                *schema_cache.0.lock().await = Some(data);
                break;
            }
            Some(other) => {
                let _ = app.emit("backend-event", &other);
            }
            None => {
                return Err("Python backend exited before sending ready event".into());
            }
        }
    }

    // Spawn event forwarding — takes ownership of proc (no mutex needed)
    let app_handle = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = proc.recv_event() => {
                    match event {
                        Some(evt) => { let _ = app_handle.emit("backend-event", &evt); }
                        None => break,
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                    for line in proc.drain_stderr() {
                        let _ = app_handle.emit("backend-log", &line);
                    }
                }
            }
        }
    });

    Ok(BackendStatus {
        running: true,
        version,
    })
}

/// Get the cached config schema.
#[tauri::command]
pub async fn get_config_schema(
    schema_cache: State<'_, SchemaCache>,
) -> Result<Option<ConfigSchemaData>, String> {
    Ok(schema_cache.0.lock().await.clone())
}

/// Helper to send a command via the stored sender.
async fn send_cmd(
    cmd_sender: &State<'_, CmdSender>,
    cmd: PythonCommand,
) -> Result<(), String> {
    let guard = cmd_sender.0.lock().await;
    let sender = guard.as_ref().ok_or("Backend not running")?;
    sender
        .send(cmd)
        .await
        .map_err(|_| "Failed to send command to backend".to_string())
}

/// Send a translate command.
#[tauri::command]
pub async fn translate(
    cmd_sender: State<'_, CmdSender>,
    settings: serde_json::Value,
    files: Vec<String>,
) -> Result<(), String> {
    send_cmd(&cmd_sender, PythonCommand::Translate { settings, files }).await
}

/// Send a cancel command.
#[tauri::command]
pub async fn cancel_translate(cmd_sender: State<'_, CmdSender>) -> Result<(), String> {
    send_cmd(&cmd_sender, PythonCommand::Cancel).await
}

/// Validate settings.
#[tauri::command]
pub async fn validate_settings(
    cmd_sender: State<'_, CmdSender>,
    settings: serde_json::Value,
) -> Result<(), String> {
    send_cmd(&cmd_sender, PythonCommand::Validate { settings }).await
}

/// Reveal a file in Finder.
#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .args(["-R", &path])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get backend status.
#[tauri::command]
pub async fn get_backend_status(cmd_sender: State<'_, CmdSender>) -> Result<BackendStatus, String> {
    let guard = cmd_sender.0.lock().await;
    Ok(BackendStatus {
        running: guard.is_some(),
        version: None,
    })
}

/// Load saved config.
#[tauri::command]
pub fn cmd_load_config() -> Result<crate::config::AppConfig, String> {
    crate::config::load_config().map_err(|e| e.to_string())
}

/// Save config.
#[tauri::command]
pub fn cmd_save_config(config: crate::config::AppConfig) -> Result<(), String> {
    crate::config::save_config(&config).map_err(|e| e.to_string())
}
