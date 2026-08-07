//! 删除安全闸口（Phase 4a）。

mod critical;
mod endpoint;
mod plan_verify;
mod validate;

#[cfg(test)]
mod property;

pub use critical::{
    is_adobe_system_log_clean_target, is_critical_deletion_path, is_icon_services_system_cache,
    is_library_caches_temp_clean_target, is_private_allowlisted, is_private_tmp_clean_target,
    is_private_var_db_diagnostic_pipeline_clean_target, is_private_var_db_diagnostics_clean_target,
    is_private_var_db_memory_limit_violations_clean_target,
    is_private_var_db_powerlog_clean_target, is_private_var_log_clean_target,
    is_rosetta_update_bundle, is_system_diagnostic_report_leaf, normalize_policy_path,
    ADOBEGC_LOG_LIVE, ADOBE_LOGS_LIVE, ADOBE_SYSTEM_LOGS_MAX_DEPTH, CREATIVE_CLOUD_LOGS_LIVE,
    DIAGNOSTIC_REPORTS_SYSTEM_MARKER_LIVE, ICON_SERVICES_SYSTEM_CACHE_LIVE, LIBRARY_CACHES_LIVE,
    LIBRARY_CACHES_TEMP_MAX_DEPTH, PRIVATE_TMP_LIVE, PRIVATE_TMP_MAX_DEPTH,
    PRIVATE_VAR_DB_DIAGNOSTICS_LIVE, PRIVATE_VAR_DB_DIAGNOSTICS_MAX_DEPTH,
    PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_LIVE, PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_MAX_DEPTH,
    PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_LIVE, PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_MAX_DEPTH,
    PRIVATE_VAR_DB_POWERLOG_LIVE, PRIVATE_VAR_DB_POWERLOG_MAX_DEPTH, PRIVATE_VAR_LOG_LIVE,
    PRIVATE_VAR_LOG_MAX_DEPTH, PRIVATE_VAR_TMP_LIVE, ROSETTA_UPDATE_BUNDLE_LIVE,
};
pub use endpoint::is_endpoint_security_cache_path;
pub use plan_verify::{
    capture_plan_entry_identity, verify_plan_entry, verify_plan_entry_for_apply, PlanApplyError,
    PlanEntryIdentity, PlanVerifyError,
};
pub use validate::{validate_path_for_deletion, NoPathProtection, PathProtection, ValidationError};
