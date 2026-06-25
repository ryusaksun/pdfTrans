use std::collections::VecDeque;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use indexmap::IndexMap;
use ratatui::prelude::*;

use crate::action::Action;
use crate::python_bridge::protocol::EngineSchema;
use crate::python_bridge::PythonEvent;
use crate::ui;

/// Current application screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Loading,
    Configure,
    Translating,
    Results,
}

/// Which panel is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Left,
    Right,
}

/// Popup types.
#[derive(Debug, Clone)]
pub enum Popup {
    Error(String),
    Help,
}

/// Identifies which dropdown is currently open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownTarget {
    LangIn,
    LangOut,
    Engine,
}

/// Main application state.
pub struct App {
    pub screen: Screen,
    pub active_panel: ActivePanel,
    pub should_quit: bool,
    pub popup: Option<Popup>,

    // Python connection
    pub python_version: String,

    // Engine/language metadata (from config_schema)
    pub engine_schemas: Vec<EngineSchema>,
    pub language_map: IndexMap<String, String>,

    // Fields (unified single-page layout):
    // 0=file_input, 1=lang_in, 2=lang_out, 3=pages,
    // 4=engine, 5=qps, 6..6+N=engine_params,
    // 6+N=no_dual, 7+N=no_mono, 8+N=dual_translate_first, 9+N=skip_clean,
    // 10+N=custom_prompt, 11+N=min_text_length,
    // 12+N..21+N=advanced translation/pdf controls
    pub selected_files: Vec<String>,
    pub file_input: String,
    pub lang_in_idx: usize,
    pub lang_out_idx: usize,
    pub pages_input: String,
    pub no_dual: bool,
    pub no_mono: bool,
    pub dual_translate_first: bool,

    pub engine_idx: usize,
    pub engine_params: IndexMap<String, String>,
    pub qps: u32,
    pub qps_input: String,

    pub custom_prompt: String,
    pub min_text_length: String,
    pub skip_clean: bool,
    pub pool_max_workers_input: String,
    pub term_qps_input: String,
    pub term_pool_max_workers_input: String,
    pub output_dir: String,
    pub glossary_files: String,
    pub max_pages_per_part_input: String,
    pub skip_scanned_detection: bool,
    pub save_auto_extracted_glossary: bool,
    pub only_include_translated_page: bool,
    pub disable_auto_extract_glossary: bool,

    // Progress state
    pub progress: ProgressState,

    // Results
    pub results: Vec<serde_json::Value>,
    pub token_usage: Option<serde_json::Value>,

    // Log buffer
    pub log_lines: VecDeque<String>,

    // Focus tracking
    pub focused_field: usize,
    pub editing: bool,
    pub scroll_offset: usize,

    // Dropdown state
    pub dropdown_open: Option<DropdownTarget>,
    pub dropdown_cursor: usize,
}

/// Translation progress tracking.
#[derive(Debug, Clone)]
pub struct ProgressState {
    pub stage: String,
    pub stage_progress: f64,
    pub overall_progress: f64,
    pub stage_current: u32,
    pub stage_total: u32,
    pub part_index: u32,
    pub total_parts: u32,
    pub current_file_idx: usize,
    pub total_files: usize,
    pub current_file: String,
    pub start_time: Option<Instant>,
    pub finished_count: usize,
}

impl Default for ProgressState {
    fn default() -> Self {
        Self {
            stage: String::new(),
            stage_progress: 0.0,
            overall_progress: 0.0,
            stage_current: 0,
            stage_total: 0,
            part_index: 0,
            total_parts: 0,
            current_file_idx: 0,
            total_files: 0,
            current_file: String::new(),
            start_time: None,
            finished_count: 0,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Loading,
            active_panel: ActivePanel::Left,
            should_quit: false,
            popup: None,
            python_version: String::new(),
            engine_schemas: Vec::new(),
            language_map: IndexMap::new(),
            selected_files: Vec::new(),
            file_input: String::new(),
            lang_in_idx: 0,
            lang_out_idx: 1,
            pages_input: String::new(),
            no_dual: false,
            no_mono: true,
            dual_translate_first: false,
            engine_idx: 0,
            engine_params: IndexMap::new(),
            qps: 4,
            qps_input: "4".to_string(),
            custom_prompt: String::new(),
            min_text_length: String::new(),
            skip_clean: false,
            pool_max_workers_input: String::new(),
            term_qps_input: String::new(),
            term_pool_max_workers_input: String::new(),
            output_dir: String::new(),
            glossary_files: String::new(),
            max_pages_per_part_input: String::new(),
            skip_scanned_detection: false,
            save_auto_extracted_glossary: false,
            only_include_translated_page: false,
            disable_auto_extract_glossary: false,
            progress: ProgressState::default(),
            results: Vec::new(),
            token_usage: None,
            log_lines: VecDeque::with_capacity(200),
            focused_field: 0,
            editing: false,
            scroll_offset: 0,
            dropdown_open: None,
            dropdown_cursor: 0,
        }
    }

