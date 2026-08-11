//! ratatui bare-vole home menu (mole interactive_main_menu shell).

#![allow(dead_code)] // Public API; wired by interactive.rs.

use std::io::{self, stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use crate::terminal::TerminalGuard;

use super::design::{
    home_controls_line, inset_content, DesignSystem, Theme, FOOTER_GAP, TOP_PAD,
};
use super::home_menu_state::{HomeAction, HomeKey, HomeMenuConfig, HomeMenuState, HOME_ITEMS};
use super::paginated_select::drain_pending_input;

pub const VOLE_TAGLINE: &str = "Deep clean and optimize your Mac.";
pub const VOLE_REPO_URL: &str = "https://github.com/wukongnotnull/vole";

pub struct HomeMenuRunOpts {
    pub cfg: HomeMenuConfig,
    pub update_message: Option<String>,
}

pub fn brand_ascii_lines() -> [&'static str; 5] {
    [
        r"__     __    _",
        r"\ \   / /___| | ___",
        r" \ \ / / _ \ |/ _ \",
        r"  \ V / (_) | |  __/",
        r"   \_/ \___/|_|\___|",
    ]
}

pub fn map_key(key: KeyEvent) -> Option<HomeKey> {
    match key.code {
        KeyCode::Up => Some(HomeKey::Up),
        KeyCode::Down => Some(HomeKey::Down),
        KeyCode::Enter => Some(HomeKey::Enter),
        KeyCode::Esc => Some(HomeKey::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(HomeKey::Quit),
        KeyCode::Char(c) => match c {
            '1'..='5' => Some(HomeKey::Digit(c as u8 - b'0')),
            'm' | 'M' => Some(HomeKey::More),
            'v' | 'V' => Some(HomeKey::Version),
            't' | 'T' => Some(HomeKey::TouchId),
            'u' | 'U' => Some(HomeKey::Update),
            'q' | 'Q' => Some(HomeKey::Quit),
            _ => None,
        },
        _ => None,
    }
}

pub fn run_home_menu(opts: HomeMenuRunOpts) -> io::Result<HomeAction> {
    let mut guard = TerminalGuard::enter()?;
    drain_pending_input(Duration::from_millis(200));
    let mut term = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut state = HomeMenuState::new(opts.cfg);
    let theme = DesignSystem::resolve().theme;
    let update_msg = opts.update_message.as_deref();
    loop {
        term.draw(|f| render_home(f, &state, &theme, update_msg))?;
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let Some(hk) = map_key(key) else {
            continue;
        };
        if let Some(action) = state.handle_key(hk) {
            guard.restore();
            drain_pending_input(Duration::from_millis(100));
            return Ok(action);
        }
    }
}

fn render_home(
    frame: &mut Frame,
    state: &HomeMenuState,
    theme: &Theme,
    update_message: Option<&str>,
) {
    let area = inset_content(frame.area());
    let show_update = update_message.is_some_and(|s| !s.trim().is_empty());
    // top + brand(5) + blank + [update + blank] + items(5) + footer_gap + footer(1) + sink
    let brand_h = 5u16;
    let update_h = if show_update { 2u16 } else { 0 };
    let items_h = 5u16;
    let footer_h = 1u16;

    let mut constraints = Vec::new();
    if TOP_PAD > 0 {
        constraints.push(Constraint::Length(TOP_PAD));
    }
    constraints.push(Constraint::Length(brand_h));
    constraints.push(Constraint::Length(1));
    if update_h > 0 {
        constraints.push(Constraint::Length(update_h));
    }
    constraints.push(Constraint::Length(items_h));
    if FOOTER_GAP > 0 {
        constraints.push(Constraint::Length(FOOTER_GAP));
    }
    constraints.push(Constraint::Length(footer_h));
    constraints.push(Constraint::Min(0));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0usize;
    if TOP_PAD > 0 {
        idx += 1;
    }

    let ascii = brand_ascii_lines();
    let brand_lines: Vec<Line> = ascii
        .iter()
        .enumerate()
        .map(|(i, line)| match i {
            3 => Line::from(vec![
                Span::styled(*line, theme.ok),
                Span::raw("  "),
                Span::styled(VOLE_REPO_URL, theme.primary),
            ]),
            4 => Line::from(vec![
                Span::styled(*line, theme.ok),
                Span::raw("  "),
                Span::styled(VOLE_TAGLINE, theme.ok),
            ]),
            _ => Line::from(Span::styled(*line, theme.ok)),
        })
        .collect();
    frame.render_widget(Paragraph::new(brand_lines), chunks[idx]);
    idx += 1;
    idx += 1; // blank under brand

    if update_h > 0 {
        let msg = update_message.unwrap_or("Update available");
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(msg, theme.warn))),
            chunks[idx],
        );
        idx += 1;
    }

    let cursor = state.cursor();
    let item_lines: Vec<Line> = HOME_ITEMS
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let marker = if i == cursor { "> " } else { "  " };
            let title = format!("{:<12}", item.title);
            let style = if i == cursor {
                theme.selected
            } else {
                theme.normal
            };
            Line::from(Span::styled(
                format!("{marker}{title}{desc}", desc = item.description),
                style,
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(item_lines), chunks[idx]);
    idx += 1;

    if FOOTER_GAP > 0 {
        idx += 1;
    }

    frame.render_widget(
        Paragraph::new(home_controls_line(
            theme,
            state.footer_shows_touchid(),
            state.footer_shows_update(),
        )),
        chunks[idx],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn brand_constants() {
        assert_eq!(VOLE_REPO_URL, "https://github.com/wukongnotnull/vole");
        assert_eq!(VOLE_TAGLINE, "Deep clean and optimize your Mac.");
        let lines = brand_ascii_lines();
        assert!(lines[0].contains("__"));
    }

    #[test]
    fn map_key_basics() {
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            Some(HomeKey::Up)
        ));
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(HomeKey::Quit)
        ));
    }
}
