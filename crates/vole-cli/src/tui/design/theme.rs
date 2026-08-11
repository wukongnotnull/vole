//! Single Mole-accent theme: body text inherits the terminal default foreground.

use ratatui::style::{Color, Modifier, Style};

/// Mole-parity accents (`cmd/status/view.go`) + unstyled body ink.
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
    /// Legacy track style; status bars colorize the whole bar like mole.
    #[allow(dead_code)]
    pub bar_track: Style,
    pub label: Style,
    pub value: Style,
    pub normal: Style,
    pub selected: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

impl Theme {
    /// One palette for all terminals.
    ///
    /// - Accents: Mole dark-terminal colors (purple / green / amber / red).
    /// - Body (`label` / `value` / `normal`): no fg — inherit the terminal default
    ///   (black on light themes, light on dark themes), same as Mole's unstyled values.
    pub fn new() -> Self {
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
            bar_track: Style::default().fg(Color::Rgb(0x40, 0x40, 0x40)),
            label: Style::default(),
            value: Style::default(),
            normal: Style::default(),
            selected: Style::default()
                .fg(Color::Rgb(0x00, 0xD7, 0xFF))
                .add_modifier(Modifier::BOLD),
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

    /// mole `getScoreStyle` thresholds / colors.
    pub fn style_for_health(&self, score: i32) -> Style {
        if score >= 90 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_theme_uses_mole_accents_and_inherits_body_fg() {
        let theme = Theme::new();
        assert_eq!(theme.title.fg, Some(Color::Rgb(0xC7, 0x9F, 0xD7)));
        assert_eq!(theme.primary.fg, Some(Color::Rgb(0xBD, 0x93, 0xF9)));
        assert_eq!(theme.subtle.fg, Some(Color::Rgb(0x73, 0x73, 0x73)));
        assert_eq!(theme.ok.fg, Some(Color::Rgb(0xA5, 0xD6, 0xA7)));
        assert_eq!(theme.warn.fg, Some(Color::Rgb(0xFF, 0xD7, 0x5F)));
        assert_eq!(theme.danger.fg, Some(Color::Rgb(0xFF, 0x5F, 0x5F)));
        assert_eq!(theme.label.fg, None);
        assert_eq!(theme.value.fg, None);
        assert_eq!(theme.normal.fg, None);
    }

    #[test]
    fn health_styles_match_mole_get_score_style() {
        let theme = Theme::new();
        assert_eq!(
            theme.style_for_health(95).fg,
            Some(Color::Rgb(0x87, 0xFF, 0x87))
        );
        assert_eq!(
            theme.style_for_health(40).fg,
            Some(Color::Rgb(0xFF, 0x6B, 0x6B))
        );
    }
}
