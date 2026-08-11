//! Vole TUI design system — tokens, universal theme, shared components.

mod components;
mod layout;
mod theme;
mod tokens;

pub use components::{
    card_title, home_controls_line, menu_footer_line, status_footer, status_footer_line,
};
#[allow(unused_imports)] // public design-system surface
pub use layout::{inset_content, inset_horizontal};
#[allow(unused_imports)] // public design-system surface
pub use theme::{color_bucket, size_tone, ColorBucket, SizeTone, Theme};
#[allow(unused_imports)] // public design-system surface
pub use tokens::{CARD_ROW_GAP, COL_GUTTER, ENV_THEME, FOOTER_GAP, OUTER_PAD, TOP_PAD};

/// Resolved design system for a TUI session (theme + layout tokens).
#[derive(Debug, Clone)]
pub struct DesignSystem {
    pub theme: Theme,
}

impl DesignSystem {
    /// Always the Mole-aligned palette.
    ///
    /// `VOLE_THEME` / `COLORFGBG` are ignored (kept as a documented no-op via [`ENV_THEME`]).
    pub fn resolve() -> Self {
        Self {
            theme: Theme::universal(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_mole_palette() {
        let ds = DesignSystem::resolve();
        assert_eq!(ds.theme.value.fg, None);
        assert_eq!(ds.theme.label.fg, None);
        assert_eq!(
            ds.theme.primary.fg,
            Some(ratatui::style::Color::Rgb(0xBD, 0x93, 0xF9))
        );
    }
}
