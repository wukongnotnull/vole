//! ratatui 立即模式 TUI。

mod analyze_actions;
mod analyze_state;
mod analyze_view;
mod home_menu;
mod home_menu_state;
mod menu_state;
mod paginated_select;
mod status_cat;
mod status_prefs;
mod status_view;
mod theme;
mod widgets;

pub use analyze_actions::{
    apply_removals, open_argv, preview_target, reveal_argv, spawn_detached, trash_analyze_paths,
};
#[allow(unused_imports)]
pub use analyze_state::{map_analyze_key, AnalyzeEffect, AnalyzeKey, AnalyzeState};
pub use analyze_view::{render_analyze, AnalyzeRenderOpts};
#[allow(unused_imports)]
pub use home_menu::{
    brand_ascii_lines, map_key, run_home_menu, HomeMenuRunOpts, VOLE_REPO_URL, VOLE_TAGLINE,
};
#[allow(unused_imports)]
pub use home_menu_state::{
    HomeAction, HomeCommand, HomeItem, HomeKey, HomeMenuConfig, HomeMenuState, HOME_ITEMS,
};
#[allow(unused_imports)]
pub use widgets::AnalyzeFooterMode;
// Re-exported for uninstall / whitelist TTY wiring and external unit tests.
#[allow(unused_imports)]
pub use menu_state::{
    EmptyMenuError, MenuConfig, MenuItem, MenuKey, MenuState, SelectOutcome, SortMode,
};
#[allow(unused_imports)]
pub use paginated_select::{drain_pending_input, run_paginated_select};
#[allow(unused_imports)]
pub use status_cat::render_mole_frame;
pub use status_prefs::{load_status_prefs, next_cpu_cores, save_cat_hidden, save_cpu_cores};
pub use status_view::{render_status, StatusRenderOpts};
pub use theme::Theme;
