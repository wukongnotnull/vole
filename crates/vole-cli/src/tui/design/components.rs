//! Shared styled fragments used across TUI surfaces.

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use super::theme::Theme;

fn key_style(theme: &Theme) -> ratatui::style::Style {
    theme.primary.add_modifier(Modifier::BOLD)
}

fn footer_sep(theme: &Theme) -> Span<'static> {
    Span::styled(" | ", theme.subtle)
}

/// Key glyph + optional label, both themed for light/dark rails.
pub fn key_hint(theme: &Theme, key: &str, label: &str) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(key.to_string(), key_style(theme))];
    if !label.is_empty() {
        spans.push(Span::styled(format!(" {label}"), theme.value));
    }
    spans
}

/// Plain status key-hint string (tests / honesty checks).
#[allow(dead_code)] // re-exported via widgets; used by unit tests
pub fn status_footer() -> String {
    "K Vole | C Cores | B Back | Q/Esc/Ctrl+C Quit".to_string()
}

/// Key letters in primary+bold; action labels in theme value ink.
pub fn status_footer_line(theme: &Theme) -> Line<'static> {
    let key = key_style(theme);
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

/// Home menu controls, matching `HomeMenuState::controls_line` copy.
pub fn home_controls_line(theme: &Theme, show_touchid: bool, show_update: bool) -> Line<'static> {
    let mut spans = Vec::new();
    spans.extend(key_hint(theme, "↑↓", ""));
    spans.push(footer_sep(theme));
    spans.extend(key_hint(theme, "Enter", ""));
    spans.push(footer_sep(theme));
    spans.extend(key_hint(theme, "M", "More"));
    spans.push(footer_sep(theme));
    spans.extend(key_hint(theme, "V", "Version"));
    if show_touchid {
        spans.push(footer_sep(theme));
        spans.extend(key_hint(theme, "T", "TouchID"));
    } else if show_update {
        spans.push(footer_sep(theme));
        spans.extend(key_hint(theme, "U", "Update"));
    }
    spans.push(footer_sep(theme));
    spans.extend(key_hint(theme, "Q", "Quit"));
    Line::from(spans)
}

/// Paginated multi-select footer.
pub fn menu_footer_line(theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    spans.extend(key_hint(theme, "Space", "Select"));
    spans.push(footer_sep(theme));
    spans.extend(key_hint(theme, "Enter", "Confirm"));
    spans.push(footer_sep(theme));
    spans.extend(key_hint(theme, "Q", "Cancel"));
    Line::from(spans)
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

    #[test]
    fn home_controls_include_optional_touchid() {
        let theme = Theme::dark();
        let with_t = home_controls_line(&theme, true, false);
        let text: String = with_t.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("T"));
        assert!(text.contains("TouchID"));
        assert!(!text.contains("Update"));
        let with_u = home_controls_line(&theme, false, true);
        let text: String = with_u.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("U"));
        assert!(text.contains("Update"));
    }
}
