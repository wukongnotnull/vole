//! Mole-aligned optimize task catalog and `rule_id` helpers.

mod catalog;
pub mod tasks;

pub use catalog::{
    optimize_action_rule_id, optimize_catalog, optimize_delete_rule_id, parse_optimize_rule_id,
    OptimizeTask, OptimizeTaskKind,
};
pub use tasks::{
    apply_optimize_action, discover_cache_refresh, discover_fix_broken_configs,
    discover_launch_agents_cleanup, discover_saved_state_cleanup, is_memory_pressure_high,
    plan_coreduet_cleanup, plan_dock_refresh, plan_launch_services_rebuild,
    plan_legacy_overrides_audit, plan_memory_pressure_relief, plan_network_optimization,
    plan_notification_cleanup, plan_prevent_network_dsstore, plan_quarantine_cleanup,
    plan_sqlite_vacuum, plan_system_maintenance, OptimizeActionError, OptimizeCandidate,
    SAVED_STATE_AGE_DAYS,
};
