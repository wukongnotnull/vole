//! 应用保护层（对齐 mole `app_protection.sh` + `app_protection_data.sh`）。

mod bundle;
mod data;
mod glob_match;
mod path;
mod uninstall;

pub use bundle::should_protect_data;
pub use data::{OfficialUninstallerRule, ProtectionCatalog};
pub use path::{should_protect_path, ProtectionMode};
pub use uninstall::{official_uninstaller_vendor, should_protect_from_uninstall};

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

    pub fn protects_path_mode(&self, path: &str, mode: ProtectionMode) -> bool {
        should_protect_path(path, &self.catalog, mode)
    }
}

impl PathProtection for AppProtection {
    fn should_protect(&self, policy_path: &str) -> bool {
        should_protect_path(policy_path, &self.catalog, ProtectionMode::Cleanup)
    }
}

/// Uninstall 模式的 `PathProtection` 适配器（供 apply 注入）。
#[derive(Debug, Clone, Copy)]
pub struct UninstallPathProtection<'a> {
    inner: &'a AppProtection,
}

impl<'a> UninstallPathProtection<'a> {
    pub fn new(inner: &'a AppProtection) -> Self {
        Self { inner }
    }
}

impl PathProtection for UninstallPathProtection<'_> {
    fn should_protect(&self, policy_path: &str) -> bool {
        self.inner
            .protects_path_mode(policy_path, ProtectionMode::Uninstall)
    }
}
