//! 从 path candidates 过滤 orphan 路径。

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::protection::ProtectionCatalog;
use crate::rules::PathEntry;

use super::judge::{
    bundle_id_from_orphan_path, is_claude_vm_bundle_path, matches_orphan_name_prefix, OrphanJudge,
};
use super::{OrphanDeps, MAX_ORPHAN_ITERATIONS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanScanError {
    /// `~/Library/Caches` 不可读（FDA 等）。
    LibraryInaccessible,
}

/// 过滤出可标为 orphan 的路径（含 B4.1 Claude VM）。
pub fn select_orphaned_paths(
    entries: &[PathEntry],
    home: &Path,
    catalog: &ProtectionCatalog,
    deps: &dyn OrphanDeps,
    age_days: u32,
    claude_age_days: u32,
    now: SystemTime,
) -> Result<Vec<PathBuf>, OrphanScanError> {
    let caches = home.join("Library/Caches");
    if fs::read_dir(&caches).is_err() {
        return Err(OrphanScanError::LibraryInaccessible);
    }

    let installed = deps
        .scan_installed_bundle_ids(home)
        .map_err(|_| OrphanScanError::LibraryInaccessible)?;

    let judge = OrphanJudge {
        catalog,
        deps,
        installed: &installed,
        age_days,
        now,
    };

    let mut selected = Vec::new();
    let mut per_root: HashSet<String> = HashSet::new();
    // 用 root key 计数：Caches / Logs / States
    let mut counts: [usize; 3] = [0, 0, 0];

    for entry in entries {
        let Some(root_idx) = orphan_root_index(&entry.path, home) else {
            continue;
        };
        let Some(name) = entry.path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !matches_orphan_name_prefix(name) {
            continue;
        }
        if counts[root_idx] >= MAX_ORPHAN_ITERATIONS {
            continue;
        }
        counts[root_idx] += 1;

        if is_zero_size(&entry.path) {
            continue;
        }

        let Some(bundle_id) = bundle_id_from_orphan_path(&entry.path) else {
            continue;
        };

        let key = entry.path.to_string_lossy().into_owned();
        if !per_root.insert(key) {
            continue;
        }

        if judge.is_bundle_orphaned(&bundle_id, &entry.path, entry.mtime) {
            selected.push(entry.path.clone());
        }
    }

    selected.extend(select_claude_vm_bundles(
        home,
        &judge,
        claude_age_days,
        &mut per_root,
    ));

    Ok(selected)
}

/// 对齐 Mole：`find … -maxdepth 3 -name '*.bundle' -type d` under Claude Support。
fn select_claude_vm_bundles(
    home: &Path,
    judge: &OrphanJudge<'_>,
    claude_age_days: u32,
    seen: &mut HashSet<String>,
) -> Vec<PathBuf> {
    let root = home.join("Library/Application Support/Claude");
    if !root.is_dir() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut checked = 0usize;
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root, 0));

    while let Some((dir, depth)) = queue.pop_front() {
        if checked >= MAX_ORPHAN_ITERATIONS {
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            let Ok(meta) = ent.metadata() else {
                continue;
            };
            if !meta.is_dir() {
                continue;
            }
            let child_depth = depth + 1;
            let is_bundle = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".bundle"));

            if is_bundle {
                if child_depth > 3 {
                    continue;
                }
                checked += 1;
                if checked > MAX_ORPHAN_ITERATIONS {
                    break;
                }
                if !is_claude_vm_bundle_path(&path, home) {
                    continue;
                }
                if is_zero_size(&path) {
                    continue;
                }
                let key = path.to_string_lossy().into_owned();
                if !seen.insert(key) {
                    continue;
                }
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                if judge.is_claude_vm_bundle_orphaned(&path, mtime, claude_age_days) {
                    out.push(path);
                }
                continue;
            }

            if child_depth < 3 {
                queue.push_back((path, child_depth));
            }
        }
    }

    out
}

fn orphan_root_index(path: &Path, home: &Path) -> Option<usize> {
    let caches = home.join("Library/Caches");
    let logs = home.join("Library/Logs");
    let states = home.join("Library/Saved Application State");
    if path.starts_with(&caches) {
        Some(0)
    } else if path.starts_with(&logs) {
        Some(1)
    } else if path.starts_with(&states) {
        Some(2)
    } else {
        None
    }
}