    /// Number of engine-specific parameter fields.
    pub fn engine_param_count(&self) -> usize {
        self.engine_schemas
            .get(self.engine_idx)
            .map(|s| s.fields.len())
            .unwrap_or(0)
    }

    /// Total number of focusable fields (unified single page).
    pub fn max_fields(&self) -> usize {
        // file_input(1) + lang_in(1) + lang_out(1) + pages(1) +
        // engine(1) + qps(1) + engine_params(N) +
        // no_dual(1) + no_mono(1) + dual_translate_first(1) + skip_clean(1) +
        // custom_prompt(1) + min_text_length(1) + advanced(10) = 22 + N
        22 + self.engine_param_count()
    }

    /// Is the currently focused field a dropdown?
    fn is_dropdown_field(&self) -> bool {
        matches!(self.focused_field, 1 | 2 | 4)
    }

    /// Is the currently focused field a checkbox?
    fn is_checkbox_field(&self) -> bool {
        let n = self.engine_param_count();
        matches!(
            self.focused_field,
            f if f == 6 + n
                || f == 7 + n
                || f == 8 + n
                || f == 9 + n
                || f == 18 + n
                || f == 19 + n
                || f == 20 + n
                || f == 21 + n
        )
    }

    /// Get the dropdown target for the current focused field.
    fn dropdown_target(&self) -> Option<DropdownTarget> {
        match self.focused_field {
            1 => Some(DropdownTarget::LangIn),
            2 => Some(DropdownTarget::LangOut),
            4 => Some(DropdownTarget::Engine),
            _ => None,
        }
    }

    /// Get the number of items for the given dropdown.
    fn dropdown_len(&self, target: DropdownTarget) -> usize {
        match target {
            DropdownTarget::LangIn | DropdownTarget::LangOut => self.language_map.len(),
            DropdownTarget::Engine => self.engine_schemas.len(),
        }
    }

    /// Ensure focused_field is visible by adjusting scroll_offset.
    /// `visible_rows` is the number of rows that can be displayed in the left panel.
    pub fn ensure_visible(&mut self, visible_rows: usize) {
        // 计算 focused_field 对应的行号（包含分节标题和间距）
        let row = self.field_to_row(self.focused_field);
        if row < self.scroll_offset {
            self.scroll_offset = row;
        } else if row >= self.scroll_offset + visible_rows {
            self.scroll_offset = row - visible_rows + 1;
        }
    }

