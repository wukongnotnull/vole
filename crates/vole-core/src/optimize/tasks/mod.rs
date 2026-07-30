//! Optimize task discoverers and action planners.

mod delete_paths;

pub use delete_paths::{
    discover_cache_refresh, discover_saved_state_cleanup, OptimizeCandidate, SAVED_STATE_AGE_DAYS,
};
