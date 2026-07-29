//! 基础 TUI 组件。

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use super::theme::Theme;

pub fn progress_bar(label: &str, value: f64) -> Gauge<'_> {
    let pct = value.clamp(0.0, 100.0) as u16;
    let color = if pct >= 90 {
        Color::Red
    } else if pct >= 75 {
        Color::Yellow
    } else {
        Color::Green
    };
    let title = label.to_string();
    let label_text = format!("{:.1}%", value);
    Gauge::default()
        .block(Block::default().title(title).borders(Borders::NONE))
        .gauge_style(Style::default().fg(color))
        .percent(pct)
        .label(label_text)
}

pub fn card(title: &str, lines: Vec<Line<'static>>) -> Paragraph<'static> {
    Paragraph::new(lines).block(
        Block::default()
            .title(title.to_string())
            .borders(Borders::ALL),
    )
}

pub fn line_pair(theme: &Theme, label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:<16}", label), theme.label),
        Span::styled(value.to_string(), theme.value),
    ])
}
