//! Shared spacing and layout tokens for all TUI surfaces.

/// Outer left/right inset so content is not flush against the terminal edge.
pub const OUTER_PAD: u16 = 1;
/// Blank line above the status header so it is not flush against the top edge.
pub const TOP_PAD: u16 = 1;
/// Blank line between card rows (TUI stand-in for line spacing).
pub const CARD_ROW_GAP: u16 = 1;
/// Horizontal gutter between two status columns.
pub const COL_GUTTER: u16 = 2;
/// Blank line between the card block and the key-hint footer.
pub const FOOTER_GAP: u16 = 1;

/// Legacy env name; ignored (TUI uses a single universal palette).
#[allow(dead_code)]
pub const ENV_THEME: &str = "VOLE_THEME";
