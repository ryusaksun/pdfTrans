use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // current file + batch info
            Constraint::Length(2), // stage name + elapsed
            Constraint::Length(3), // overall progress
            Constraint::Length(3), // stage progress
            Constraint::Length(2), // part info
            Constraint::Length(1), // spacer
            Constraint::Min(3),   // log output
        ])
        .split(area);

    let p = &app.progress;

    // Current file + batch info
    let file_info = if p.total_files > 1 {
        format!(
            "[{}/{}] {}  (已完成 {})",
            p.current_file_idx + 1,
            p.total_files,
            p.current_file,
            p.finished_count,
        )
    } else if !p.current_file.is_empty() {
        p.current_file.clone()
    } else {
        "准备中...".to_string()
    };
    frame.render_widget(
        Paragraph::new(file_info).style(Style::default().fg(Color::White)),
        chunks[0],
    );

    // Stage name + elapsed time
    let elapsed = app.elapsed_display();
    let stage_text = if p.stage_total > 0 {
        format!(
            "阶段: {} ({}/{})    已用时间: {}",
            p.stage, p.stage_current, p.stage_total, elapsed
        )
    } else if !p.stage.is_empty() {
        format!("阶段: {}    已用时间: {}", p.stage, elapsed)
    } else {
        format!("正在启动...    已用时间: {}", elapsed)
    };
    frame.render_widget(
        Paragraph::new(stage_text).style(Style::default().fg(Color::Yellow)),
        chunks[1],
    );

    // Overall progress gauge
    let overall_pct = p.overall_progress.clamp(0.0, 100.0) as u16;
    let overall_gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent(overall_pct)
        .label(format!("{:.1}% 总进度", p.overall_progress));
    frame.render_widget(overall_gauge, chunks[2]);

    // Stage progress gauge
    let stage_pct = p.stage_progress.clamp(0.0, 100.0) as u16;
    let stage_gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .percent(stage_pct)
        .label(format!("{:.1}% 当前阶段", p.stage_progress));
    frame.render_widget(stage_gauge, chunks[3]);

    // Part info
    if p.total_parts > 1 {
        let part_text = format!("Part {}/{}", p.part_index + 1, p.total_parts);
        frame.render_widget(
            Paragraph::new(part_text).style(Style::default().fg(Color::DarkGray)),
            chunks[4],
        );
    }

    // Log output (last N lines)
    let max_lines = chunks[6].height as usize;
    let log_lines: Vec<Line> = app
        .log_lines
        .iter()
        .rev()
        .take(max_lines)
        .rev()
        .map(|l| Line::from(l.as_str()).style(Style::default().fg(Color::DarkGray)))
        .collect();
    frame.render_widget(Paragraph::new(log_lines), chunks[6]);
}
