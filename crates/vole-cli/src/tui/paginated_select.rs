//! ratatui paginated multi-select runner (mole-aligned).

#![allow(dead_code)] // Public API; wired by uninstall interactive path (Task 4).

use std::io::{self, stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::{Frame, Terminal};

use crate::terminal::TerminalGuard;

use super::menu_state::{MenuConfig, MenuItem, MenuKey, MenuState, SelectOutcome};
use super::design::{DesignSystem, Theme};

/// Drain pending keyboard input until `timeout` elapses (mole #726).
pub fn drain_pending_input(timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !event::poll(Duration::from_millis(10)).unwrap_or(false) {
            continue;
        }
        let _ = event::read();
    }
}

pub fn run_paginated_select(
    title: &str,
    items: Vec<MenuItem>,
    cfg: MenuConfig,
) -> io::Result<SelectOutcome> {
    let mut guard = TerminalGuard::enter()?;
    drain_pending_input(Duration::from_millis(200));
    let mut term = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut state = MenuState::new(items, cfg).map_err(|e| io::Error::other(e.to_string()))?;
    let theme = DesignSystem::resolve().theme;
    loop {
        term.draw(|f| render_menu(f, title, &state, &theme))?;
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let Some(mk) = map_key(key) else {
            continue;
        };
        if let Some(out) = state.handle_key(mk) {
            guard.restore();
            return Ok(out);
        }
    }
}

fn map_key(key: KeyEvent) -> Option<MenuKey> {
    match key.code {
        KeyCode::Up => Some(MenuKey::Up),
        KeyCode::Down => Some(MenuKey::Down),
        KeyCode::Enter => Some(MenuKey::Enter),
        KeyCode::Esc => Some(MenuKey::Quit),
        KeyCode::Backspace => Some(MenuKey::Backspace),
        KeyCode::Char(' ') => Some(MenuKey::Space),
        KeyCode::Char('q') | KeyCode::Char('Q') => Some(MenuKey::Quit),
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                None
            } else {
                Some(MenuKey::Char(c))
            }
        }
        _ => None,
    }
}

fn render_menu(frame: &mut Frame, title: &str, state: &MenuState, theme: &Theme) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    let header = if state.filter_text().is_empty() {
        format!(
            "{title}  {}/{} selected",
            state.selected_count(),
            state.items().len()
        )
    } else {
        format!(
            "{title}  / Search: {}_  ({}/{})",
            state.filter_text(),
            state.view_len(),
            state.items().len()
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(header)).style(theme.title),
        chunks[0],
    );

    let page = state.visible_page();
    let cursor_row = state.cursor_in_page();
    let items: Vec<ListItem> = page
        .iter()
        .enumerate()
        .map(|(row, &orig)| {
            let item = &state.items()[orig];
            let mark = if state.is_selected(orig) {
                "[x]"
            } else {
                "[ ]"
            };
            let size = item
                .size_kb
                .map(|kb| format!("  ({kb} KB)"))
                .unwrap_or_default();
            let content = format!("{mark} {}{size}", item.label);
            let style = if row == cursor_row {
                theme.selected
            } else {
                theme.normal
            };
            ListItem::new(Span::styled(content, style))
        })
        .collect();

    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::NONE)),
        chunks[1],
    );

    let footer = Line::from(vec![
        Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" Select | "),
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" Confirm | "),
        Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" Cancel"),
    ]);
    frame.render_widget(Paragraph::new(footer).style(theme.label), chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_crossterm_key_space_enter_quit() {
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Some(MenuKey::Space)
        ));
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(MenuKey::Enter)
        ));
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(MenuKey::Quit)
        ));
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(MenuKey::Quit)
        ));
    }
}
