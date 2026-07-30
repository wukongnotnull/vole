//! Mole-aligned optimize task catalog and `rule_id` helpers.

mod catalog;
pub mod tasks;

pub use catalog::{
    optimize_action_rule_id, optimize_catalog, optimize_delete_rule_id, parse_optimize_rule_id,
    OptimizeTask, OptimizeTaskKind,
};
pub use tasks::{
    discover_cache_refresh, discover_saved_state_cleanup, OptimizeCandidate, SAVED_STATE_AGE_DAYS,
};