fn is_zero_size(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return true;
    };
    if meta.is_file() {
        return meta.len() == 0;
    }
    if meta.is_dir() {
        return fs::read_dir(path)
            .map(|mut it| it.next().is_none())
            .unwrap_or(true);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orphan::{FakeOrphanDeps, CLAUDE_DESKTOP_BUNDLE_ID};
    use crate::protection::ProtectionCatalog;
    use std::collections::HashMap;
    use std::time::Duration;

    fn old_mtime() -> SystemTime {
        SystemTime::now() - Duration::from_secs(40 * 86400)
    }

    fn fresh_mtime() -> SystemTime {
        SystemTime::now() - Duration::from_secs(2 * 86400)
    }

    fn touch_mtime(path: &Path, mtime: SystemTime) {
        let f = fs::File::open(path).unwrap();
        f.set_modified(mtime).unwrap();
    }

    fn claude_deps_ok() -> FakeOrphanDeps {
        FakeOrphanDeps {
            spotlight: true,
            mdfind: HashMap::from([(CLAUDE_DESKTOP_BUNDLE_ID.into(), Ok(false))]),
            ..Default::default()
        }
    }

    fn make_claude_bundle(home: &Path, rel: &str, mtime: SystemTime) -> PathBuf {
        let bundle = home.join("Library/Application Support/Claude").join(rel);
        fs::create_dir_all(&bundle).unwrap();
        fs::write(bundle.join("rootfs.img"), b"vm data").unwrap();
        touch_mtime(&bundle, mtime);
        bundle
    }

    #[test]
    fn select_picks_old_uninstalled_cache() {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join("Library/Caches/com.gone.app");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("x"), b"data").unwrap();

        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            mdfind: HashMap::from([("com.gone.app".into(), Ok(false))]),
            scan_error: false,
            ..Default::default()
        };
        let entries = vec![PathEntry::new(cache.clone(), old_mtime())];
        let got = select_orphaned_paths(
            &entries,
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert_eq!(got, vec![cache]);
    }

    #[test]
    fn select_skips_dev_prefix() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/Caches")).unwrap();
        let cache = home.path().join("Library/Caches/dev.orbstack.OrbStack");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("x"), b"data").unwrap();

        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            mdfind: HashMap::new(),
            scan_error: false,
            ..Default::default()
        };
        let entries = vec![PathEntry::new(cache, old_mtime())];
        let got = select_orphaned_paths(
            &entries,
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn select_skips_fresh_mtime() {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join("Library/Caches/com.fresh.app");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("x"), b"data").unwrap();

        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            mdfind: HashMap::from([("com.fresh.app".into(), Ok(false))]),
            scan_error: false,
            ..Default::default()
        };
        let entries = vec![PathEntry::new(cache, fresh_mtime())];
        let got = select_orphaned_paths(
            &entries,
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn library_inaccessible_errors() {
        let home = tempfile::tempdir().unwrap();
        // 无 Library/Caches
        let deps = FakeOrphanDeps::default();
        let err = select_orphaned_paths(
            &[],
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap_err();
        assert_eq!(err, OrphanScanError::LibraryInaccessible);
    }

    #[test]
    fn select_picks_old_claude_vm_bundle() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/Caches")).unwrap();
        let bundle = make_claude_bundle(
            home.path(),
            "vm_bundles/claudevm.bundle",
            SystemTime::now() - Duration::from_secs(10 * 86400),
        );
        let deps = claude_deps_ok();
        let got = select_orphaned_paths(
            &[],
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert_eq!(got, vec![bundle]);
    }

    #[test]
    fn select_skips_claude_vm_when_running() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/Caches")).unwrap();
        let _ = make_claude_bundle(
            home.path(),
            "vm_bundles/claudevm.bundle",
            SystemTime::now() - Duration::from_secs(10 * 86400),
        );
        let deps = FakeOrphanDeps {
            claude_running: true,
            spotlight: true,
            mdfind: HashMap::from([(CLAUDE_DESKTOP_BUNDLE_ID.into(), Ok(false))]),
            ..Default::default()
        };
        let got = select_orphaned_paths(
            &[],
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn select_skips_claude_vm_when_installed() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/Caches")).unwrap();
        let _ = make_claude_bundle(
            home.path(),
            "vm_bundles/claudevm.bundle",
            SystemTime::now() - Duration::from_secs(10 * 86400),
        );
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::from([CLAUDE_DESKTOP_BUNDLE_ID.into()]),
            mdfind: HashMap::from([(CLAUDE_DESKTOP_BUNDLE_ID.into(), Ok(false))]),
            ..Default::default()
        };
        let got = select_orphaned_paths(
            &[],
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn select_skips_young_claude_vm() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/Caches")).unwrap();
        let _ = make_claude_bundle(
            home.path(),
            "vm_bundles/claudevm.bundle",
            SystemTime::now() - Duration::from_secs(86400),
        );
        let deps = claude_deps_ok();
        let got = select_orphaned_paths(
            &[],
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn select_skips_other_app_support_bundle() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/Caches")).unwrap();
        let other = home
            .path()
            .join("Library/Application Support/Other/x.bundle");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("rootfs.img"), b"x").unwrap();
        touch_mtime(&other, SystemTime::now() - Duration::from_secs(10 * 86400));
        let deps = claude_deps_ok();
        let got = select_orphaned_paths(
            &[],
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn select_skips_claude_vm_deeper_than_maxdepth() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/Caches")).unwrap();
        // Claude/a/b/c/d.bundle → relative depth 4 from Claude → excluded
        let _ = make_claude_bundle(
            home.path(),
            "a/b/c/d.bundle",
            SystemTime::now() - Duration::from_secs(10 * 86400),
        );
        let deps = claude_deps_ok();
        let got = select_orphaned_paths(
            &[],
            home.path(),
            &ProtectionCatalog::embedded(),
            &deps,
            30,
            7,
            SystemTime::now(),
        )
        .unwrap();
        assert!(got.is_empty());
    }
}
