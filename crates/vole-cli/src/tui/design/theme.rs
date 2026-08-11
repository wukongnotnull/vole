//! Dual-rail color themes (dark / light) and resolution from the environment.

use std::env;

use ratatui::style::{Color, Modifier, Style};

use super::tokens::ENV_THEME;

/// Resolved appearance mode for the TUI design system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Dark,
    Light,
}

/// mole lipgloss-aligned semantic styles, with a light-background rail for readability.
#[derive(Debug, Clone)]
pub struct Theme {
    pub title: Style,
    pub primary: Style,
    pub subtle: Style,
    pub ok: Style,
    pub warn: Style,
    pub danger: Style,
    #[allow(dead_code)] // kept for mole-parity accents; status cards use clean titles
    pub rule: Style,
    pub label: Style,
    pub value: Style,
    pub normal: Style,
    pub selected: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Current mole-parity dark terminal palette.
    pub fn dark() -> Self {
        Self {
            title: Style::default()
                .fg(Color::Rgb(0xC7, 0x9F, 0xD7))
                .add_modifier(Modifier::BOLD),
            primary: Style::default().fg(Color::Rgb(0xBD, 0x93, 0xF9)),
            subtle: Style::default().fg(Color::Rgb(0x73, 0x73, 0x73)),
            ok: Style::default().fg(Color::Rgb(0xA5, 0xD6, 0xA7)),
            warn: Style::default().fg(Color::Rgb(0xFF, 0xD7, 0x5F)),
            danger: Style::default()
                .fg(Color::Rgb(0xFF, 0x5F, 0x5F))
                .add_modifier(Modifier::BOLD),
            rule: Style::default().fg(Color::Rgb(0x40, 0x40, 0x40)),
            label: Style::default().fg(Color::Rgb(0x73, 0x73, 0x73)),
            value: Style::default().fg(Color::White),
            normal: Style::default().fg(Color::White),
            selected: Style::default()
                .fg(Color::Rgb(0x00, 0xD7, 0xFF))
                .add_modifier(Modifier::BOLD),
        }
    }

    /// High-contrast palette for light terminal backgrounds.
    pub fn light() -> Self {
        Self {
            title: Style::default()
                .fg(Color::Rgb(0x6B, 0x3F, 0xA0))
                .add_modifier(Modifier::BOLD),
            primary: Style::default()
                .fg(Color::Rgb(0x5B, 0x2C, 0xB0))
                .add_modifier(Modifier::BOLD),
            subtle: Style::default().fg(Color::Rgb(0x5A, 0x5A, 0x5A)),
            ok: Style::default().fg(Color::Rgb(0x1B, 0x7A, 0x3C)),
            warn: Style::default().fg(Color::Rgb(0xA0, 0x6E, 0x00)),
            danger: Style::default()
                .fg(Color::Rgb(0xC4, 0x28, 0x28))
                .add_modifier(Modifier::BOLD),
            rule: Style::default().fg(Color::Rgb(0xB0, 0xB0, 0xB0)),
            label: Style::default().fg(Color::Rgb(0x4A, 0x4A, 0x4A)),
            value: Style::default().fg(Color::Rgb(0x1A, 0x1A, 0x1A)),
            normal: Style::default().fg(Color::Rgb(0x1A, 0x1A, 0x1A)),
            selected: Style::default()
                .fg(Color::Rgb(0x00, 0x6D, 0xAE))
                .add_modifier(Modifier::BOLD),
        }
    }

    pub fn for_mode(mode: ColorMode) -> Self {
        match mode {
            ColorMode::Dark => Self::dark(),
            ColorMode::Light => Self::light(),
        }
    }

    pub fn style_for_bucket(&self, bucket: ColorBucket) -> Style {
        match bucket {
            ColorBucket::Ok => self.ok,
            ColorBucket::Warn => self.warn,
            ColorBucket::Danger => self.danger,
        }
    }

    #[allow(dead_code)] // available for analyze size coloring refinements
    pub fn style_for_size_tone(&self, tone: SizeTone) -> Style {
        match tone {
            SizeTone::High => self.danger,
            SizeTone::Mid => self.warn,
            SizeTone::Low => Style::default().fg(Color::Rgb(0x5F, 0xAF, 0xFF)),
            SizeTone::Quiet => self.subtle,
        }
    }

