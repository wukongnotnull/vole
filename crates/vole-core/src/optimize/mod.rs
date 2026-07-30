//! Mole-aligned optimize task catalog and `rule_id` helpers.

mod catalog;

pub use catalog::{
    optimize_action_rule_id, optimize_catalog, optimize_delete_rule_id, parse_optimize_rule_id,
    OptimizeTask, OptimizeTaskKind,
};
