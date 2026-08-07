//! 删除安全闸口（Phase 4a）。

mod critical;
mod endpoint;
mod plan_verify;
mod validate;

#[cfg(test)]
mod property;

pub use critical::{
    is_critical_deletion_path, is_icon_services_system_cache, is_private_allowlisted,
    is_private_var_log_clean_target, is_rosetta_update_bundle, is_system_diagnostic_report_leaf,
    normalize_policy_path, DIAGNOSTIC_REPORTS_SYSTEM_MARKER_LIVE, ICON_SERVICES_SYSTEM_CACHE_LIVE,
    PRIVATE_VAR_LOG_LIVE, PRIVATE_VAR_LOG_MAX_DEPTH, ROSETTA_UPDATE_BUNDLE_LIVE,
};
pub use endpoint::is_endpoint_security_cache_path;
pub use plan_verify::{
    capture_plan_entry_identity, verify_plan_entry, verify_plan_entry_for_apply, PlanApplyError,
    PlanEntryIdentity, PlanVerifyError,
};
pub use validate::{validate_path_for_deletion, NoPathProtection, PathProtection, ValidationError};
