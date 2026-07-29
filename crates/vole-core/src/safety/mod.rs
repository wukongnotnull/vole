//! 删除安全闸口（Phase 4a）。

mod critical;
mod endpoint;
mod validate;

pub use critical::{is_critical_deletion_path, is_private_allowlisted, normalize_policy_path};
pub use endpoint::is_endpoint_security_cache_path;
pub use validate::{validate_path_for_deletion, NoPathProtection, PathProtection, ValidationError};
