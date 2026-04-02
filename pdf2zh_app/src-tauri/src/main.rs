#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod python_bridge;

use std::sync::Arc;

use commands::{CmdSender, SchemaCache};
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
