mod action;
mod app;
mod config;
mod event;
mod python_bridge;
mod tui;
mod ui;

use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::KeyCode;

use crate::action::Action;
use crate::app::{App, Screen};
use crate::event::{AppEvent, EventReader};
use crate::python_bridge::{find_python, PythonCommand, PythonProcess};

/// Process pasted text (from bracketed paste or accumulated chars): add as file paths.
fn process_paste(app: &mut App, text: &str) {
    for raw in text.lines() {
        let path = raw
            .trim()
            .replace("\\ ", " ") // unescape spaces
            .trim_matches('\'') // remove shell quotes
            .trim_matches('"')
            .to_string();
        if !path.is_empty() {
            app.apply(Action::AddFile(path));
        }
    }
    // 粘贴后将焦点设到文件字段
    if app.screen == Screen::Configure {
        app.focused_field = 0;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let python_path = find_python()?;
    let mut python = PythonProcess::spawn(&python_path).await?;

    let mut terminal = tui::init()?;
    let mut app = App::new();
    let mut events = EventReader::new(Duration::from_millis(100));
    let mut config_loaded = false;

    // CLI 直接翻译模式：pt file1.pdf file2.pdf
    let cli_files: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    let cli_mode = !cli_files.is_empty();
    let mut auto_translate = cli_mode;

    // Paste buffer for terminals without bracketed paste support.
    let mut paste_buf = String::new();
    let mut paste_last_char: Option<Instant> = None;
    const PASTE_TIMEOUT: Duration = Duration::from_millis(50);

    loop {
        // Flush paste buffer if timed out
        if let Some(last) = paste_last_char {
            if last.elapsed() >= PASTE_TIMEOUT && !paste_buf.is_empty() {
                process_paste(&mut app, &paste_buf);
                paste_buf.clear();
                paste_last_char = None;
            }
        }

        // CLI 模式：配置加载完成后自动触发翻译
        if auto_translate && app.screen == Screen::Configure {
            auto_translate = false;
            if let Err(e) = config::save_config(&app) {
                app.push_log(format!("自动保存配置失败: {e}"));
            }
            if let Some(err) = app.prepare_translation() {
                app.apply(Action::ShowError(err));
            } else {
                let settings = app.build_settings_json();
                let files = app.selected_files.clone();
                if let Err(e) = python
                    .send(PythonCommand::Translate { settings, files })
                    .await
                {
                    app.screen = Screen::Configure;
                    app.apply(Action::ShowError(
                        format!("无法发送翻译命令: {e}\nPython 进程可能已崩溃"),
                    ));
                }
            }
        }

        // 在绘制前调整 scroll_offset 保持焦点可见
        if app.screen == Screen::Configure {
            // header(3) + footer(1) + left panel border(2) = 6
            let visible = (terminal.size()?.height as usize).saturating_sub(6);
            app.ensure_visible(visible);
        }

        terminal.draw(|frame| app.draw(frame))?;

        // Collect Python events (non-blocking)
        while let Ok(event) = tokio::time::timeout(
            Duration::from_millis(1),
            python.recv_event(),
        )
        .await
        {
            if let Some(event) = event {
                let is_config_schema =
                    matches!(&event, python_bridge::PythonEvent::ConfigSchema { .. });
                app.apply(Action::PythonEvent(event));

                if is_config_schema && !config_loaded {
                    config_loaded = true;
                    match config::load_config(&mut app) {
                        Ok(true) => app.push_log("已加载配置文件".to_string()),
                        Ok(false) => app.push_log("未找到配置文件，使用默认值".to_string()),
                        Err(e) => app.push_log(format!("加载配置失败: {e}")),
                    }
                    // CLI 模式：注入命令行文件
                    if cli_mode {
                        for f in &cli_files {
                            app.apply(Action::AddFile(f.clone()));
                        }
                    }
                }
            } else {
                app.apply(Action::ShowError("Python 进程已退出".to_string()));
                break;
            }
        }

        // Collect stderr logs
        while let Some(line) = python.try_recv_stderr() {
            app.push_log(line);
        }

        // Handle terminal events
        if let Some(event) = tokio::time::timeout(Duration::from_millis(16), events.next())
            .await
            .ok()
            .flatten()
        {
            let action = match event {
                AppEvent::Key(key) => {
                    // Fallback paste detection
                    if app.screen == Screen::Configure
                        && !app.editing
                        && app.dropdown_open.is_none()
                        && app.popup.is_none()
                    {
                        if let KeyCode::Char(c) = key.code {
                            if !key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                paste_buf.push(c);
                                paste_last_char = Some(Instant::now());
                                Action::None
                            } else {
                                app.handle_key(key)
                            }
                        } else {
                            // Non-char key: flush any pending paste buffer first
                            if !paste_buf.is_empty() {
                                process_paste(&mut app, &paste_buf);
                                paste_buf.clear();
                                paste_last_char = None;
                            }
                            app.handle_key(key)
                        }
                    } else {
                        app.handle_key(key)
                    }
                }
                AppEvent::Mouse(_) => Action::None,
                AppEvent::Paste(text) => {
                    process_paste(&mut app, &text);
                    Action::None
                }
                AppEvent::Resize(w, h) => Action::Resize(w, h),
                AppEvent::Tick => Action::Tick,
            };

            // Actions that need Python interaction
            match &action {
                Action::StartTranslation => {
                    // 翻译前自动保存配置
                    if let Err(e) = config::save_config(&app) {
                        app.push_log(format!("自动保存配置失败: {e}"));
                    }
                    if let Some(err) = app.prepare_translation() {
                        app.apply(Action::ShowError(err));
                    } else {
                        let settings = app.build_settings_json();
                        let files = app.selected_files.clone();
                        if let Err(e) = python
                            .send(PythonCommand::Translate { settings, files })
                            .await
                        {
                            app.screen = Screen::Configure;
                            app.apply(Action::ShowError(
                                format!("无法发送翻译命令: {e}\nPython 进程可能已崩溃"),
                            ));
                        }
                    }
                }
                Action::CancelTranslation => {
                    let _ = python.send(PythonCommand::Cancel).await;
                    app.screen = Screen::Configure;
                    app.push_log("翻译已取消".to_string());
                }
                Action::SaveConfig => {
                    match config::save_config(&app) {
                        Ok(()) => app.push_log("配置已保存".to_string()),
                        Err(e) => app.apply(Action::ShowError(format!("保存配置失败: {e}"))),
                    }
                }
                _ => {}
            }

            // File input: on ExitField, add the typed file path
            if matches!(&action, Action::ExitField)
                && app.focused_field == 0
                && !app.file_input.is_empty()
            {
                let path = app.file_input.trim().to_string();
                app.file_input.clear();
                app.apply(Action::AddFile(path));
            }

            app.apply(action);
        }

        // CLI 模式：翻译完成后自动退出
        if cli_mode && app.screen == Screen::Results {
            break;
        }

        if app.should_quit {
            break;
        }
    }

    // 退出前自动保存配置
    if let Err(e) = config::save_config(&app) {
        eprintln!("自动保存配置失败: {e}");
    }

    python.shutdown().await;
    tui::restore()?;

    // CLI 模式：打印翻译结果摘要到终端
    if cli_mode {
        for result in &app.results {
            if let Some(p) = result.get("mono_pdf_path").and_then(|v| v.as_str()) {
                if !p.is_empty() && p != "null" {
                    println!("单语PDF: {p}");
                }
            }
            if let Some(p) = result.get("dual_pdf_path").and_then(|v| v.as_str()) {
                if !p.is_empty() && p != "null" {
                    println!("双语PDF: {p}");
                }
            }
        }
        println!("翻译完成 ({})", app.elapsed_display());
    }

    Ok(())
}