    /// Map a field index to its approximate row in the layout.
    fn field_to_row(&self, field: usize) -> usize {
        let n = self.engine_param_count();
        // 行布局:
        // 0: ── 文件 ── (section header)
        // 1: file_input (field 0)
        // 2: (selected files, dynamic but at least 1 line)
        // 3: ── 语言 ──
        // 4: lang_in (field 1)
        // 5: lang_out (field 2)
        // 6: pages (field 3)
        // 7: ── 翻译引擎 ──
        // 8: engine (field 4)
        // 9: qps (field 5)
        // 10..10+N: engine params (fields 6..6+N)
        // 10+N: ── PDF 选项 ──
        // 11+N: no_dual (field 6+N)
        // 12+N: no_mono (field 7+N)
        // 13+N: dual_translate_first (field 8+N)
        // 14+N: skip_clean (field 9+N)
        // 15+N: ── 高级 ──
        // 16+N: custom_prompt (field 10+N)
        // 17+N: min_text_length (field 11+N)
        // 18+N..27+N: advanced controls (fields 12+N..21+N)

        let file_list_extra = self.selected_files.len().max(1);
        match field {
            0 => 1,                                              // file_input
            1 => 2 + file_list_extra + 1,                        // lang_in (after section header)
            2 => 2 + file_list_extra + 2,                        // lang_out
            3 => 2 + file_list_extra + 3,                        // pages
            4 => 2 + file_list_extra + 5,                        // engine (after section header)
            5 => 2 + file_list_extra + 6,                        // qps
            f if f < 6 + n => 2 + file_list_extra + 7 + (f - 6), // engine params
            f if f == 6 + n => 2 + file_list_extra + 7 + n + 1,  // no_dual (after section)
            f if f == 7 + n => 2 + file_list_extra + 7 + n + 2,  // no_mono
            f if f == 8 + n => 2 + file_list_extra + 7 + n + 3,  // dual_translate_first
            f if f == 9 + n => 2 + file_list_extra + 7 + n + 4,  // skip_clean
            f if f == 10 + n => 2 + file_list_extra + 7 + n + 6, // custom_prompt (after section)
            f if f == 11 + n => 2 + file_list_extra + 7 + n + 7, // min_text_length
            f if f == 12 + n => 2 + file_list_extra + 7 + n + 8,
            f if f == 13 + n => 2 + file_list_extra + 7 + n + 9,
            f if f == 14 + n => 2 + file_list_extra + 7 + n + 10,
            f if f == 15 + n => 2 + file_list_extra + 7 + n + 11,
            f if f == 16 + n => 2 + file_list_extra + 7 + n + 12,
            f if f == 17 + n => 2 + file_list_extra + 7 + n + 13,
            f if f == 18 + n => 2 + file_list_extra + 7 + n + 14,
            f if f == 19 + n => 2 + file_list_extra + 7 + n + 15,
            f if f == 20 + n => 2 + file_list_extra + 7 + n + 16,
            f if f == 21 + n => 2 + file_list_extra + 7 + n + 17,
            _ => 0,
        }
    }

