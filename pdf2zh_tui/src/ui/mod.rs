pub mod views;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use unicode_width::UnicodeWidthStr;

use crate::app::{ActivePanel, App, DropdownTarget, Popup, Screen};

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn draw(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Loading => draw_loading(frame, app),
        Screen::Configure | Screen::Translating | Screen::Results => draw_main(frame, app),
    }

    // Draw dropdown overlay if open
    if let Some(target) = app.dropdown_open {
        draw_dropdown_overlay(frame, app, target);
    }

    // Draw popup overlay if present
    if let Some(popup) = &app.popup {
        draw_popup(frame, popup);
    }
}

fn draw_loading(frame: &mut Frame, _app: &App) {
    let area = frame.area();
    let text = Paragraph::new("正在启动 pdf2zh_next Python 后端...")
        .alignment(Alignment::Center)
        .style(Style::default().fg(ACCENT))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" PDFMathTranslate TUI ")
                .title_alignment(Alignment::Center),
        );
    frame.render_widget(text, area);
}

fn draw_main(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(10),  // body
            Constraint::Length(1), // footer
        ])
        .split(area);

    draw_header(frame, app, chunks[0]);

    // Body: left + right
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    let left_style = if app.active_panel == ActivePanel::Left {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(DIM)
    };
    let right_style = if app.active_panel == ActivePanel::Right {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(DIM)
    };

    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_style(left_style)
        .title(" 配置 ")
        .padding(Padding::horizontal(1));

    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_style(right_style)
        .title(match app.screen {
            Screen::Translating => " 翻译进度 ",
            Screen::Results => " 翻译结果 ",
            _ => " 状态 ",
        })
        .padding(Padding::horizontal(1));

    let left_inner = left_block.inner(body_chunks[0]);
    let right_inner = right_block.inner(body_chunks[1]);

    frame.render_widget(left_block, body_chunks[0]);
    frame.render_widget(right_block, body_chunks[1]);

    // Left panel: unified configure view
    views::configure::draw(frame, app, left_inner);

    // Right panel content
    match app.screen {
        Screen::Translating => views::progress::draw(frame, app, right_inner),
        Screen::Results => views::results::draw(frame, app, right_inner),
        _ => views::status::draw(frame, app, right_inner),
    }

    draw_footer(frame, app, chunks[2]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(" PDFMathTranslate TUI v{} ", app.python_version);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_alignment(Alignment::Center);
    frame.render_widget(block, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let shortcuts = match app.screen {
        Screen::Configure => {
            if app.dropdown_open.is_some() {
                "[↑↓]选择 [Enter]确认 [Esc]取消"
            } else if app.editing {
                "[输入文本] [Enter/Esc]完成"
            } else {
                "[↑↓]导航 [Enter]编辑 [Del]删除文件 [Ctrl+T]翻译 [Ctrl+S]保存 [Ctrl+Q]退出"
            }
        }
        Screen::Translating => "[Ctrl+X]取消 [Ctrl+Q]退出",
        Screen::Results => "[Enter]返回配置 [Ctrl+Q]退出",
        Screen::Loading => "正在加载...",
    };

    let footer =
        Paragraph::new(shortcuts).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(footer, area);
}

/// Draw a dropdown overlay on top of the current layout.
fn draw_dropdown_overlay(frame: &mut Frame, app: &App, target: DropdownTarget) {
    let area = frame.area();

    let items: Vec<String> = match target {
        DropdownTarget::LangIn | DropdownTarget::LangOut => {
            app.language_map.keys().cloned().collect()
        }
        DropdownTarget::Engine => app.engine_schemas.iter().map(|s| s.name.clone()).collect(),
    };

    if items.is_empty() {
        return;
    }

    let max_visible = 12.min(items.len());
    let width = items.iter().map(|s| UnicodeWidthStr::width(s.as_str())).max().unwrap_or(10) + 6;
    let width = width.min(area.width as usize - 4).max(20) as u16;
    let height = (max_visible + 2) as u16;

    let left_panel_width = area.width * 45 / 100;
    let x = if width < left_panel_width {
        (left_panel_width - width) / 2 + 1
    } else {
        1
    };

    let y = (area.height.saturating_sub(height)) / 2;

    let dropdown_area = Rect::new(x, y, width, height);

    let scroll_offset = if app.dropdown_cursor >= max_visible {
        app.dropdown_cursor - max_visible + 1
    } else {
        0
    };

    let visible_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_visible)
        .map(|(i, name)| {
            let style = if i == app.dropdown_cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let selected = match target {
                DropdownTarget::LangIn => i == app.lang_in_idx,
                DropdownTarget::LangOut => i == app.lang_out_idx,
                DropdownTarget::Engine => i == app.engine_idx,
            };
            let prefix = if selected { "● " } else { "  " };
            ListItem::new(format!("{prefix}{name}")).style(style)
        })
        .collect();

    let title = match target {
        DropdownTarget::LangIn => " 源语言 ",
        DropdownTarget::LangOut => " 目标语言 ",
        DropdownTarget::Engine => " 翻译引擎 ",
    };

    frame.render_widget(Clear, dropdown_area);
    let list = List::new(visible_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .title(title)
            .title_alignment(Alignment::Center),
    );
    frame.render_widget(list, dropdown_area);

    if items.len() > max_visible {
        let indicator = format!(
            " {}/{} ",
            app.dropdown_cursor + 1,
            items.len()
        );
        let indicator_area = Rect::new(
            dropdown_area.x + dropdown_area.width.saturating_sub(indicator.len() as u16 + 1),
            dropdown_area.y + dropdown_area.height.saturating_sub(1),
            indicator.len() as u16,
            1,
        );
        frame.render_widget(
            Paragraph::new(indicator).style(Style::default().fg(DIM)),
            indicator_area,
        );
    }
}

fn draw_popup(frame: &mut Frame, popup: &Popup) {
    let area = frame.area();
    let popup_area = centered_rect(60, 40, area);

    frame.render_widget(Clear, popup_area);

    match popup {
        Popup::Error(msg) => {
            let text = Paragraph::new(msg.as_str())
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(Color::Red))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red))
                        .title(" 错误 ")
                        .title_alignment(Alignment::Center)
                        .padding(Padding::uniform(1)),
                );
            frame.render_widget(text, popup_area);
        }
        Popup::Help => {
            let help_text = vec![
                "Ctrl+T   开始翻译",
                "Ctrl+X   取消翻译",
                "Ctrl+S   保存配置",
                "Ctrl+Q   退出",
                "",
                "↑/↓      导航字段",
                "Enter    编辑/确认/切换复选框",
                "Esc      取消编辑/关闭下拉",
                "Del/⌫    删除最后一个选中文件",
                "←/→      切换左右面板",
                "?/F1     显示此帮助",
            ];
            let text = Paragraph::new(help_text.join("\n")).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT))
                    .title(" 快捷键帮助 ")
                    .title_alignment(Alignment::Center)
                    .padding(Padding::uniform(1)),
            );
            frame.render_widget(text, popup_area);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
