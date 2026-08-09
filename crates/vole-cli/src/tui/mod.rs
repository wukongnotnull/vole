//! ratatui 立即模式 TUI。

mod analyze_view;
mod menu_state;
mod paginated_select;
mod status_view;
mod theme;
mod widgets;

pub use analyze_view::render_analyze;
// Re-exported for uninstall TTY wiring (Task 4+) and external unit tests.
#[allow(unused_imports)]
pub use menu_state::{
    EmptyMenuError, MenuConfig, MenuItem, MenuKey, MenuState, SelectOutcome, SortMode,
};
#[allow(unused_imports)]
pub use paginated_select::{drain_pending_input, run_paginated_select};
pub use status_view::render_status;
pub use theme::Theme;
