#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod python_bridge;

use std::sync::Arc;

use commands::{CmdSender, SchemaCache};
use python_bridge::protocol::PythonCommand;
use tauri::Manager;
use tokio::sync::Mutex;

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .manage(CmdSender(Arc::new(Mutex::new(None))))
        .manage(SchemaCache::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_backend,
            commands::get_config_schema,
            commands::translate,
            commands::cancel_translate,
            commands::validate_settings,
            commands::reveal_in_finder,
            commands::get_backend_status,
            commands::cmd_load_config,
            commands::cmd_save_config,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let handle: tauri::AppHandle = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<CmdSender>();
                    let guard = state.0.lock().await;
                    if let Some(sender) = guard.as_ref() {
                        let _ = sender.send(PythonCommand::Shutdown).await;
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
