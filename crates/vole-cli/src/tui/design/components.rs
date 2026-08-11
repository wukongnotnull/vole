//! Shared styled fragments used across TUI surfaces.

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use super::theme::Theme;

/// Plain status key-hint string (tests / honesty checks).
#[allow(dead_code)] // re-exported via widgets; used by unit tests
pub fn status_footer() -> String {
    "K Vole | C Cores | B Back | Q/Esc/Ctrl+C Quit".to_string()
}

/// Key letters in primary+bold; action labels in theme value ink.
pub fn status_footer_line(theme: &Theme) -> Line<'static> {
    let key = theme.primary.add_modifier(Modifier::BOLD);
    let sep = theme.subtle;
    let text = theme.value;
    Line::from(vec![
        Span::styled("K", key),
        Span::styled(" Vole", text),
        Span::styled(" | ", sep),
        Span::styled("C", key),
        Span::styled(" Cores", text),
        Span::styled(" | ", sep),
        Span::styled("B", key),
        Span::styled(" Back", text),
        Span::styled(" | ", sep),
        Span::styled("Q", key),
        Span::styled("/", sep),
        Span::styled("Esc", key),
        Span::styled("/", sep),
        Span::styled("Ctrl+C", key),
        Span::styled(" Quit", text),
    ])
}

pub fn card_title(icon: &str, title: &str, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(format!("{icon} {title}"), theme.title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_footer_line_matches_plain_text() {
        let theme = Theme::light();
        let line = status_footer_line(&theme);
        let joined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(joined, status_footer());
    }
}
