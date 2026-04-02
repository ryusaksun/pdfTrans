use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::styled(
        "翻译完成!",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));

    // Total elapsed time
    let elapsed = app.elapsed_display();
    lines.push(Line::styled(
        format!("总用时: {elapsed}"),
        Style::default().fg(Color::White),
    ));
    lines.push(Line::raw(""));

    for (i, result) in app.results.iter().enumerate() {
        if app.results.len() > 1 {
            lines.push(Line::styled(
                format!("--- 文件 {} ---", i + 1),
                Style::default().fg(Color::Yellow),
            ));
        }

        if let Some(mono) = result.get("mono_pdf_path").and_then(|v| v.as_str()) {
            if !mono.is_empty() && mono != "null" {
                lines.push(Line::from(format!("单语PDF: {mono}")));
            }
        }
        if let Some(dual) = result.get("dual_pdf_path").and_then(|v| v.as_str()) {
            if !dual.is_empty() && dual != "null" {
                lines.push(Line::from(format!("双语PDF: {dual}")));
            }
        }
        if let Some(glossary) = result
            .get("auto_extracted_glossary_path")
            .and_then(|v| v.as_str())
        {
            if !glossary.is_empty() && glossary != "null" {
                lines.push(Line::from(format!("术语表:  {glossary}")));
            }
        }
        if let Some(seconds) = result.get("total_seconds").and_then(|v| v.as_f64()) {
            lines.push(Line::styled(
                format!("耗时:    {seconds:.1}s"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        lines.push(Line::raw(""));
    }

    // Token usage
    if let Some(usage) = &app.token_usage {
        lines.push(Line::styled(
            "Token 用量:",
            Style::default().fg(Color::Yellow),
        ));
        if let Some(main) = usage.get("main") {
            let total = main.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
            let prompt = main.get("prompt").and_then(|v| v.as_i64()).unwrap_or(0);
            let completion = main
                .get("completion")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            lines.push(Line::from(format!(
                "  主翻译: 总计 {total}, 输入 {prompt}, 输出 {completion}"
            )));
        }
        if let Some(term) = usage.get("term") {
            let total = term.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
            let prompt = term.get("prompt").and_then(|v| v.as_i64()).unwrap_or(0);
            let completion = term
                .get("completion")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            lines.push(Line::from(format!(
                "  术语:   总计 {total}, 输入 {prompt}, 输出 {completion}"
            )));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "按 Enter 或 Esc 返回配置页面",
        Style::default().fg(Color::DarkGray),
    ));

    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}
