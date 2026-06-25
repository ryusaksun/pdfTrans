use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{ActivePanel, App};

const ACCENT: Color = Color::Cyan;
const LABEL: Color = Color::Yellow;
const SECTION: Color = Color::DarkGray;

/// Draw the unified configure panel (all settings in one scrollable page).
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == ActivePanel::Left;
    let n = app.engine_param_count();

    // 构建所有行
    let mut rows: Vec<Row> = Vec::new();

    // ── 文件 ──
    rows.push(Row::Section("── 文件 ──"));
    rows.push(Row::Field { index: 0 }); // file_input
                                        // 已选文件列表（不可聚焦）
    if app.selected_files.is_empty() {
        rows.push(Row::Static(
            "  (未选择文件，输入路径后按Enter添加，按Del删除)",
            SECTION,
        ));
    } else {
        for (i, f) in app.selected_files.iter().enumerate() {
            let name = std::path::Path::new(f)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(f);
            rows.push(Row::FileItem(i + 1, name.to_string()));
        }
    }

    // ── 语言 ──
    rows.push(Row::Section("── 语言 ──"));
    rows.push(Row::Field { index: 1 }); // lang_in
    rows.push(Row::Field { index: 2 }); // lang_out
    rows.push(Row::Field { index: 3 }); // pages

    // ── 翻译引擎 ──
    rows.push(Row::Section("── 翻译引擎 ──"));
    rows.push(Row::Field { index: 4 }); // engine
    rows.push(Row::Field { index: 5 }); // qps
    for i in 0..n {
        rows.push(Row::Field { index: 6 + i }); // engine params
    }

    // ── PDF 选项 ──
    rows.push(Row::Section("── PDF 选项 ──"));
    rows.push(Row::Field { index: 6 + n }); // no_dual
    rows.push(Row::Field { index: 7 + n }); // no_mono
    rows.push(Row::Field { index: 8 + n }); // dual_translate_first
    rows.push(Row::Field { index: 9 + n }); // skip_clean

    // ── 高级 ──
    rows.push(Row::Section("── 高级 ──"));
    rows.push(Row::Field { index: 10 + n }); // custom_prompt
    rows.push(Row::Field { index: 11 + n }); // min_text_length
    rows.push(Row::Field { index: 12 + n }); // pool_max_workers
    rows.push(Row::Field { index: 13 + n }); // term_qps
    rows.push(Row::Field { index: 14 + n }); // term_pool_max_workers
    rows.push(Row::Field { index: 15 + n }); // output_dir
    rows.push(Row::Field { index: 16 + n }); // glossary_files
    rows.push(Row::Field { index: 17 + n }); // max_pages_per_part
    rows.push(Row::Field { index: 18 + n }); // skip_scanned_detection
    rows.push(Row::Field { index: 19 + n }); // save_auto_extracted_glossary
    rows.push(Row::Field { index: 20 + n }); // only_include_translated_page
    rows.push(Row::Field { index: 21 + n }); // disable_auto_extract_glossary

    // 渲染可见行
    let visible_height = area.height as usize;
    let scroll = app.scroll_offset;

    for (i, row) in rows.iter().enumerate().skip(scroll).take(visible_height) {
        let y = area.y + (i - scroll) as u16;
        if y >= area.y + area.height {
            break;
        }
        let row_area = Rect::new(area.x, y, area.width, 1);

        match row {
            Row::Section(title) => {
                let line = Line::styled(*title, Style::default().fg(SECTION));
                frame.render_widget(Paragraph::new(line), row_area);
            }
            Row::Static(text, color) => {
                let line = Line::styled(*text, Style::default().fg(*color));
                frame.render_widget(Paragraph::new(line), row_area);
            }
            Row::FileItem(num, name) => {
                let line = Line::styled(
                    format!("  {}. {}", num, name),
                    Style::default().fg(Color::White),
                );
                frame.render_widget(Paragraph::new(line), row_area);
            }
            Row::Field { index } => {
                render_field(frame, app, *index, n, is_active, row_area);
            }
        }
    }
}

