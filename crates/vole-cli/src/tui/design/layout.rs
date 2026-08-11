//! Shared geometry helpers for TUI surfaces.

use ratatui::layout::Rect;

use super::tokens::OUTER_PAD;

/// Inset the draw area horizontally; shrinks/zeros the pad on very narrow terminals.
pub fn inset_horizontal(area: Rect, pad: u16) -> Rect {
    let pad = pad.min(area.width / 4);
    if pad == 0 || area.width <= pad * 2 {
        return area;
    }
    Rect {
        x: area.x.saturating_add(pad),
        y: area.y,
        width: area.width.saturating_sub(pad * 2),
        height: area.height,
    }
}

/// Apply the design-system outer horizontal pad.
pub fn inset_content(area: Rect) -> Rect {
    inset_horizontal(area, OUTER_PAD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_content_pads_both_sides() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let inset = inset_content(area);
        assert_eq!(inset.x, OUTER_PAD);
        assert_eq!(inset.width, 80 - OUTER_PAD * 2);
    }
}
