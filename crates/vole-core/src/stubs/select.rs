//! Mole `clean_orphaned_container_stubs` 扫描判定同形（无删除）。

use std::path::{Path, PathBuf};

use crate::orphan::OrphanDeps;
use crate::protection::glob_match::bundle_matches_pattern;
use crate::protection::is_reverse_dns_bundle_id;

use super::{CONTAINER_STUB_METADATA, STUB_ALLOWLIST};

#[derive(Debug, PartialEq, Eq)]
pub enum StubScanError {
    /// `~/Library/Containers` 存在但不可列（FDA 缺失等）→ 整规则降级。
    ContainersInaccessible,
}

/// 非 symlink 目录，且唯一子项是普通文件 `.com.apple.containermanagerd.metadata.plist`。
/// select 与 apply 重验共用（Mole `_remove_verified_container_stub` 前半段同形）。
pub fn is_verified_stub_dir(dir: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(dir) else {
        return false;
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut saw_metadata = false;
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        if entry.file_name().to_str() != Some(CONTAINER_STUB_METADATA) {
            return false;
        }
        match entry.file_type() {
            Ok(ft) if ft.is_file() => saw_metadata = true,
            _ => return false,
        }
    }
    saw_metadata
}

/// 扫描 `$HOME/Library/Containers`，返回 allowlist 命中且 app 不在的 stub 目录。
pub fn select_container_stubs(
    home: &Path,
    deps: &dyn OrphanDeps,
) -> Result<Vec<PathBuf>, StubScanError> {
    let containers = home.join("Library/Containers");
    if !containers.exists() {
        return Ok(Vec::new());
    }
    let entries =
        std::fs::read_dir(&containers).map_err(|_| StubScanError::ContainersInaccessible)?;

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name_os = entry.file_name();
        let Some(name) = name_os.to_str() else {
            continue;
        };
        let Some(app_path) = allowlist_app_path(name) else {
            continue;
        };
        let dir = entry.path();
        if !is_verified_stub_dir(&dir) {
            continue;
        }
        if container_stub_app_exists(name, app_path, home, deps) {
            continue;
        }
        out.push(dir);
    }
    out.sort();
    Ok(out)
}

fn allowlist_app_path(bundle_id: &str) -> Option<&'static str> {
    STUB_ALLOWLIST
        .iter()
        .find(|(glob, _)| bundle_matches_pattern(bundle_id, glob))
        .map(|(_, app_path)| *app_path)
}

/// apply 对不可信 / 过期 plan 的策略重验（对齐 `recheck_orphaned_entry`）。
///
/// 必须全部通过才允许 carve-out 删除：路径形状（Containers 单层）→
/// 硬编码 allowlist → stub 形状 → app 不存在（fail-closed）。
pub fn recheck_container_stub_entry(path: &Path, home: &Path, deps: &dyn OrphanDeps) -> bool {
    if !super::is_container_stub_candidate_path(path, home) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(app_path) = allowlist_app_path(name) else {
        return false;
    };
    if !is_verified_stub_dir(path) {
        return false;
    }
    !container_stub_app_exists(name, app_path, home, deps)
}

