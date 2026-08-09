//! ratatui 主题（mole `cmd/status` / `cmd/analyze` 色板映射）。

use ratatui::style::{Color, Modifier, Style};

/// mole lipgloss：title `#C79FD7`、primary `#BD93F9`、subtle `#737373`、
/// warn `#FFD75F`、danger `#FF5F5F`、ok `#A5D6A7`、rule `#404040`。
#[derive(Debug, Clone)]
pub struct Theme {
    pub title: Style,
    #[allow(dead_code)] // reserved for hardware identity accents (mole primary)
    pub primary: Style,
    pub subtle: Style,
    pub ok: Style,
    pub warn: Style,
    pub danger: Style,
    pub rule: Style,
    pub label: Style,
    pub value: Style,
    pub normal: Style,
    pub selected: Style,
}

impl Default for Theme {
    fn default() -> Self {
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

impl Theme {
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