enum Row<'a> {
    Section(&'a str),
    Static(&'a str, Color),
    FileItem(usize, String),
    Field { index: usize },
}

fn render_field(
    frame: &mut Frame,
    app: &App,
    field_idx: usize,
    n: usize,
    is_active: bool,
    area: Rect,
) {
    let focused = is_active && app.focused_field == field_idx;
    let editing = app.editing && focused;

    match field_idx {
        0 => {
            let style = field_style(focused, editing);
            let display = if editing {
                format!("{}|", app.file_input)
            } else if app.file_input.is_empty() {
                "<输入PDF路径后按Enter添加>".to_string()
            } else {
                app.file_input.clone()
            };
            render_labeled_field(frame, "文件路径: ", &display, style, area);
        }
        1 => {
            let name = app
                .language_map
                .get_index(app.lang_in_idx)
                .map(|(k, _)| k.as_str())
                .unwrap_or("English");
            render_labeled_field(
                frame,
                "源语言:   ",
                &format!("{name} ▼"),
                dropdown_style(focused),
                area,
            );
        }
        2 => {
            let name = app
                .language_map
                .get_index(app.lang_out_idx)
                .map(|(k, _)| k.as_str())
                .unwrap_or("Simplified Chinese");
            render_labeled_field(
                frame,
                "目标语言: ",
                &format!("{name} ▼"),
                dropdown_style(focused),
                area,
            );
        }
        3 => {
            let display = if editing {
                format!("{}|", app.pages_input)
            } else if app.pages_input.is_empty() {
                "全部".to_string()
            } else {
                app.pages_input.clone()
            };
            render_labeled_field(
                frame,
                "页面范围: ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        4 => {
            let name = app
                .engine_schemas
                .get(app.engine_idx)
                .map(|s| s.name.as_str())
                .unwrap_or("(无)");
            render_labeled_field(
                frame,
                "翻译引擎: ",
                &format!("{name} ▼"),
                dropdown_style(focused),
                area,
            );
        }
        5 => {
            let qps_default = app.qps.to_string();
            let display = if editing {
                &app.qps_input
            } else {
                &qps_default
            };
            render_labeled_field(
                frame,
                "QPS:      ",
                display,
                field_style(focused, editing),
                area,
            );
        }
        f if f >= 6 && f < 6 + n => {
            let param_idx = f - 6;
            if let Some(schema) = app.engine_schemas.get(app.engine_idx) {
                if let Some(field) = schema.fields.get(param_idx) {
                    let value = app
                        .engine_params
                        .get(&field.name)
                        .map(|s| s.as_str())
                        .unwrap_or("");

                    let display_value = if editing {
                        format!("{value}|")
                    } else if field.password && !value.is_empty() {
                        "********".to_string()
                    } else if value.is_empty() {
                        match &field.default {
                            serde_json::Value::String(s) if !s.is_empty() => format!("{s} (默认)"),
                            serde_json::Value::Null => "(未设置)".to_string(),
                            serde_json::Value::String(_) => "(未设置)".to_string(),
                            other => format!("{other} (默认)"),
                        }
                    } else {
                        value.to_string()
                    };

                    let label = format!("{:width$}", format!("{}: ", field.name), width = 10);
                    render_labeled_field(
                        frame,
                        &label,
                        &display_value,
                        field_style(focused, editing),
                        area,
                    );
                }
            }
        }
        f if f == 6 + n => render_checkbox(frame, !app.no_dual, "输出双语PDF", focused, area),
        f if f == 7 + n => render_checkbox(frame, !app.no_mono, "输出单语PDF", focused, area),
        f if f == 8 + n => render_checkbox(
            frame,
            app.dual_translate_first,
            "双语翻译优先",
            focused,
            area,
        ),
        f if f == 9 + n => render_checkbox(frame, app.skip_clean, "跳过清理", focused, area),
        f if f == 10 + n => {
            let display = if editing {
                format!("{}|", app.custom_prompt)
            } else if app.custom_prompt.is_empty() {
                "(使用默认)".to_string()
            } else {
                app.custom_prompt.clone()
            };
            render_labeled_field(
                frame,
                "自定义提示: ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        f if f == 11 + n => {
            let display = if editing {
                format!("{}|", app.min_text_length)
            } else if app.min_text_length.is_empty() {
                "默认".to_string()
            } else {
                app.min_text_length.clone()
            };
            render_labeled_field(
                frame,
                "最小文本长度: ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        f if f == 12 + n => {
            let display = numeric_display(editing, &app.pool_max_workers_input);
            render_labeled_field(
                frame,
                "翻译线程: ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        f if f == 13 + n => {
            let display = numeric_display(editing, &app.term_qps_input);
            render_labeled_field(
                frame,
                "术语QPS:  ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        f if f == 14 + n => {
            let display = numeric_display(editing, &app.term_pool_max_workers_input);
            render_labeled_field(
                frame,
                "术语线程: ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        f if f == 15 + n => {
            let display = text_display(editing, &app.output_dir, "默认");
            render_labeled_field(
                frame,
                "输出目录: ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        f if f == 16 + n => {
            let display = text_display(editing, &app.glossary_files, "(未设置)");
            render_labeled_field(
                frame,
                "术语表CSV: ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        f if f == 17 + n => {
            let display = numeric_display(editing, &app.max_pages_per_part_input);
            render_labeled_field(
                frame,
                "每部分页数: ",
                &display,
                field_style(focused, editing),
                area,
            );
        }
        f if f == 18 + n => {
            render_checkbox(
                frame,
                app.skip_scanned_detection,
                "跳过扫描检测",
                focused,
                area,
            );
        }
        f if f == 19 + n => {
            render_checkbox(
                frame,
                app.save_auto_extracted_glossary,
                "保存自动术语表",
                focused,
                area,
            );
        }
        f if f == 20 + n => {
            render_checkbox(
                frame,
                app.only_include_translated_page,
                "仅输出选中页",
                focused,
                area,
            );
        }
        f if f == 21 + n => {
            render_checkbox(
                frame,
                app.disable_auto_extract_glossary,
                "禁用自动术语抽取",
                focused,
                area,
            );
        }
        _ => {}
    }
}

fn numeric_display(editing: bool, value: &str) -> String {
    if editing {
        format!("{value}|")
    } else if value.is_empty() {
        "默认".to_string()
    } else {
        value.to_string()
    }
}

fn text_display(editing: bool, value: &str, empty_label: &str) -> String {
    if editing {
        format!("{value}|")
    } else if value.is_empty() {
        empty_label.to_string()
    } else {
        value.to_string()
    }
}

fn field_style(focused: bool, editing: bool) -> Style {
    if focused {
        if editing {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default()
    }
}

fn dropdown_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn render_checkbox(frame: &mut Frame, checked: bool, label: &str, focused: bool, area: Rect) {
    let mark = if checked { "☑" } else { "☐" };
    let style = if focused {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let text = format!("  {mark} {label}");
    frame.render_widget(Paragraph::new(text).style(style), area);
}

/// 截断字符串到指定显示宽度，超出部分用 … 替代
fn truncate_to_width(s: &str, max_width: usize) -> String {
    let w = UnicodeWidthStr::width(s);
    if w <= max_width {
        return s.to_string();
    }
    let mut result = String::new();
    let mut cur_w = 0;
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if cur_w + cw + 1 > max_width {
            result.push('…');
            break;
        }
        result.push(c);
        cur_w += cw;
    }
    result
}

/// 渲染 "标签: [值]" 格式的字段行，自动截断值部分
fn render_labeled_field(frame: &mut Frame, label: &str, value: &str, style: Style, area: Rect) {
    let label_w = UnicodeWidthStr::width(label);
    // [, ], 和至少 3 字符的值
    let max_val_w = (area.width as usize).saturating_sub(label_w + 2);
    let truncated = truncate_to_width(value, max_val_w);
    let line = Line::from(vec![
        Span::styled(label, Style::default().fg(LABEL)),
        Span::styled(format!("[{truncated}]"), style),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
