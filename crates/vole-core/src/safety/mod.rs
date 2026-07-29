//! 删除安全闸口（Phase 4a）。

mod critical;
mod endpoint;
mod plan_verify;
mod validate;

#[cfg(test)]
mod property;

pub use critical::{is_critical_deletion_path, is_private_allowlisted, normalize_policy_path};
pub use endpoint::is_endpoint_security_cache_path;
pub use plan_verify::{
    capture_plan_entry_identity, verify_plan_entry, verify_plan_entry_for_apply, PlanApplyError,
    PlanEntryIdentity, PlanVerifyError,
};
pub use validate::{validate_path_for_deletion, NoPathProtection, PathProtection, ValidationError};
