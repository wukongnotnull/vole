//! ratatui paginated multi-select runner (mole-aligned).

#![allow(dead_code)] // Public API; wired by uninstall interactive path (Task 4).

use std::io::{self, stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::{Frame, Terminal};

use crate::terminal::TerminalGuard;

use super::design::{inset_content, menu_footer_line, DesignSystem, Theme, FOOTER_GAP, TOP_PAD};
use super::menu_state::{MenuConfig, MenuItem, MenuKey, MenuState, SelectOutcome};
use super::widgets::wrap_menu_block;

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
        if let Ok((cols, rows)) = crossterm::terminal::size() {
            state.set_term_size(cols, rows);
        }
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
        KeyCode::Char('b') | KeyCode::Char('B') => Some(MenuKey::Back),
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
    let area = inset_content(frame.area());
    let mut constraints = Vec::new();
    if TOP_PAD > 0 {
        constraints.push(Constraint::Length(TOP_PAD));
    }
    constraints.push(Constraint::Length(2));
    constraints.push(Constraint::Min(1));
    if FOOTER_GAP > 0 {
        constraints.push(Constraint::Length(FOOTER_GAP));
    }
    constraints.push(Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0usize;
    if TOP_PAD > 0 {
        idx += 1;
    }

    let header = if state.filter_text().is_empty() {
        format!("{title}  {}", state.selection_summary())
    } else {
        format!(
            "{title}  / Search: {}_  ({})",
            state.filter_text(),
            state.selection_summary()
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(header)).style(theme.title),
        chunks[idx],
    );
    idx += 1;

    let page = state.visible_page();
    let cursor_row = state.cursor_in_page();
    let list_w = chunks[idx].width as usize;
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
            let label_lines: Vec<&str> = item.label.split('\n').collect();
            let block = wrap_menu_block(mark, &label_lines, &size, list_w);
            let style = if row == cursor_row {
                theme.selected
            } else {
                theme.normal
            };
            ListItem::new(
                block
                    .into_iter()
                    .map(|line| Line::from(Span::styled(line, style)))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::NONE)),
        chunks[idx],
    );
    idx += 1;

    if FOOTER_GAP > 0 {
        idx += 1;
    }

    frame.render_widget(Paragraph::new(menu_footer_line(theme)), chunks[idx]);
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
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)),
            Some(MenuKey::Back)
        ));
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            Some(MenuKey::Up)
        ));
    }
}
