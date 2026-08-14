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
use super::home_menu_state::{
    format_home_item_line, HomeAction, HomeKey, HomeMenuConfig, HomeMenuState, HOME_ITEMS,
};
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

pub fn brand_version_label() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// Brand ASCII rows; version sits at the bottom-right of the last line.
pub fn brand_display_lines(version: &str) -> [String; 5] {
    let ascii = brand_ascii_lines();
    [
        ascii[0].to_string(),
        ascii[1].to_string(),
        ascii[2].to_string(),
        ascii[3].to_string(),
        format!("{}  {version}", ascii[4]),
    ]
}

fn brand_left_cols() -> u16 {
    let last = brand_display_lines(&brand_version_label())[4]
        .chars()
        .count() as u16;
    last.max(BRAND_ASCII_COLS)
}

pub fn map_key(key: KeyEvent) -> Option<HomeKey> {
    match key.code {
        KeyCode::Up => Some(HomeKey::Up),
        KeyCode::Down => Some(HomeKey::Down),
        KeyCode::Enter => Some(HomeKey::Enter),
        KeyCode::Esc => Some(HomeKey::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(HomeKey::Quit),
        KeyCode::Char(c) => match c {
            '1'..='6' => Some(HomeKey::Digit(c as u8 - b'0')),
            'h' | 'H' => Some(HomeKey::Help),
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
    total.saturating_sub(brand_left_cols().saturating_add(BRAND_GUTTER))
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
    // top + brand(5) + meta(2) + blank + [update] + items + footer_gap + footer(1) + sink
    let update_h = if show_update { 2u16 } else { 0 };
    let items_h = HOME_ITEMS.len() as u16;
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
            let style = if i == cursor {
                theme.selected
            } else {
                theme.normal
            };
            Line::from(Span::styled(
                format_home_item_line(i, i == cursor, item),
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
    let version = brand_version_label();
    let display = brand_display_lines(&version);
    let brand_lines: Vec<Line> = display
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if i == 4 {
                let ascii = brand_ascii_lines()[4];
                Line::from(vec![
                    Span::styled(ascii.to_string(), theme.ok),
                    Span::styled(format!("  {version}"), theme.subtle),
                ])
            } else {
                Line::from(Span::styled(line.clone(), theme.ok))
            }
        })
        .collect();

    if !show_vole {
        frame.render_widget(Paragraph::new(brand_lines), area);
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(brand_left_cols()),
            Constraint::Length(BRAND_GUTTER),
            Constraint::Min(VOLE_SPRITE_COLS),
        ])
        .split(area);

    frame.render_widget(Paragraph::new(brand_lines), cols[0]);
    // cols[1] = gutter — vertically center the 4-line vole in the 5-line brand row.
    let vole_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top pad → share midline with VOLE ascii
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .split(cols[2]);
    let vole = render_mole_frame(anim_frame, vole_rows[1].width as usize);
    frame.render_widget(Paragraph::new(vole).style(theme.ok), vole_rows[1]);
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
    fn brand_version_sits_at_bottom_right_of_ascii() {
        let version = brand_version_label();
        assert_eq!(version, format!("v{}", env!("CARGO_PKG_VERSION")));
        let ascii = brand_ascii_lines();
        let lines = brand_display_lines(&version);
        let last = ascii.len() - 1;
        for (i, row) in ascii.iter().enumerate().take(last) {
            assert_eq!(lines[i], *row);
        }
        assert!(lines[last].starts_with(ascii[last]), "{}", lines[last]);
        assert!(lines[last].ends_with(&version), "{}", lines[last]);
        assert!(
            lines[last].contains(&format!("{}  {version}", ascii[last])),
            "{}",
            lines[last]
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
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Char('6'), KeyModifiers::NONE)),
            Some(HomeKey::Digit(6))
        ));
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE)),
            Some(HomeKey::Digit(1))
        ));
        assert!(matches!(
            map_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)),
            Some(HomeKey::Help)
        ));
        assert!(
            map_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE)).is_none(),
            "M More was replaced by H Helper"
        );
    }

    #[test]
    fn right_column_hides_vole_when_too_narrow() {
        assert_eq!(right_column_width(brand_left_cols() + BRAND_GUTTER), 0);
        assert!(
            right_column_width(brand_left_cols() + BRAND_GUTTER + VOLE_SPRITE_COLS)
                >= VOLE_SPRITE_COLS
        );
    }
}
