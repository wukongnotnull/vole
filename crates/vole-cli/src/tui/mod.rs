//! ratatui 立即模式 TUI。

mod analyze_view;
mod menu_state;
mod status_view;
mod theme;
mod widgets;

pub use analyze_view::render_analyze;
pub use menu_state::{
    EmptyMenuError, MenuConfig, MenuItem, MenuKey, MenuState, SelectOutcome, SortMode,
};
pub use status_view::render_status;
pub use theme::Theme;
