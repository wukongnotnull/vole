//! Optimize task discoverers and action planners.

mod actions;
mod delete_paths;

pub use actions::{
    apply_optimize_action, plan_coreduet_cleanup, plan_dock_refresh, plan_launch_services_rebuild,
    plan_legacy_overrides_audit, plan_network_optimization, plan_notification_cleanup,
    plan_prevent_network_dsstore, plan_quarantine_cleanup, plan_sqlite_vacuum,
    plan_system_maintenance, OptimizeActionError,
};
pub use delete_paths::{
    discover_cache_refresh, discover_fix_broken_configs, discover_launch_agents_cleanup,
    discover_saved_state_cleanup, OptimizeCandidate, SAVED_STATE_AGE_DAYS,
};