/// Mole `_container_stub_app_exists` 同形：canonical 路径 → `~/Applications` →
/// Setapp 两处 → reverse-DNS 才 mdfind（fail-closed 视为仍安装）。
fn container_stub_app_exists(
    bundle_id: &str,
    app_path_raw: &str,
    home: &Path,
    deps: &dyn OrphanDeps,
) -> bool {
    let app_path = Path::new(app_path_raw);
    if app_path.exists() {
        return true;
    }
    if let Some(name) = app_path.file_name() {
        if app_path_raw.starts_with("/Applications/") {
            if home.join("Applications").join(name).exists() {
                return true;
            }
            if Path::new("/Applications/Setapp").join(name).exists() {
                return true;
            }
            if home
                .join("Library/Application Support/Setapp/Applications")
                .join(name)
                .exists()
            {
                return true;
            }
        }
    }
    if is_reverse_dns_bundle_id(bundle_id) {
        if !deps.spotlight_available() {
            return true;
        }
        match deps.mdfind_bundle(bundle_id) {
            Ok(true) => true,
            Ok(false) => false,
            Err(_) => true,
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orphan::FakeOrphanDeps;
    use std::fs;

    fn temp_home(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("vole-stubs-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Library/Containers")).unwrap();
        root
    }

    fn make_stub(home: &Path, bundle_id: &str) -> PathBuf {
        let dir = home.join("Library/Containers").join(bundle_id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CONTAINER_STUB_METADATA), b"plist").unwrap();
        dir
    }

    fn deps_spotlight_on() -> FakeOrphanDeps {
        FakeOrphanDeps {
            spotlight: true,
            ..Default::default()
        }
    }

    #[test]
    fn pure_stub_in_allowlist_selected() {
        let home = temp_home("pure");
        let dir = make_stub(&home, "com.macpaw.CleanMyMac4");
        let got = select_container_stubs(&home, &deps_spotlight_on()).unwrap();
        assert_eq!(got, vec![dir]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn stub_with_data_dir_not_selected() {
        let home = temp_home("data");
        let dir = make_stub(&home, "com.macpaw.CleanMyMac4");
        fs::create_dir_all(dir.join("Data")).unwrap();
        assert!(select_container_stubs(&home, &deps_spotlight_on())
            .unwrap()
            .is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn symlink_dir_not_selected() {
        let home = temp_home("symlink");
        let real = home.join("real-target");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join(CONTAINER_STUB_METADATA), b"plist").unwrap();
        let link = home.join("Library/Containers/com.macpaw.CleanMyMac4");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(select_container_stubs(&home, &deps_spotlight_on())
            .unwrap()
            .is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn missing_or_dir_metadata_not_selected() {
        let home = temp_home("meta");
        // 空目录（无 metadata）
        fs::create_dir_all(home.join("Library/Containers/com.macpaw.CleanMyMac4")).unwrap();
        // metadata 是目录
        let dir2 = home.join("Library/Containers/com.macpaw.CleanMyMacX");
        fs::create_dir_all(dir2.join(CONTAINER_STUB_METADATA)).unwrap();
        assert!(select_container_stubs(&home, &deps_spotlight_on())
            .unwrap()
            .is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn teamid_prefixed_stub_selected_via_mdfind_miss() {
        let home = temp_home("teamid");
        let dir = make_stub(&home, "S8EX82NJP6.com.macpaw.CleanMyMac4");
        // TeamID 前缀仍是 reverse-DNS（Mole 正则同形）→ 走 mdfind；Ok(false) → 未安装。
        let got = select_container_stubs(&home, &deps_spotlight_on()).unwrap();
        assert_eq!(got, vec![dir]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn non_reverse_dns_prefix_skips_mdfind() {
        let home = temp_home("nonrdns");
        // 含空格 → 非 reverse-DNS → 跳过 mdfind，spotlight 关闭也照选。
        let dir = make_stub(&home, "My Team.com.macpaw.CleanMyMac4");
        let deps = FakeOrphanDeps {
            spotlight: false,
            ..Default::default()
        };
        let got = select_container_stubs(&home, &deps).unwrap();
        assert_eq!(got, vec![dir]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn outside_allowlist_not_selected() {
        let home = temp_home("outside");
        make_stub(&home, "com.example.app");
        assert!(select_container_stubs(&home, &deps_spotlight_on())
            .unwrap()
            .is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn app_in_home_applications_blocks_selection() {
        let home = temp_home("appdir");
        make_stub(&home, "com.macpaw.CleanMyMac4");
        fs::create_dir_all(home.join("Applications/CleanMyMac X.app")).unwrap();
        assert!(select_container_stubs(&home, &deps_spotlight_on())
            .unwrap()
            .is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn spotlight_unavailable_fail_closed_not_selected() {
        let home = temp_home("nospotlight");
        make_stub(&home, "com.macpaw.CleanMyMac4");
        let deps = FakeOrphanDeps {
            spotlight: false,
            ..Default::default()
        };
        assert!(select_container_stubs(&home, &deps).unwrap().is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn mdfind_error_fail_closed_not_selected() {
        let home = temp_home("mderr");
        make_stub(&home, "com.macpaw.CleanMyMac4");
        let mut deps = deps_spotlight_on();
        deps.mdfind.insert(
            "com.macpaw.CleanMyMac4".to_string(),
            Err(crate::orphan::OrphanProbeError::Unavailable),
        );
        assert!(select_container_stubs(&home, &deps).unwrap().is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn missing_containers_root_yields_empty() {
        let root = std::env::temp_dir().join(format!("vole-stubs-noroot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        assert!(select_container_stubs(&root, &deps_spotlight_on())
            .unwrap()
            .is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unreadable_containers_root_degrades() {
        use std::os::unix::fs::PermissionsExt;
        let home = temp_home("denied");
        let containers = home.join("Library/Containers");
        fs::set_permissions(&containers, fs::Permissions::from_mode(0o000)).unwrap();
        let got = select_container_stubs(&home, &deps_spotlight_on());
        fs::set_permissions(&containers, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(got, Err(StubScanError::ContainersInaccessible));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn recheck_rejects_outside_path_non_allowlist_and_app_present() {
        let home = temp_home("recheck");
        let stub = make_stub(&home, "com.macpaw.CleanMyMac4");
        let deps = deps_spotlight_on();
        assert!(recheck_container_stub_entry(&stub, &home, &deps));

        let outside = home.join("Library/Preferences/com.macpaw.CleanMyMac4");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join(CONTAINER_STUB_METADATA), b"p").unwrap();
        assert!(!recheck_container_stub_entry(&outside, &home, &deps));

        let other = make_stub(&home, "com.example.app");
        assert!(!recheck_container_stub_entry(&other, &home, &deps));

        fs::create_dir_all(home.join("Applications/CleanMyMac X.app")).unwrap();
        assert!(!recheck_container_stub_entry(&stub, &home, &deps));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn candidate_path_shape_gate() {
        use super::super::is_container_stub_candidate_path as gate;
        let home = Path::new("/Users/t");
        assert!(gate(
            Path::new("/Users/t/Library/Containers/com.macpaw.CleanMyMac4"),
            home
        ));
        // 更深层级 / 根本身 / 家外路径 / `..` 均拒绝。
        assert!(!gate(
            Path::new("/Users/t/Library/Containers/com.macpaw.CleanMyMac4/Data"),
            home
        ));
        assert!(!gate(Path::new("/Users/t/Library/Containers"), home));
        assert!(!gate(Path::new("/Users/other/Library/Containers/x"), home));
        assert!(!gate(
            Path::new("/Users/t/Library/Containers/../Preferences"),
            home
        ));
    }

    #[test]
    fn label_uses_basename() {
        assert_eq!(
            super::super::container_stub_label(Path::new(
                "/Users/t/Library/Containers/com.macpaw.CleanMyMac4"
            )),
            "Orphaned container stub: com.macpaw.CleanMyMac4"
        );
    }
}
