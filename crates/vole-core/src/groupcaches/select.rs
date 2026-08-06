//! Group Containers 扫描判定（Mole `clean_group_container_caches` 同形）。

use std::fs;
use std::path::{Path, PathBuf};

use crate::protection::ProtectionCatalog;

use super::{
    is_apple_group_container, is_group_container_protected, MAX_LEAVES_PER_CANDIDATE,
    MAX_LEAVES_TOTAL,
};

#[derive(Debug, PartialEq, Eq)]
pub enum GroupCacheScanError {
    /// `~/Library/Group Containers` 存在但不可列。
    GroupContainersInaccessible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupCacheSelectResult {
    pub paths: Vec<PathBuf>,
    /// 任一候选子树 / 整规则触达规模上限。
    pub truncated: bool,
}

pub fn select_group_container_caches(
    home: &Path,
) -> Result<GroupCacheSelectResult, GroupCacheScanError> {
    let root = home.join("Library/Group Containers");
    if !root.exists() {
        return Ok(GroupCacheSelectResult {
            paths: Vec::new(),
            truncated: false,
        });
    }
    let entries =
        fs::read_dir(&root).map_err(|_| GroupCacheScanError::GroupContainersInaccessible)?;
    let catalog = ProtectionCatalog::embedded();

    let mut out = Vec::new();
    let mut truncated = false;

    for entry in entries.flatten() {
        let dir = entry.path();
        let Ok(meta) = fs::symlink_metadata(&dir) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        // 不可读容器跳过（避免重复 TCC），不 degrade
        if fs::read_dir(&dir).is_err() {
            continue;
        }
        let Some(id) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_apple_group_container(id) {
            continue;
        }
        if looks_like_safari_web_extension(home, id) {
            continue;
        }
        let protected = is_group_container_protected(id, &catalog);

        let mut candidates: Vec<PathBuf> = vec![dir.join("Logs"), dir.join("Library/Logs")];
        if !protected {
            candidates.extend([
                dir.join("tmp"),
                dir.join("Library/tmp"),
                dir.join("Caches"),
                dir.join("Library/Caches"),
            ]);
        }

        for cand in candidates {
            match collect_leaves(&cand, &mut out, &mut truncated) {
                CollectOutcome::Continue => {}
                CollectOutcome::StopTotal => {
                    out.sort();
                    return Ok(GroupCacheSelectResult {
                        paths: out,
                        truncated: true,
                    });
                }
            }
        }
    }

    out.sort();
    Ok(GroupCacheSelectResult {
        paths: out,
        truncated,
    })
}

enum CollectOutcome {
    Continue,
    StopTotal,
}

fn collect_leaves(
    candidate: &Path,
    out: &mut Vec<PathBuf>,
    truncated: &mut bool,
) -> CollectOutcome {
    let Ok(meta) = fs::symlink_metadata(candidate) else {
        return CollectOutcome::Continue;
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return CollectOutcome::Continue;
    }
    let Ok(rd) = fs::read_dir(candidate) else {
        return CollectOutcome::Continue;
    };

    let mut leaves = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(m) = fs::symlink_metadata(&path) else {
            continue;
        };
        if m.file_type().is_symlink() {
            continue;
        }
        leaves.push(path);
        if leaves.len() > MAX_LEAVES_PER_CANDIDATE {
            *truncated = true;
            // 整树不提任何叶子
            return CollectOutcome::Continue;
        }
    }

    for path in leaves {
        if out.len() >= MAX_LEAVES_TOTAL {
            *truncated = true;
            return CollectOutcome::StopTotal;
        }
        out.push(path);
    }
    CollectOutcome::Continue
}

