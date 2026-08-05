//! Mole `known_protect_patterns` + `_system_service_app_exists` 同形。

use std::path::Path;

use crate::orphan::OrphanDeps;
use crate::protection::glob_match::bundle_matches_pattern;
use crate::protection::is_reverse_dns_bundle_id;

/// `(bundle_id_glob, pipe-separated app paths)`；空 app path = 无条件保护。
pub const KNOWN_PROTECT_PATTERNS: &[(&str, &str)] = &[
    ("com.sogou.*", "/Library/Input Methods/SogouInput.app"),
    ("com.west2online.ClashX.*", "/Applications/ClashX.app"),
    ("com.clashmac.*", "/Applications/ClashMac.app"),
    (
        "com.nektony.AC*",
        "/Applications/App Cleaner & Uninstaller.app",
    ),
    ("cn.i4tools.*", "/Applications/i4Tools.app"),
    ("com.macpaw.CleanMyMac*", "/Applications/CleanMyMac X.app"),
    ("org.wireshark.ChmodBPF", "/Applications/Wireshark.app"),
    ("us.zoom.*", "/Applications/zoom.us.app"),
    ("it.remote.cli", "/Applications/Remote.It.app"),
    ("com.docker.*", "/Applications/Docker.app"),
    ("netbird", "/usr/local/bin/netbird"),
    (
        "com.intego.*",
        "/Library/Intego|/Applications/Intego|/Library/Application Support/Intego",
    ),
    ("homebrew.mxcl.*", ""),
];

/// Mole `_system_service_app_exists`：空 path 恒 true；mdfind fail-closed 视为仍安装。
pub fn system_service_app_exists(
    bundle_id: &str,
    app_path_raw: &str,
    home: &Path,
    deps: &dyn OrphanDeps,
) -> bool {
    if app_path_raw.is_empty() {
        return true;
    }
    for raw in app_path_raw.split('|') {
        if app_path_present(raw, home) {
            return true;
        }
    }
    if is_reverse_dns_bundle_id(bundle_id) {
        if !deps.spotlight_available() {
            return true;
        }
        match deps.mdfind_bundle(bundle_id) {
            Ok(true) => return true,
            Ok(false) => {}
            Err(_) => return true,
        }
    }
    false
}

/// 某个 protect pattern 是否覆盖该 id 且判定 app 仍在（保护）。
pub fn is_known_protected(bundle_or_filename: &str, home: &Path, deps: &dyn OrphanDeps) -> bool {
    for (pattern, app_paths) in KNOWN_PROTECT_PATTERNS {
        if !bundle_matches_pattern(bundle_or_filename, pattern) {
            continue;
        }
        // Mole: pattern 匹配后以 `_system_service_app_exists` 判定；匹配即 stop 搜。
        return system_service_app_exists(bundle_or_filename, app_paths, home, deps);
    }
    false
}

fn app_path_present(raw: &str, home: &Path) -> bool {
    let path = Path::new(raw);
    if path_exists(path) {
        return true;
    }
    let Some(name) = path.file_name() else {
        return false;
    };
    if raw.starts_with("/Applications/") {
        if path_exists(&home.join("Applications").join(name)) {
            return true;
        }
        if path_exists(&Path::new("/Applications/Setapp").join(name)) {
            return true;
        }
    }
    if raw.starts_with("/Library/Input Methods/")
        && path_exists(&home.join("Library/Input Methods").join(name))
    {
        return true;
    }
    false
}

fn path_exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orphan::FakeOrphanDeps;
    use std::collections::HashSet;
    use std::fs;

    #[test]
    fn empty_app_path_always_protected() {
        let deps = FakeOrphanDeps::default();
        assert!(system_service_app_exists(
            "homebrew.mxcl.nginx",
            "",
            Path::new("/Users/t"),
            &deps
        ));
        assert!(is_known_protected(
            "homebrew.mxcl.nginx",
            Path::new("/Users/t"),
            &deps
        ));
    }

    #[test]
    fn protect_when_app_dir_exists() {
        let root =
            std::env::temp_dir().join(format!("vole-sysorphan-protect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let apps = root.join("Applications");
        fs::create_dir_all(apps.join("ClashX.app")).unwrap();
        // Point known pattern to our fake app by using system_service_app_exists directly.
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        assert!(system_service_app_exists(
            "com.west2online.ClashX.helper",
            &apps.join("ClashX.app").to_string_lossy(),
            &root,
            &deps
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn mdfind_unavailable_fail_closed_as_installed() {
        let deps = FakeOrphanDeps {
            spotlight: false,
            ..Default::default()
        };
        assert!(system_service_app_exists(
            "com.example.helper",
            "/Applications/Missing.app",
            Path::new("/Users/t"),
            &deps
        ));
    }
}
