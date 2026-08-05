//! 用户域 orphaned app data（对齐 Mole `clean_orphaned_app_data`）。
//!
//! NEVER 扫描：Containers / Group Containers / LaunchAgents / Application Scripts / `/Library/**`。

mod deps;
mod installed;
mod judge;
mod select;

pub use deps::{FakeOrphanDeps, LiveOrphanDeps, MdfindBudget, OrphanDeps, OrphanProbeError};
pub use installed::default_app_scan_roots;
pub use judge::{
    bundle_id_from_orphan_path, claude_vm_orphan_age_days_from_env,
    claude_vm_orphan_age_days_from_raw, is_claude_vm_bundle_path, is_sensitive_orphan_bundle,
    is_system_component_bundle, matches_orphan_name_prefix, orphan_age_days_from_env,
    orphan_age_days_from_raw, orphan_label, resource_kind_label, OrphanJudge,
};
pub use select::{select_orphaned_paths, OrphanScanError};

pub const ORPHANED_RULE_ID: &str = "orphaned-app-data";
pub const CLAUDE_DESKTOP_BUNDLE_ID: &str = "com.anthropic.claudefordesktop";
pub const DEFAULT_ORPHAN_AGE_DAYS: u32 = 30;
pub const DEFAULT_CLAUDE_VM_ORPHAN_AGE_DAYS: u32 = 7;
pub const MIN_ORPHAN_AGE_DAYS: u32 = 7;
pub const MAX_ORPHAN_ITERATIONS: usize = 100;
pub const MAX_MDFIND_CALLS: usize = 64;
