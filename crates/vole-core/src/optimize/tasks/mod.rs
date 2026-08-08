//! Optimize task discoverers and action planners.

mod actions;
mod delete_paths;
mod disk_verify;
mod login_items_audit;
mod shared_file_list;
mod spotlight_index;
mod spotlight_orphan_rules;

pub use actions::{
    apply_optimize_action, has_active_vpn, is_memory_pressure_high, needs_disk_permissions_repair,
    network_stack_needs_flush, optimize_action_home, periodic_needs_run, plan_coreduet_cleanup,
    plan_disk_permissions_repair, plan_dock_refresh, plan_launch_services_rebuild,
    plan_legacy_overrides_audit, plan_memory_pressure_relief, plan_network_optimization,
    plan_network_stack_optimize, plan_notification_cleanup, plan_periodic_maintenance,
    plan_prevent_network_dsstore, plan_quarantine_cleanup, plan_sqlite_vacuum,
    plan_system_maintenance, OptimizeActionError,
};
pub use delete_paths::{
    discover_cache_refresh, discover_fix_broken_configs, discover_launch_agents_cleanup,
    discover_saved_state_cleanup, OptimizeCandidate, SAVED_STATE_AGE_DAYS,
};
pub use disk_verify::{
    plan_disk_verify, DiskVerifyDeps, DiskVerifyError, FakeDiskVerifyDeps, LiveDiskVerifyDeps,
};
pub use login_items_audit::{
    plan_login_items_audit, FakeLoginItemsAuditDeps, LiveLoginItemsAuditDeps, LoginItemSnapshot,
    LoginItemsAuditDeps, LoginItemsAuditError,
};
pub use shared_file_list::{
    plan_shared_file_list_repair, FakeSharedFileListDeps, LiveSharedFileListDeps,
    SharedFileListDeps, SharedFileListError,
};
pub use spotlight_index::plan_spotlight_index_optimize;
pub use spotlight_orphan_rules::{
    plan_spotlight_orphan_rules_cleanup, FakeSpotlightOrphanDeps, LiveSpotlightOrphanDeps,
    SpotlightOrphanDeps, SpotlightOrphanError,
};
