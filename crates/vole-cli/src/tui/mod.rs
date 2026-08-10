//! ratatui 立即模式 TUI。

mod analyze_actions;
mod analyze_state;
mod analyze_view;
mod home_menu;
mod home_menu_state;
mod menu_state;
mod paginated_select;
mod status_view;
mod theme;
mod widgets;

pub use analyze_actions::{
    apply_removals, open_argv, preview_target, spawn_detached, trash_analyze_paths,
};
pub use analyze_state::{map_analyze_key, AnalyzeEffect, AnalyzeKey, AnalyzeState};
pub use analyze_view::{render_analyze, AnalyzeRenderOpts};
pub use widgets::AnalyzeFooterMode;
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
