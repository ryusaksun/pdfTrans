use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::Paragraph,
    Frame,
};

use crate::app::App;

/// Draw the status panel (right side, when not translating).
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::styled(
        "状态: 就绪",
        Style::default().fg(Color::Green),
    ));
    lines.push(Line::raw(""));

    // Selected files
    let file_count = app.selected_files.len();
    let file_badge = if file_count == 0 {
        "已选文件: 0 个".to_string()
    } else {
        format!("已选文件: {} 个", file_count)
    };
    lines.push(Line::styled(
        file_badge,
        if file_count > 0 {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    ));
    for f in &app.selected_files {
        let name = std::path::Path::new(f)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(f);
        lines.push(Line::styled(
            format!("  - {name}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    lines.push(Line::raw(""));

    // Engine
    let engine_name = app
        .engine_schemas
        .get(app.engine_idx)
        .map(|s| s.name.as_str())
        .unwrap_or("(未选择)");
    lines.push(Line::from(format!("引擎: {engine_name}")));

    // Languages
    let lang_in = app
        .language_map
        .get_index(app.lang_in_idx)
        .map(|(k, _)| k.as_str())
        .unwrap_or("?");
    let lang_out = app
        .language_map
        .get_index(app.lang_out_idx)
        .map(|(k, _)| k.as_str())
        .unwrap_or("?");
    lines.push(Line::from(format!("语言: {lang_in} → {lang_out}")));
    lines.push(Line::from(format!("QPS:  {}", app.qps)));

    // Pages
    if !app.pages_input.is_empty() {
        lines.push(Line::from(format!("页面: {}", app.pages_input)));
    }

    // PDF options
    lines.push(Line::raw(""));
    let dual = if app.no_dual { "否" } else { "是" };
    let mono = if app.no_mono { "否" } else { "是" };
    lines.push(Line::styled(
        format!("双语: {dual}  单语: {mono}"),
        Style::default().fg(Color::DarkGray),
    ));

    // Engine params (show non-empty ones)
    if !app.engine_params.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "引擎参数:",
            Style::default().fg(Color::DarkGray),
        ));
        for (k, v) in &app.engine_params {
            if !v.is_empty() {
                // Mask sensitive values
                let display = if k.contains("key") || k.contains("secret") || k.contains("token")
                {
                    format!("  {k}: ****")
                } else {
                    format!("  {k}: {v}")
                };
                lines.push(Line::styled(display, Style::default().fg(Color::DarkGray)));
            }
        }
    }

    lines.push(Line::raw(""));
    if file_count > 0 {
        lines.push(Line::styled(
            "按 Ctrl+T 开始翻译",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        lines.push(Line::styled(
            "请在左侧添加 PDF 文件",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}
