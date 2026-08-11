//! ratatui bare-vole home menu (mole interactive_main_menu shell).

#![allow(dead_code)] // Public API; wired by interactive.rs.

use std::io::{self, stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use crate::terminal::TerminalGuard;

use super::design::{home_controls_line, inset_content, DesignSystem, Theme, FOOTER_GAP, TOP_PAD};
use super::home_menu_state::{HomeAction, HomeKey, HomeMenuConfig, HomeMenuState, HOME_ITEMS};
use super::paginated_select::drain_pending_input;
use super::status_cat::render_mole_frame;

pub const VOLE_TAGLINE: &str = "Deep clean and optimize your Mac.";
pub const VOLE_REPO_URL: &str = "https://github.com/wukongnotnull/vole";

/// Widest `brand_ascii_lines` glyph width (+ gutter before the vole column).
const BRAND_ASCII_COLS: u16 = 22;
const BRAND_GUTTER: u16 = 2;
/// Sprite width from `status_cat` (must fit in the right column to show).
const VOLE_SPRITE_COLS: u16 = 12;
const BRAND_ROW_H: u16 = 5;
const META_H: u16 = 2; // repo URL + tagline

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
    let mut anim_frame: u64 = 0;
    loop {
        term.draw(|f| render_home(f, &state, &theme, update_msg, anim_frame))?;
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(hk) = map_key(key) {
                        if let Some(action) = state.handle_key(hk) {
                            guard.restore();
                            drain_pending_input(Duration::from_millis(100));
                            return Ok(action);
                        }
                    }
                }
            }
        }
        anim_frame = anim_frame.wrapping_add(1);
    }
}

fn right_column_width(total: u16) -> u16 {
    total.saturating_sub(BRAND_ASCII_COLS.saturating_add(BRAND_GUTTER))
}

fn render_home(
    frame: &mut Frame,
    state: &HomeMenuState,
    theme: &Theme,
    update_message: Option<&str>,
    anim_frame: u64,
) {
    let area = inset_content(frame.area());
    let show_update = update_message.is_some_and(|s| !s.trim().is_empty());
    let right_w = right_column_width(area.width);
    let show_vole = right_w >= VOLE_SPRITE_COLS;
    // top + brand(5) + meta(2) + blank + [update] + items(5) + footer_gap + footer(1) + sink
    let update_h = if show_update { 2u16 } else { 0 };
    let items_h = 5u16;
    let footer_h = 1u16;

    let mut constraints = Vec::new();
    if TOP_PAD > 0 {
        constraints.push(Constraint::Length(TOP_PAD));
    }
    constraints.push(Constraint::Length(BRAND_ROW_H));
    constraints.push(Constraint::Length(META_H));
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

    render_brand_row(frame, chunks[idx], theme, anim_frame, show_vole);
    idx += 1;

    let meta_lines = vec![
        Line::from(Span::styled(VOLE_REPO_URL, theme.primary)),
        Line::from(Span::styled(VOLE_TAGLINE, theme.ok)),
    ];
    frame.render_widget(Paragraph::new(meta_lines), chunks[idx]);
    idx += 1;
    idx += 1; // blank under meta

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

fn render_brand_row(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    anim_frame: u64,
    show_vole: bool,
) {
    let ascii = brand_ascii_lines();
    let brand_lines: Vec<Line> = ascii
        .iter()
        .map(|line| Line::from(Span::styled(*line, theme.ok)))
        .collect();

    if !show_vole {
        frame.render_widget(Paragraph::new(brand_lines), area);
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(BRAND_ASCII_COLS),
            Constraint::Length(BRAND_GUTTER),
            Constraint::Min(VOLE_SPRITE_COLS),
        ])
        .split(area);

    frame.render_widget(Paragraph::new(brand_lines), cols[0]);
    // cols[1] = gutter
    let vole = render_mole_frame(anim_frame, cols[2].width as usize);
    frame.render_widget(Paragraph::new(vole).style(theme.ok), cols[2]);
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
        let max_w = lines.iter().map(|l| l.chars().count()).max().unwrap();
        assert!(
            max_w <= BRAND_ASCII_COLS as usize,
            "ascii width {max_w} exceeds BRAND_ASCII_COLS"
        );
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

    #[test]
    fn right_column_hides_vole_when_too_narrow() {
        assert_eq!(right_column_width(BRAND_ASCII_COLS + BRAND_GUTTER), 0);
        assert!(
            right_column_width(BRAND_ASCII_COLS + BRAND_GUTTER + VOLE_SPRITE_COLS)
                >= VOLE_SPRITE_COLS
        );
    }
}
