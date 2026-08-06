//! CLI 非交互提权（`sudo -n`）与 allowlist。
//!
//! 桌面 SMAppService 另开接缝实现，本期仅 `SudoNoninteractive` / `NoPrivilege`。

mod sudo;

pub use sudo::{NoPrivilege, RecordingPrivilege, SudoNoninteractive};

use std::path::{Component, Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivilegeError {
    Unavailable,
    Refused,
    CommandFailed(String),
}

pub trait PrivilegeBackend: Send + Sync {
    fn probe_noninteractive(&self) -> bool;
    fn remove_permanent(&self, path: &Path) -> Result<(), PrivilegeError>;
    fn launchctl_unload(&self, plist: &Path) -> Result<(), PrivilegeError>;
}

const ALLOWED_PREFIXES: &[&str] = &[
    "/Library/LaunchDaemons/",
    "/Library/LaunchAgents/",
    "/Library/PrivilegedHelperTools/",
];

/// 绝对路径、无 `..`，且位于三树前缀之下。
pub fn path_allowed_for_privilege(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    let Some(s) = path.to_str() else {
        return false;
    };
    ALLOWED_PREFIXES.iter().any(|prefix| s.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn allowlist_accepts_three_roots_only() {
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/LaunchDaemons/com.example.plist"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/LaunchAgents/com.example.plist"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/PrivilegedHelperTools/com.example.helper"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Caches/foo"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/LaunchDaemons/../Preferences/com.apple.plist"
        )));
        assert!(!path_allowed_for_privilege(Path::new("LaunchDaemons/x")));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/LaunchDaemonsEvil/x"
        )));
    }

    #[test]
    fn no_privilege_probe_false_and_refuses_remove() {
        let b = NoPrivilege;
        assert!(!b.probe_noninteractive());
        assert!(matches!(
            b.remove_permanent(Path::new("/Library/LaunchDaemons/x.plist")),
            Err(PrivilegeError::Unavailable)
        ));
    }

    #[test]
    fn recording_backend_remove_requires_allowlist() {
        let b = RecordingPrivilege::allowing();
        assert!(matches!(
            b.remove_permanent(Path::new("/tmp/evil")),
            Err(PrivilegeError::Refused)
        ));
        assert!(b.removed.lock().unwrap().is_empty());
        b.remove_permanent(Path::new("/Library/LaunchDaemons/com.x.plist"))
            .unwrap();
        assert_eq!(b.removed.lock().unwrap().len(), 1);
    }
}
