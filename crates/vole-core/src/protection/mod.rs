//! 应用保护层（对齐 mole `app_protection.sh` + `app_protection_data.sh`）。

mod bundle;
mod data;
mod glob_match;
mod path;

pub use bundle::should_protect_data;
pub use data::ProtectionCatalog;
pub use path::should_protect_path;

use crate::safety::PathProtection;

/// 默认 cleanup 保护策略。
#[derive(Debug)]
pub struct AppProtection {
    catalog: ProtectionCatalog,
}

impl Default for AppProtection {
    fn default() -> Self {
        Self::new()
    }
}

impl AppProtection {
    pub fn new() -> Self {
        Self {
            catalog: ProtectionCatalog::embedded(),
        }
    }

    pub fn catalog(&self) -> &ProtectionCatalog {
        &self.catalog
    }
}

impl PathProtection for AppProtection {
    fn should_protect(&self, policy_path: &str) -> bool {
        should_protect_path(policy_path, &self.catalog)
    }
}