    pub fn style_for_health(&self, score: i32) -> Style {
        // Light rail: darker inks. Dark rail: bright mole greens.
        let light = matches!(self.value.fg, Some(Color::Rgb(0x1A, 0x1A, 0x1A)));
        if light {
            if score >= 90 {
                Style::default()
                    .fg(Color::Rgb(0x1B, 0x7A, 0x3C))
                    .add_modifier(Modifier::BOLD)
            } else if score >= 75 {
                Style::default()
                    .fg(Color::Rgb(0x2E, 0x7D, 0x32))
                    .add_modifier(Modifier::BOLD)
            } else if score >= 50 {
                Style::default()
                    .fg(Color::Rgb(0xA0, 0x6E, 0x00))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Rgb(0xC4, 0x28, 0x28))
                    .add_modifier(Modifier::BOLD)
            }
        } else if score >= 90 {
            Style::default()
                .fg(Color::Rgb(0x87, 0xFF, 0x87))
                .add_modifier(Modifier::BOLD)
        } else if score >= 75 {
            Style::default()
                .fg(Color::Rgb(0x87, 0xD7, 0x87))
                .add_modifier(Modifier::BOLD)
        } else if score >= 50 {
            Style::default()
                .fg(Color::Rgb(0xFF, 0xD7, 0x5F))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Rgb(0xFF, 0x6B, 0x6B))
                .add_modifier(Modifier::BOLD)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBucket {
    Ok,
    Warn,
    Danger,
}

/// mole `colorizePercent`：≥85 danger / ≥60 warn / else ok。
pub fn color_bucket(percent: f64) -> ColorBucket {
    if percent >= 85.0 {
        ColorBucket::Danger
    } else if percent >= 60.0 {
        ColorBucket::Warn
    } else {
        ColorBucket::Ok
    }
}

/// analyze size/percent 着色：≥50 red / ≥20 yellow / ≥5 blue / else gray。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SizeTone {
    High,
    Mid,
    Low,
    Quiet,
}

#[allow(dead_code)]
pub fn size_tone(percent: f64) -> SizeTone {
    if percent >= 50.0 {
        SizeTone::High
    } else if percent >= 20.0 {
        SizeTone::Mid
    } else if percent >= 5.0 {
        SizeTone::Low
    } else {
        SizeTone::Quiet
    }
}

/// Parse `VOLE_THEME` / `COLORFGBG` into a color mode. Defaults to dark.
pub fn resolve_color_mode() -> ColorMode {
    resolve_color_mode_from(
        env::var(ENV_THEME).ok().as_deref(),
        env::var("COLORFGBG").ok().as_deref(),
    )
}

pub fn resolve_color_mode_from(vole_theme: Option<&str>, colorfgbg: Option<&str>) -> ColorMode {
    match vole_theme.map(str::trim).map(|s| s.to_ascii_lowercase()) {
        Some(ref s) if s == "light" => return ColorMode::Light,
        Some(ref s) if s == "dark" => return ColorMode::Dark,
        _ => {}
    }
    match colorfgbg_prefers_light(colorfgbg) {
        Some(true) => ColorMode::Light,
        Some(false) => ColorMode::Dark,
        None => ColorMode::Dark,
    }
}

/// `COLORFGBG` is typically `fg;bg` with ANSI color indexes. 7/15 ≈ light bg.
fn colorfgbg_prefers_light(colorfgbg: Option<&str>) -> Option<bool> {
    let raw = colorfgbg?.trim();
    if raw.is_empty() {
        return None;
    }
    let bg = raw.split(';').next_back()?.trim().parse::<u8>().ok()?;
    Some(matches!(bg, 7 | 15))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_theme_uses_dark_ink_not_white() {
        let light = Theme::light();
        assert_ne!(light.value.fg, Some(Color::White));
        assert_ne!(light.normal.fg, Some(Color::White));
        assert_eq!(light.value.fg, Some(Color::Rgb(0x1A, 0x1A, 0x1A)));
    }

    #[test]
    fn dark_theme_keeps_mole_white_values() {
        let dark = Theme::dark();
        assert_eq!(dark.value.fg, Some(Color::White));
    }

    #[test]
    fn resolve_honors_vole_theme_override() {
        assert_eq!(
            resolve_color_mode_from(Some("light"), Some("0;0")),
            ColorMode::Light
        );
        assert_eq!(
            resolve_color_mode_from(Some("dark"), Some("15;15")),
            ColorMode::Dark
        );
    }

    #[test]
    fn resolve_reads_colorfgbg_when_auto() {
        assert_eq!(
            resolve_color_mode_from(None, Some("0;15")),
            ColorMode::Light
        );
        assert_eq!(resolve_color_mode_from(None, Some("15;0")), ColorMode::Dark);
        assert_eq!(resolve_color_mode_from(None, None), ColorMode::Dark);
    }
}
