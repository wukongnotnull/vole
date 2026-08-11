//! Single mid-contrast theme readable on both dark and light terminal backgrounds.

use ratatui::style::{Color, Modifier, Style};

/// Semantic styles tuned for dual-background readability (no pure white / near-black ink).
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
    /// Empty progress-bar track (`░` / `▯`) — separate from fill color for contrast.
    pub bar_track: Style,
    pub label: Style,
    pub value: Style,
    pub normal: Style,
    pub selected: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::universal()
    }
}

impl Theme {
    /// Mid-contrast palette: aims for readable contrast on both black and white terminal BGs.
    ///
    /// Body ink sits near `#767676` (classic dual-BG gray). Accents use saturated mid-tones
    /// rather than neon-on-dark or ink-on-light rails.
    pub fn universal() -> Self {
        Self {
            title: Style::default()
                .fg(Color::Rgb(0x6B, 0x4F, 0x9A))
                .add_modifier(Modifier::BOLD),
            primary: Style::default()
                .fg(Color::Rgb(0x5B, 0x45, 0x8C))
                .add_modifier(Modifier::BOLD),
            subtle: Style::default().fg(Color::Rgb(0x76, 0x76, 0x76)),
            ok: Style::default().fg(Color::Rgb(0x2E, 0x7D, 0x4F)),
            warn: Style::default().fg(Color::Rgb(0xA6, 0x7C, 0x00)),
            danger: Style::default()
                .fg(Color::Rgb(0xB3, 0x3A, 0x3A))
                .add_modifier(Modifier::BOLD),
            rule: Style::default().fg(Color::Rgb(0x8A, 0x8A, 0x8A)),
            bar_track: Style::default().fg(Color::Rgb(0x8A, 0x8A, 0x8A)),
            label: Style::default().fg(Color::Rgb(0x5A, 0x5A, 0x5A)),
            value: Style::default().fg(Color::Rgb(0x4A, 0x4A, 0x4A)),
            normal: Style::default().fg(Color::Rgb(0x4A, 0x4A, 0x4A)),
            selected: Style::default()
                .fg(Color::Rgb(0x00, 0x6D, 0x8F))
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
            SizeTone::Low => Style::default().fg(Color::Rgb(0x00, 0x6D, 0x8F)),
            SizeTone::Quiet => self.subtle,
        }
    }

    pub fn style_for_health(&self, score: i32) -> Style {
        if score >= 90 {
            Style::default()
                .fg(Color::Rgb(0x2E, 0x7D, 0x4F))
                .add_modifier(Modifier::BOLD)
        } else if score >= 75 {
            Style::default()
                .fg(Color::Rgb(0x3D, 0x8B, 0x5F))
                .add_modifier(Modifier::BOLD)
        } else if score >= 50 {
            Style::default()
                .fg(Color::Rgb(0xA6, 0x7C, 0x00))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Rgb(0xB3, 0x3A, 0x3A))
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
    fn universal_avoids_white_and_near_black_ink() {
        let theme = Theme::universal();
        assert_ne!(theme.value.fg, Some(Color::White));
        assert_ne!(theme.normal.fg, Some(Color::Black));
        assert_eq!(theme.value.fg, Some(Color::Rgb(0x4A, 0x4A, 0x4A)));
        assert_eq!(theme.subtle.fg, Some(Color::Rgb(0x76, 0x76, 0x76)));
        assert_eq!(theme.bar_track.fg, Some(Color::Rgb(0x8A, 0x8A, 0x8A)));
    }

    #[test]
    fn health_styles_use_mid_contrast_greens() {
        let theme = Theme::universal();
        assert_eq!(
            theme.style_for_health(95).fg,
            Some(Color::Rgb(0x2E, 0x7D, 0x4F))
        );
        assert_eq!(
            theme.style_for_health(40).fg,
            Some(Color::Rgb(0xB3, 0x3A, 0x3A))
        );
    }
}