    /// Map a key event to an action.
    pub fn handle_key(&self, key: KeyEvent) -> Action {
        // Popup takes priority
        if self.popup.is_some() {
            return match key.code {
                KeyCode::Esc | KeyCode::Enter => Action::DismissPopup,
                _ => Action::None,
            };
        }

        // Global shortcuts (Ctrl+key)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('t') if self.screen == Screen::Configure => {
                    return Action::StartTranslation;
                }
                KeyCode::Char('x') if self.screen == Screen::Translating => {
                    return Action::CancelTranslation;
                }
                KeyCode::Char('s') => return Action::SaveConfig,
                KeyCode::Char('c') | KeyCode::Char('q') => return Action::Quit,
                _ => {}
            }
        }

        match key.code {
            KeyCode::F(1) => return Action::ShowHelp,
            KeyCode::Char('?') if !self.editing && self.dropdown_open.is_none() => {
                return Action::ShowHelp;
            }
            _ => {}
        }

        match self.screen {
            Screen::Loading => Action::None,
            Screen::Configure => self.handle_configure_key(key),
            Screen::Translating => self.handle_translating_key(key),
            Screen::Results => self.handle_results_key(key),
        }
    }

    fn handle_configure_key(&self, key: KeyEvent) -> Action {
        // Dropdown mode
        if self.dropdown_open.is_some() {
            return match key.code {
                KeyCode::Up => Action::DropdownUp,
                KeyCode::Down => Action::DropdownDown,
                KeyCode::Enter => Action::SelectDropdownItem(self.dropdown_cursor),
                KeyCode::Esc => Action::CloseDropdown,
                KeyCode::Char(c) => Action::CharInput(c), // type-to-filter
                _ => Action::None,
            };
        }

        // Text editing mode
        if self.editing {
            return match key.code {
                KeyCode::Esc | KeyCode::Enter => Action::ExitField,
                KeyCode::Backspace => Action::Backspace,
                KeyCode::Char(c) => Action::CharInput(c),
                KeyCode::Up => Action::ExitField,
                KeyCode::Down => Action::ExitField,
                _ => Action::None,
            };
        }

        // Normal navigation
        match key.code {
            KeyCode::Up => Action::FocusPrev,
            KeyCode::Down => Action::FocusNext,
            KeyCode::Enter => Action::EnterField,
            KeyCode::Left | KeyCode::Right => Action::SwitchPanel,
            KeyCode::Delete | KeyCode::Backspace => Action::DeleteFocusedFile,
            _ => Action::None,
        }
    }

    fn handle_translating_key(&self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::CancelTranslation,
            _ => Action::None,
        }
    }

    fn handle_results_key(&self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => Action::BackToConfigure,
            _ => Action::None,
        }
    }

    /// Apply an action to update state.
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,

            // Panel switch
            Action::SwitchPanel => {
                self.active_panel = match self.active_panel {
                    ActivePanel::Left => ActivePanel::Right,
                    ActivePanel::Right => ActivePanel::Left,
                };
                self.editing = false;
                self.dropdown_open = None;
            }

            // Field navigation
            Action::FocusNext => {
                let max = self.max_fields();
                if self.focused_field + 1 < max {
                    self.focused_field += 1;
                }
            }
            Action::FocusPrev => {
                self.focused_field = self.focused_field.saturating_sub(1);
            }

            // Enter a field
            Action::EnterField => {
                if self.is_checkbox_field() {
                    self.toggle_checkbox();
                } else if self.is_dropdown_field() {
                    let target = self.dropdown_target().unwrap();
                    self.dropdown_cursor = match target {
                        DropdownTarget::LangIn => self.lang_in_idx,
                        DropdownTarget::LangOut => self.lang_out_idx,
                        DropdownTarget::Engine => self.engine_idx,
                    };
                    self.dropdown_open = Some(target);
                } else {
                    // Text field — enter editing mode
                    self.editing = true;
                    // Initialize QPS input buffer
                    if self.focused_field == 5 {
                        self.qps_input = self.qps.to_string();
                    }
                }
            }

            // Exit editing
            Action::ExitField => {
                self.editing = false;
                // Finalize QPS
                if self.focused_field == 5 {
                    if let Ok(v) = self.qps_input.parse::<u32>() {
                        self.qps = v.max(1);
                    }
                    self.qps_input = self.qps.to_string();
                }
            }

            // Text input
            Action::CharInput(c) => {
                if self.dropdown_open.is_some() {
                    self.dropdown_type_search(c);
                } else if self.editing {
                    self.handle_char_input(c);
                }
            }
            Action::Backspace => {
                if self.editing {
                    self.handle_backspace();
                }
            }

            // Dropdown navigation
            Action::DropdownUp => {
                if self.dropdown_cursor > 0 {
                    self.dropdown_cursor -= 1;
                }
            }
            Action::DropdownDown => {
                if let Some(target) = self.dropdown_open {
                    let max = self.dropdown_len(target);
                    if self.dropdown_cursor + 1 < max {
                        self.dropdown_cursor += 1;
                    }
                }
            }
            Action::SelectDropdownItem(idx) => {
                if let Some(target) = self.dropdown_open.take() {
                    match target {
                        DropdownTarget::LangIn => {
                            if idx < self.language_map.len() {
                                self.lang_in_idx = idx;
                            }
                        }
                        DropdownTarget::LangOut => {
                            if idx < self.language_map.len() {
                                self.lang_out_idx = idx;
                            }
                        }
                        DropdownTarget::Engine => {
                            if idx < self.engine_schemas.len() && idx != self.engine_idx {
                                self.engine_idx = idx;
                                self.reset_engine_params();
                            }
                        }
                    }
                }
            }
            Action::CloseDropdown => {
                self.dropdown_open = None;
            }
            Action::OpenDropdown => {
                if self.is_dropdown_field() {
                    let target = self.dropdown_target().unwrap();
                    self.dropdown_cursor = match target {
                        DropdownTarget::LangIn => self.lang_in_idx,
                        DropdownTarget::LangOut => self.lang_out_idx,
                        DropdownTarget::Engine => self.engine_idx,
                    };
                    self.dropdown_open = Some(target);
                }
            }

            // File management
            Action::AddFile(path) => {
                let path = path.trim().to_string();
                let path = if path.starts_with("~/") {
                    if let Some(home) = dirs::home_dir() {
                        home.join(&path[2..]).to_string_lossy().to_string()
                    } else {
                        path
                    }
                } else {
                    path
                };
                if !path.is_empty() && !self.selected_files.contains(&path) {
                    self.selected_files.push(path);
                }
            }
            Action::RemoveFile(idx) => {
                if idx < self.selected_files.len() {
                    self.selected_files.remove(idx);
                }
            }
            Action::DeleteFocusedFile => {
                if self.focused_field == 0 && !self.selected_files.is_empty() {
                    self.selected_files.pop();
                }
            }

            // Screen transitions
            Action::BackToConfigure => {
                self.screen = Screen::Configure;
                self.results.clear();
                self.token_usage = None;
            }

            // Popups
            Action::ShowError(msg) => self.popup = Some(Popup::Error(msg)),
            Action::ShowHelp => self.popup = Some(Popup::Help),
            Action::DismissPopup => self.popup = None,

            // Python events
            Action::PythonEvent(event) => self.handle_python_event(event),

            // No-ops
            _ => {}
        }
    }

    /// Handle character input based on unified field index.
    fn handle_char_input(&mut self, c: char) {
        let n = self.engine_param_count();
        match self.focused_field {
            0 => self.file_input.push(c),
            3 => self.pages_input.push(c),
            5 => {
                if c.is_ascii_digit() {
                    self.qps_input.push(c);
                }
            }
            f if (6..6 + n).contains(&f) => {
                let param_idx = f - 6;
                if let Some(schema) = self.engine_schemas.get(self.engine_idx) {
                    if let Some(field) = schema.fields.get(param_idx) {
                        self.engine_params
                            .entry(field.name.clone())
                            .or_default()
                            .push(c);
                    }
                }
            }
            f if f == 10 + n => self.custom_prompt.push(c),
            f if f == 11 + n => {
                if c.is_ascii_digit() {
                    self.min_text_length.push(c);
                }
            }
            f if f == 12 + n => {
                if c.is_ascii_digit() {
                    self.pool_max_workers_input.push(c);
                }
            }
            f if f == 13 + n => {
                if c.is_ascii_digit() {
                    self.term_qps_input.push(c);
                }
            }
            f if f == 14 + n => {
                if c.is_ascii_digit() {
                    self.term_pool_max_workers_input.push(c);
                }
            }
            f if f == 15 + n => self.output_dir.push(c),
            f if f == 16 + n => self.glossary_files.push(c),
            f if f == 17 + n => {
                if c.is_ascii_digit() {
                    self.max_pages_per_part_input.push(c);
                }
            }
            _ => {}
        }
    }

    /// Handle backspace input based on unified field index.
    fn handle_backspace(&mut self) {
        let n = self.engine_param_count();
        match self.focused_field {
            0 => {
                self.file_input.pop();
            }
            3 => {
                self.pages_input.pop();
            }
            5 => {
                self.qps_input.pop();
            }
            f if (6..6 + n).contains(&f) => {
                let param_idx = f - 6;
                if let Some(schema) = self.engine_schemas.get(self.engine_idx) {
                    if let Some(field) = schema.fields.get(param_idx) {
                        if let Some(val) = self.engine_params.get_mut(&field.name) {
                            val.pop();
                        }
                    }
                }
            }
            f if f == 10 + n => {
                self.custom_prompt.pop();
            }
            f if f == 11 + n => {
                self.min_text_length.pop();
            }
            f if f == 12 + n => {
                self.pool_max_workers_input.pop();
            }
            f if f == 13 + n => {
                self.term_qps_input.pop();
            }
            f if f == 14 + n => {
                self.term_pool_max_workers_input.pop();
            }
            f if f == 15 + n => {
                self.output_dir.pop();
            }
            f if f == 16 + n => {
                self.glossary_files.pop();
            }
            f if f == 17 + n => {
                self.max_pages_per_part_input.pop();
            }
            _ => {}
        }
    }

    /// Toggle checkbox for the currently focused field.
    fn toggle_checkbox(&mut self) {
        let n = self.engine_param_count();
        match self.focused_field {
            f if f == 6 + n => self.no_dual = !self.no_dual,
            f if f == 7 + n => self.no_mono = !self.no_mono,
            f if f == 8 + n => self.dual_translate_first = !self.dual_translate_first,
            f if f == 9 + n => self.skip_clean = !self.skip_clean,
            f if f == 18 + n => self.skip_scanned_detection = !self.skip_scanned_detection,
            f if f == 19 + n => {
                self.save_auto_extracted_glossary = !self.save_auto_extracted_glossary;
            }
            f if f == 20 + n => {
                self.only_include_translated_page = !self.only_include_translated_page;
            }
            f if f == 21 + n => {
                self.disable_auto_extract_glossary = !self.disable_auto_extract_glossary;
            }
            _ => {}
        }
    }

    /// Reset engine params when switching engines.
    fn reset_engine_params(&mut self) {
        self.engine_params.clear();
        if let Some(schema) = self.engine_schemas.get(self.engine_idx) {
            for field in &schema.fields {
                let default_str = match &field.default {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                };
                if !default_str.is_empty() {
                    self.engine_params.insert(field.name.clone(), default_str);
                }
            }
        }
    }

    /// Jump to first item in dropdown matching typed character.
    fn dropdown_type_search(&mut self, c: char) {
        let c_lower = c.to_lowercase().next().unwrap_or(c);
        if let Some(target) = self.dropdown_open {
            let items: Vec<&str> = match target {
                DropdownTarget::LangIn | DropdownTarget::LangOut => {
                    self.language_map.keys().map(|k| k.as_str()).collect()
                }
                DropdownTarget::Engine => self
                    .engine_schemas
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect(),
            };
            let len = items.len();
            for offset in 1..=len {
                let idx = (self.dropdown_cursor + offset) % len;
                if let Some(first_char) = items[idx].chars().next() {
                    if first_char.to_lowercase().next() == Some(c_lower) {
                        self.dropdown_cursor = idx;
                        return;
                    }
                }
            }
        }
    }

    fn handle_python_event(&mut self, event: PythonEvent) {
        match event {
            PythonEvent::Ready { version, .. } => {
                self.python_version = version;
                self.screen = Screen::Configure;
            }
            PythonEvent::ConfigSchema {
                engines, languages, ..
            } => {
                self.engine_schemas = engines;
                self.language_map = languages;
                // 默认引擎: Gemini
                if let Some(idx) = self
                    .engine_schemas
                    .iter()
                    .position(|s| s.name.eq_ignore_ascii_case("gemini") || s.cli_flag == "gemini")
                {
                    self.engine_idx = idx;
                }
                self.reset_engine_params();
                if let Some(idx) = self.language_map.get_index_of("Simplified Chinese") {
                    self.lang_out_idx = idx;
                }
            }
            PythonEvent::StageSummary {
                part_index,
                total_parts,
                ..
            } => {
                self.progress.part_index = part_index;
                self.progress.total_parts = total_parts;
            }
            PythonEvent::ProgressStart(data) | PythonEvent::ProgressUpdate(data) => {
                self.progress.stage = data.stage;
                self.progress.stage_progress = data.stage_progress;
                self.progress.overall_progress = data.overall_progress;
                self.progress.stage_current = data.stage_current;
                self.progress.stage_total = data.stage_total;
                self.progress.part_index = data.part_index;
                self.progress.total_parts = data.total_parts;
            }
            PythonEvent::ProgressEnd(data) => {
                self.progress.stage = data.stage;
                self.progress.stage_progress = 100.0;
                self.progress.overall_progress = data.overall_progress;
            }
            PythonEvent::Finish {
                translate_result,
                token_usage,
            } => {
                self.results.push(translate_result);
                if let Some(usage) = token_usage {
                    let new_val = serde_json::to_value(&usage).unwrap_or_default();
                    self.token_usage = Some(match self.token_usage.take() {
                        Some(existing) => merge_token_usage(existing, new_val),
                        None => new_val,
                    });
                }
                self.progress.finished_count += 1;

                if self.progress.finished_count >= self.progress.total_files {
                    self.screen = Screen::Results;
                } else {
                    self.progress.current_file_idx = self.progress.finished_count;
                    if let Some(next_file) = self.selected_files.get(self.progress.current_file_idx)
                    {
                        self.progress.current_file = std::path::Path::new(next_file)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(next_file)
                            .to_string();
                    }
                    self.progress.stage = "等待下一个文件...".to_string();
                    self.progress.stage_progress = 0.0;
                    self.push_log(format!(
                        "文件 {}/{} 翻译完成",
                        self.progress.finished_count, self.progress.total_files,
                    ));
                }
            }
            PythonEvent::Error {
                error,
                error_type,
                details,
            } => {
                let msg = if details.is_empty() {
                    format!("[{error_type}] {error}")
                } else {
                    format!("[{error_type}] {error}\n{details}")
                };
                self.popup = Some(Popup::Error(msg));
                if self.screen == Screen::Translating {
                    self.screen = Screen::Configure;
                }
            }
            PythonEvent::ValidationResult { valid, error } => {
                if !valid {
                    let msg = error.unwrap_or_else(|| "Validation failed".to_string());
                    self.popup = Some(Popup::Error(msg));
                }
            }
        }
    }

    /// Build the settings JSON to send to Python.
    pub fn build_settings_json(&self) -> serde_json::Value {
        let lang_in = self
            .language_map
            .get_index(self.lang_in_idx)
            .map(|(_, v)| v.as_str())
            .unwrap_or("en");
        let lang_out = self
            .language_map
            .get_index(self.lang_out_idx)
            .map(|(_, v)| v.as_str())
            .unwrap_or("zh-CN");

        let mut settings = serde_json::json!({
            "basic": {
                "input_files": [],
                "debug": false,
                "gui": false,
            },
            "translation": {
                "lang_in": lang_in,
                "lang_out": lang_out,
                "qps": self.qps,
            },
            "pdf": {
                "no_dual": self.no_dual,
                "no_mono": self.no_mono,
                "dual_translate_first": self.dual_translate_first,
            },
        });

        if !self.pages_input.is_empty() {
            settings["pdf"]["pages"] = serde_json::Value::String(self.pages_input.clone());
        }

        if !self.custom_prompt.is_empty() {
            settings["translation"]["custom_system_prompt"] =
                serde_json::Value::String(self.custom_prompt.clone());
        }

        if !self.min_text_length.is_empty() {
            if let Ok(n) = self.min_text_length.parse::<i64>() {
                settings["translation"]["min_text_length"] = serde_json::Value::Number(n.into());
            }
        }

        if self.skip_clean {
            settings["pdf"]["skip_clean"] = serde_json::Value::Bool(true);
        }

        if let Some(n) = parse_positive_i64(&self.pool_max_workers_input) {
            settings["translation"]["pool_max_workers"] = serde_json::Value::Number(n.into());
        }
        if let Some(n) = parse_positive_i64(&self.term_qps_input) {
            settings["translation"]["term_qps"] = serde_json::Value::Number(n.into());
        }
        if let Some(n) = parse_positive_i64(&self.term_pool_max_workers_input) {
            settings["translation"]["term_pool_max_workers"] = serde_json::Value::Number(n.into());
        }
        if !self.output_dir.trim().is_empty() {
            settings["translation"]["output"] =
                serde_json::Value::String(self.output_dir.trim().to_string());
        }
        if !self.glossary_files.trim().is_empty() {
            settings["translation"]["glossaries"] =
                serde_json::Value::String(self.glossary_files.trim().to_string());
        }
        if self.save_auto_extracted_glossary {
            settings["translation"]["save_auto_extracted_glossary"] = serde_json::Value::Bool(true);
        }
        if self.disable_auto_extract_glossary {
            settings["translation"]["no_auto_extract_glossary"] = serde_json::Value::Bool(true);
        }
        if let Some(n) = parse_positive_i64(&self.max_pages_per_part_input) {
            settings["pdf"]["max_pages_per_part"] = serde_json::Value::Number(n.into());
        }
        if self.skip_scanned_detection {
            settings["pdf"]["skip_scanned_detection"] = serde_json::Value::Bool(true);
        }
        if self.only_include_translated_page {
            settings["pdf"]["only_include_translated_page"] = serde_json::Value::Bool(true);
        }

        if let Some(schema) = self.engine_schemas.get(self.engine_idx) {
            let mut engine_settings = serde_json::json!({"translate_engine_type": schema.name});
            for field in &schema.fields {
                if let Some(value) = self.engine_params.get(&field.name) {
                    if !value.is_empty() {
                        match field.field_type.as_str() {
                            "bool" => {
                                engine_settings[&field.name] =
                                    serde_json::Value::Bool(value == "true");
                            }
                            "int" => {
                                if let Ok(n) = value.parse::<i64>() {
                                    engine_settings[&field.name] =
                                        serde_json::Value::Number(n.into());
                                }
                            }
                            _ => {
                                engine_settings[&field.name] =
                                    serde_json::Value::String(value.clone());
                            }
                        }
                    }
                } else if !field.default.is_null() {
                    engine_settings[&field.name] = field.default.clone();
                }
            }
            settings["translate_engine_settings"] = engine_settings;
        }

        settings
    }

    /// Prepare the app state for starting translation.
    pub fn prepare_translation(&mut self) -> Option<String> {
        if self.selected_files.is_empty() {
            return Some("请先选择要翻译的 PDF 文件".to_string());
        }

        for f in &self.selected_files {
            let path = std::path::Path::new(f);
            if !path.exists() {
                return Some(format!("文件不存在: {f}"));
            }
            if !path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
            {
                return Some(format!("不是 PDF 文件: {f}"));
            }
        }

        self.progress = ProgressState::default();
        self.progress.total_files = self.selected_files.len();
        self.progress.current_file_idx = 0;
        self.progress.start_time = Some(Instant::now());
        if let Some(first) = self.selected_files.first() {
            self.progress.current_file = std::path::Path::new(first)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(first)
                .to_string();
        }

        self.results.clear();
        self.token_usage = None;
        self.screen = Screen::Translating;
        self.log_lines.clear();
        self.push_log(format!("开始翻译 {} 个文件...", self.selected_files.len()));

        None
    }

    /// Get elapsed time since translation started.
    pub fn elapsed_time(&self) -> Option<std::time::Duration> {
        self.progress.start_time.map(|t| t.elapsed())
    }

    /// Format elapsed time as MM:SS.
    pub fn elapsed_display(&self) -> String {
        match self.elapsed_time() {
            Some(d) => {
                let secs = d.as_secs();
                format!("{:02}:{:02}", secs / 60, secs % 60)
            }
            None => "--:--".to_string(),
        }
    }

    /// Add a log line to the buffer.
    pub fn push_log(&mut self, line: String) {
        if self.log_lines.len() >= 200 {
            self.log_lines.pop_front();
        }
        self.log_lines.push_back(line);
    }

    /// Draw the UI.
    pub fn draw(&self, frame: &mut Frame) {
        ui::draw(frame, self);
    }
}

