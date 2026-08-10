//! ratatui 立即模式 TUI。

mod analyze_state;
mod analyze_view;
mod home_menu;
mod home_menu_state;
mod menu_state;
mod paginated_select;
mod status_view;
mod theme;
mod widgets;

pub use analyze_view::render_analyze;
#[allow(unused_imports)]
pub use home_menu::{
    brand_ascii_lines, map_key, run_home_menu, HomeMenuRunOpts, VOLE_REPO_URL, VOLE_TAGLINE,
};
#[allow(unused_imports)]
pub use home_menu_state::{
    HomeAction, HomeCommand, HomeItem, HomeKey, HomeMenuConfig, HomeMenuState, HOME_ITEMS,
};
// Re-exported for uninstall / whitelist TTY wiring and external unit tests.
#[allow(unused_imports)]
pub use menu_state::{
    EmptyMenuError, MenuConfig, MenuItem, MenuKey, MenuState, SelectOutcome, SortMode,
};
#[allow(unused_imports)]
pub use paginated_select::{drain_pending_input, run_paginated_select};
pub use status_view::render_status;
pub use theme::Theme;
