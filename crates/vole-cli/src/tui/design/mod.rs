//! Vole TUI design system — tokens, dual-rail themes, shared components.

mod components;
mod theme;
mod tokens;

pub use components::{card_title, status_footer, status_footer_line};
#[allow(unused_imports)] // public design-system surface
pub use theme::{
    color_bucket, resolve_color_mode, size_tone, ColorBucket, ColorMode, SizeTone, Theme,
};
#[allow(unused_imports)] // public design-system surface
pub use tokens::{CARD_ROW_GAP, COL_GUTTER, ENV_THEME, FOOTER_GAP, OUTER_PAD};

/// Resolved design system for a TUI session (theme + mode + layout tokens).
#[derive(Debug, Clone)]
pub struct DesignSystem {
    #[allow(dead_code)] // available for mode-aware widgets / prefs UI
    pub mode: ColorMode,
    pub theme: Theme,
}

impl DesignSystem {
    pub fn resolve() -> Self {
        let mode = resolve_color_mode();
        Self::for_mode(mode)
    }

    pub fn for_mode(mode: ColorMode) -> Self {
        Self {
            mode,
            theme: Theme::for_mode(mode),
        }
    }

    #[allow(dead_code)] // explicit rails for tests / future prefs UI
    pub fn dark() -> Self {
        Self::for_mode(ColorMode::Dark)
    }

    #[allow(dead_code)] // explicit rails for tests / future prefs UI
    pub fn light() -> Self {
        Self::for_mode(ColorMode::Light)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_matching_theme_rail() {
        let light = DesignSystem::light();
        assert_eq!(light.mode, ColorMode::Light);
        assert_eq!(light.theme.value.fg, Some(ratatui::style::Color::Rgb(0x12, 0x12, 0x12)));
        let dark = DesignSystem::dark();
        assert_eq!(dark.mode, ColorMode::Dark);
        assert_eq!(dark.theme.value.fg, Some(ratatui::style::Color::White));
    }
}