/// 累加合并两组 token 用量 JSON。
fn merge_token_usage(
    mut existing: serde_json::Value,
    new_val: serde_json::Value,
) -> serde_json::Value {
    for (key, new_entry) in new_val.as_object().into_iter().flatten() {
        if let Some(existing_entry) = existing.get_mut(key).and_then(|v| v.as_object_mut()) {
            for field in ["total", "prompt", "completion", "cache_hit_prompt"] {
                let add = new_entry.get(field).and_then(|v| v.as_i64()).unwrap_or(0);
                let cur = existing_entry
                    .get(field)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                existing_entry.insert(
                    field.to_string(),
                    serde_json::Value::Number((cur + add).into()),
                );
            }
        } else {
            existing[key] = new_entry.clone();
        }
    }
    existing
}

fn parse_positive_i64(input: &str) -> Option<i64> {
    input.trim().parse::<i64>().ok().filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::App;

    #[test]
    fn build_settings_json_includes_advanced_translation_and_pdf_options() {
        let mut app = App::new();
        app.qps = 5;
        app.pool_max_workers_input = "6".to_string();
        app.term_qps_input = "4".to_string();
        app.term_pool_max_workers_input = "3".to_string();
        app.output_dir = "/tmp/pdf2zh-output".to_string();
        app.glossary_files = "/tmp/glossary.csv".to_string();
        app.max_pages_per_part_input = "50".to_string();
        app.skip_scanned_detection = true;
        app.save_auto_extracted_glossary = true;
        app.only_include_translated_page = true;
        app.disable_auto_extract_glossary = true;

        let settings = app.build_settings_json();

        assert_eq!(settings["translation"]["qps"], 5);
        assert_eq!(settings["translation"]["pool_max_workers"], 6);
        assert_eq!(settings["translation"]["term_qps"], 4);
        assert_eq!(settings["translation"]["term_pool_max_workers"], 3);
        assert_eq!(settings["translation"]["output"], "/tmp/pdf2zh-output");
        assert_eq!(settings["translation"]["glossaries"], "/tmp/glossary.csv");
        assert_eq!(
            settings["translation"]["save_auto_extracted_glossary"],
            true
        );
        assert_eq!(settings["translation"]["no_auto_extract_glossary"], true);
        assert_eq!(settings["pdf"]["max_pages_per_part"], 50);
        assert_eq!(settings["pdf"]["skip_scanned_detection"], true);
        assert_eq!(settings["pdf"]["only_include_translated_page"], true);
    }
}
