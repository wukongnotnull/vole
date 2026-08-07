//! CLI 非交互提权（`sudo -n`）与 allowlist。
//!
//! 桌面 SMAppService 另开接缝实现，本期仅 `SudoNoninteractive` / `NoPrivilege`。

mod sudo;

pub use sudo::{NoPrivilege, RecordingPrivilege, SudoNoninteractive};

use crate::safety::is_rosetta_update_bundle;

use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// `rosetta-2-cache` 规则 id（1.12.0）。
pub const ROSETTA_CACHE_RULE_ID: &str = "rosetta-2-cache";

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

const LIVE_PREFIXES: &[&str] = &[
    "/Library/LaunchDaemons/",
    "/Library/LaunchAgents/",
    "/Library/PrivilegedHelperTools/",
];

fn privilege_prefixes() -> Vec<String> {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let base = PathBuf::from(base);
        return vec![
            format!("{}/", base.join("LaunchDaemons").display()),
            format!("{}/", base.join("LaunchAgents").display()),
            format!("{}/", base.join("PrivilegedHelperTools").display()),
        ];
    }
    LIVE_PREFIXES.iter().map(|s| (*s).to_string()).collect()
}

/// 运行时是否 Apple Silicon 原生进程（对齐 Mole `uname -m == arm64`）。
pub fn is_arm64_host() -> bool {
    if let Ok(v) = std::env::var("VOLE_TEST_FORCE_UNAME_M") {
        return v.trim() == "arm64";
    }
    let Ok(out) = Command::new("uname").arg("-m").output() else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    String::from_utf8_lossy(&out.stdout).trim() == "arm64"
}

/// live 或 `VOLE_TEST_SYSTEM_LIBRARY` 映射下的 Rosetta bundle 路径。
pub fn rosetta_bundle_path() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        return PathBuf::from(base).join("Apple/usr/share/rosetta/rosetta_update_bundle");
    }
    PathBuf::from(crate::safety::ROSETTA_UPDATE_BUNDLE_LIVE)
}

/// plan 候选：arm64 且路径存在时返回该 exact。
pub fn rosetta_plan_candidates() -> Vec<PathBuf> {
    if !is_arm64_host() {
        return Vec::new();
    }
    let path = rosetta_bundle_path();
    if path.exists() {
        vec![path]
    } else {
        Vec::new()
    }
}

/// 绝对路径、无 `..`，且：Rosetta exact **或** 三树下单层叶。
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
    if is_rosetta_update_bundle(s) {
        return true;
    }
    for prefix in privilege_prefixes() {
        let Some(rest) = s.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() || rest.contains('/') {
            return false;
        }
        if prefix.ends_with("LaunchDaemons/") || prefix.ends_with("LaunchAgents/") {
            return rest.ends_with(".plist") && !rest.starts_with("com.apple.");
        }
        return !rest.starts_with("com.apple.");
    }
    false
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
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/LaunchDaemons/"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/LaunchDaemons/com.apple.foo.plist"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/LaunchDaemons/subdir/x.plist"
        )));
    }

    #[test]
    fn allowlist_accepts_rosetta_exact() {
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Apple/usr/share/rosetta/rosetta_update_bundle"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Apple/usr/share/rosetta"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Apple/usr/share/rosetta/rosetta_update_bundle/x"
        )));
    }

    #[test]
    fn arm64_host_respects_force_env() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "arm64");
        assert!(is_arm64_host());
        std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "x86_64");
        assert!(!is_arm64_host());
        std::env::remove_var("VOLE_TEST_FORCE_UNAME_M");
    }

    #[test]
    fn rosetta_plan_candidates_respect_arch_and_fixture() {
        let _guard = crate::test_env::lock();
        let root = tempfile::tempdir().unwrap();
        let lib = root.path().join("Library");
        let bundle = lib.join("Apple/usr/share/rosetta/rosetta_update_bundle");
        std::fs::create_dir_all(bundle.parent().unwrap()).unwrap();
        std::fs::write(&bundle, b"x").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "x86_64");
        assert!(rosetta_plan_candidates().is_empty());

        std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "arm64");
        let c = rosetta_plan_candidates();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0], bundle);

        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        std::env::remove_var("VOLE_TEST_FORCE_UNAME_M");
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
