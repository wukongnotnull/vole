//! Single Vole accent theme: warm mascot-inspired accents + inherited body foreground.

use ratatui::style::{Color, Modifier, Style};

/// Vole accents (warm brown / coral / meadow) + unstyled body ink.
#[derive(Debug, Clone)]
pub struct Theme {
    pub title: Style,
    pub primary: Style,
    pub subtle: Style,
    pub ok: Style,
    pub warn: Style,
    pub danger: Style,
    #[allow(dead_code)] // kept for card/rule accents when needed
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
    /// - Accents: Vole mascot warmth (brown / coral terracotta / meadow green), distinct from Mole purple.
    /// - Body (`label` / `value` / `normal`): no fg — inherit the terminal default
    ///   (black on light themes, light on dark themes).
    pub fn new() -> Self {
        Self {
            title: Style::default()
                .fg(Color::Rgb(0xB8, 0x95, 0x6C))
                .add_modifier(Modifier::BOLD),
            primary: Style::default().fg(Color::Rgb(0xC4, 0x78, 0x5A)),
            subtle: Style::default().fg(Color::Rgb(0x73, 0x73, 0x73)),
            ok: Style::default().fg(Color::Rgb(0x6B, 0xAA, 0x8A)),
            warn: Style::default().fg(Color::Rgb(0xE0, 0xB4, 0x56)),
            danger: Style::default()
                .fg(Color::Rgb(0xE8, 0x5D, 0x5D))
                .add_modifier(Modifier::BOLD),
            rule: Style::default().fg(Color::Rgb(0x40, 0x40, 0x40)),
            bar_track: Style::default().fg(Color::Rgb(0x40, 0x40, 0x40)),
            label: Style::default(),
            value: Style::default(),
            normal: Style::default(),
            selected: Style::default()
                .fg(Color::Rgb(0x3A, 0x9E, 0x8E))
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
            SizeTone::Low => Style::default().fg(Color::Rgb(0x3A, 0x9E, 0x8E)),
            SizeTone::Quiet => self.subtle,
        }
    }

    pub fn style_for_health(&self, score: i32) -> Style {
        if score >= 90 {
            Style::default()
                .fg(Color::Rgb(0x5F, 0xBF, 0x8A))
                .add_modifier(Modifier::BOLD)
        } else if score >= 75 {
            Style::default()
                .fg(Color::Rgb(0x6B, 0xAA, 0x8A))
                .add_modifier(Modifier::BOLD)
        } else if score >= 50 {
            Style::default()
                .fg(Color::Rgb(0xE0, 0xB4, 0x56))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Rgb(0xE8, 0x5D, 0x5D))
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
    fn single_theme_uses_vole_accents_and_inherits_body_fg() {
        let theme = Theme::new();
        assert_eq!(theme.title.fg, Some(Color::Rgb(0xB8, 0x95, 0x6C)));
        assert_eq!(theme.primary.fg, Some(Color::Rgb(0xC4, 0x78, 0x5A)));
        assert_eq!(theme.subtle.fg, Some(Color::Rgb(0x73, 0x73, 0x73)));
        assert_eq!(theme.ok.fg, Some(Color::Rgb(0x6B, 0xAA, 0x8A)));
        assert_eq!(theme.warn.fg, Some(Color::Rgb(0xE0, 0xB4, 0x56)));
        assert_eq!(theme.danger.fg, Some(Color::Rgb(0xE8, 0x5D, 0x5D)));
        assert_eq!(theme.selected.fg, Some(Color::Rgb(0x3A, 0x9E, 0x8E)));
        assert_eq!(theme.label.fg, None);
        assert_eq!(theme.value.fg, None);
        assert_eq!(theme.normal.fg, None);
    }

    #[test]
    fn health_styles_use_vole_meadow_scale() {
        let theme = Theme::new();
        assert_eq!(
            theme.style_for_health(95).fg,
            Some(Color::Rgb(0x5F, 0xBF, 0x8A))
        );
        assert_eq!(
            theme.style_for_health(40).fg,
            Some(Color::Rgb(0xE8, 0x5D, 0x5D))
        );
    }
}