/// Safari Web Extension：若 `~/Library/Containers/<id>` 存在，列其顶层；
/// 任一名字（不分大小写）含 `safari` → 跳过；目录不可读 → 跳过（fail-closed）。
fn looks_like_safari_web_extension(home: &Path, container_id: &str) -> bool {
    let containers = home.join("Library/Containers").join(container_id);
    if !containers.exists() {
        return false;
    }
    let Ok(rd) = fs::read_dir(&containers) else {
        return true; // fail-closed
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let Some(s) = name.to_str() else {
            continue;
        };
        if s.to_ascii_lowercase().contains("safari") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn temp_home(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("vole-gcc-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Library/Group Containers")).unwrap();
        root
    }

    fn mk_leaves(home: &Path, container: &str, sub: &str, names: &[&str]) {
        let dir = home
            .join("Library/Group Containers")
            .join(container)
            .join(sub);
        fs::create_dir_all(&dir).unwrap();
        for n in names {
            fs::write(dir.join(n), b"x").unwrap();
        }
    }

    #[test]
    fn non_protected_selects_logs_caches_tmp_leaves() {
        let home = temp_home("np");
        mk_leaves(
            &home,
            "group.com.example.app",
            "Logs",
            &["a.log", ".DS_Store"],
        );
        mk_leaves(&home, "group.com.example.app", "Library/Caches", &["c1"]);
        mk_leaves(&home, "group.com.example.app", "tmp", &["t1"]);
        mk_leaves(
            &home,
            "group.com.example.app",
            "Library/Application Support",
            &["keep"],
        );

        let got = select_group_container_caches(&home).unwrap();
        assert!(!got.truncated);
        let rels: Vec<_> = got
            .paths
            .iter()
            .map(|p| {
                p.strip_prefix(home.join("Library/Group Containers"))
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect();
        assert!(rels.iter().any(|r| r.ends_with("Logs/a.log")));
        assert!(rels.iter().any(|r| r.ends_with("Logs/.DS_Store")));
        assert!(rels.iter().any(|r| r.ends_with("Library/Caches/c1")));
        assert!(rels.iter().any(|r| r.ends_with("tmp/t1")));
        assert!(!rels.iter().any(|r| r.contains("Application Support")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn protected_macpaw_only_logs_candidates_from_handler() {
        let home = temp_home("prot");
        mk_leaves(&home, "com.macpaw.CleanMyMac", "Logs", &["x.log"]);
        mk_leaves(&home, "com.macpaw.CleanMyMac", "Library/Caches", &["y"]);
        let got = select_group_container_caches(&home).unwrap();
        assert!(got.paths.iter().any(|p| p.ends_with("Logs/x.log")));
        assert!(!got
            .paths
            .iter()
            .any(|p| p.to_string_lossy().contains("Caches")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn teamid_protected_vendor_skips_caches() {
        let home = temp_home("teamid");
        mk_leaves(
            &home,
            "S8EX82NJP6.com.macpaw.CleanMyMac",
            "Logs",
            &["x.log"],
        );
        mk_leaves(&home, "S8EX82NJP6.com.macpaw.CleanMyMac", "Caches", &["y"]);
        let got = select_group_container_caches(&home).unwrap();
        assert!(got.paths.iter().any(|p| p.ends_with("Logs/x.log")));
        assert!(!got
            .paths
            .iter()
            .any(|p| p.to_string_lossy().contains("Caches")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn apple_and_notes_skipped() {
        let home = temp_home("apple");
        mk_leaves(&home, "group.com.apple.notes", "Logs", &["n.log"]);
        mk_leaves(&home, "com.apple.foo", "Caches", &["c"]);
        let got = select_group_container_caches(&home).unwrap();
        assert!(got.paths.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn safari_extension_container_skipped_fail_closed() {
        let home = temp_home("safari");
        mk_leaves(&home, "group.com.example.ext", "Library/Caches", &["c"]);
        let cdir = home.join("Library/Containers/group.com.example.ext");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(cdir.join("SomethingSafariWebExtension"), b"x").unwrap();

        let got = select_group_container_caches(&home).unwrap();
        assert!(got.paths.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn safari_probe_unreadable_containers_fail_closed() {
        let home = temp_home("safari-deny");
        mk_leaves(&home, "group.com.example.ext", "Library/Caches", &["c"]);
        let cdir = home.join("Library/Containers/group.com.example.ext");
        fs::create_dir_all(&cdir).unwrap();
        fs::set_permissions(&cdir, fs::Permissions::from_mode(0o000)).unwrap();

        let got = select_group_container_caches(&home).unwrap();
        fs::set_permissions(&cdir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            got.paths.is_empty(),
            "unreadable Containers ⇒ skip container"
        );
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn symlink_container_and_leaf_skipped() {
        let home = temp_home("sym");
        let real = home.join("real-container");
        fs::create_dir_all(real.join("Caches")).unwrap();
        fs::write(real.join("Caches/c"), b"x").unwrap();
        std::os::unix::fs::symlink(
            &real,
            home.join("Library/Group Containers/group.com.example.app"),
        )
        .unwrap();

        let home2 = temp_home("sym-leaf");
        let caches = home2.join("Library/Group Containers/group.com.example.app/Caches");
        fs::create_dir_all(&caches).unwrap();
        let target = home2.join("outside");
        fs::write(&target, b"x").unwrap();
        std::os::unix::fs::symlink(&target, caches.join("c")).unwrap();

        assert!(select_group_container_caches(&home)
            .unwrap()
            .paths
            .is_empty());
        assert!(select_group_container_caches(&home2)
            .unwrap()
            .paths
            .is_empty());
        let _ = fs::remove_dir_all(&home);
        let _ = fs::remove_dir_all(&home2);
    }

    #[test]
    fn missing_root_ok_empty_unreadable_root_errors() {
        let bare = std::env::temp_dir().join(format!("vole-gcc-noroot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&bare);
        fs::create_dir_all(&bare).unwrap();
        assert!(select_group_container_caches(&bare)
            .unwrap()
            .paths
            .is_empty());
        let _ = fs::remove_dir_all(&bare);

        let home = temp_home("denied");
        let root = home.join("Library/Group Containers");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
        let got = select_group_container_caches(&home);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(got, Err(GroupCacheScanError::GroupContainersInaccessible));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn per_candidate_cap_skips_whole_tree_and_sets_truncated() {
        let home = temp_home("cap");
        let logs = home.join("Library/Group Containers/group.com.example.app/Logs");
        fs::create_dir_all(&logs).unwrap();
        for i in 0..(crate::groupcaches::MAX_LEAVES_PER_CANDIDATE + 1) {
            fs::write(logs.join(format!("f{i}")), b"x").unwrap();
        }
        mk_leaves(&home, "group.com.example.app", "tmp", &["only"]);

        let got = select_group_container_caches(&home).unwrap();
        assert!(got.truncated);
        assert!(
            !got.paths
                .iter()
                .any(|p| p.to_string_lossy().contains("/Logs/")),
            "over-cap tree must contribute zero leaves"
        );
        assert!(got.paths.iter().any(|p| p.ends_with("tmp/only")));
        assert!(!got.paths.iter().any(|p| p.ends_with("Logs")));
        let _ = fs::remove_dir_all(&home);
    }
}
