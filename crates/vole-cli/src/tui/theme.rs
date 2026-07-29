//! ratatui 主题。

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    pub title: Style,
    #[allow(dead_code)]
    pub ok: Style,
    #[allow(dead_code)]
    pub warn: Style,
    #[allow(dead_code)]
    pub danger: Style,
    pub label: Style,
    pub value: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            title: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            ok: Style::default().fg(Color::Green),
            warn: Style::default().fg(Color::Yellow),
            danger: Style::default().fg(Color::Red),
            label: Style::default().fg(Color::DarkGray),
            value: Style::default().fg(Color::White),
        }
    }
}
