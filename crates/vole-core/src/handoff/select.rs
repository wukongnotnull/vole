//! Handoff pasteboard 扫描与 apply 政策重验。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{
    handoff_pasteboard_root, is_handoff_pasteboard_leaf_path, HANDOFF_MTIME_MINUTES,
    MAX_HANDOFF_LEAVES,
};

#[derive(Debug, PartialEq, Eq)]
pub enum HandoffScanError {
    /// `shared-pasteboard` 根存在但不可列。
    RootInaccessible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffSelectResult {
    pub paths: Vec<PathBuf>,
    pub truncated: bool,
}

pub fn select_handoff_pasteboard(
    home: &Path,
    now: SystemTime,
) -> Result<HandoffSelectResult, HandoffScanError> {
    let root = handoff_pasteboard_root(home);
    if !root.exists() {
        return Ok(HandoffSelectResult {
            paths: Vec::new(),
            truncated: false,
        });
    }
    let meta = fs::symlink_metadata(&root).map_err(|_| HandoffScanError::RootInaccessible)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Ok(HandoffSelectResult {
            paths: Vec::new(),
            truncated: false,
        });
    }
    let rd = fs::read_dir(&root).map_err(|_| HandoffScanError::RootInaccessible)?;

    let min_age = Duration::from_secs(HANDOFF_MTIME_MINUTES * 60);
    let mut out = Vec::new();
    let mut truncated = false;

    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(m) = fs::symlink_metadata(&path) else {
            continue;
        };
        if m.file_type().is_symlink() {
            continue;
        }
        let Ok(mtime) = m.modified() else {
            continue;
        };
        let Ok(age) = now.duration_since(mtime) else {
            continue; // clock skew / future mtime → skip
        };
        if age <= min_age {
            continue;
        }
        if out.len() >= MAX_HANDOFF_LEAVES {
            truncated = true;
            break;
        }
        out.push(path);
    }

    out.sort();
    Ok(HandoffSelectResult {
        paths: out,
        truncated,
    })
}

/// apply 政策重验：单层根下 + 非 symlink + mtime>60min（非 protect 豁免）。
pub fn recheck_handoff_pasteboard_entry(path: &Path, home: &Path, now: SystemTime) -> bool {
    if !is_handoff_pasteboard_leaf_path(path, home) {
        return false;
    }
    let Ok(m) = fs::symlink_metadata(path) else {
        return false;
    };
    if m.file_type().is_symlink() {
        return false;
    }
    let Ok(mtime) = m.modified() else {
        return false;
    };
    match now.duration_since(mtime) {
        Ok(age) => age > Duration::from_secs(HANDOFF_MTIME_MINUTES * 60),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    fn temp_home(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("vole-handoff-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(handoff_pasteboard_root(&root)).unwrap();
        root
    }

    fn set_mtime(path: &Path, when: SystemTime) {
        filetime::set_file_mtime(path, filetime::FileTime::from_system_time(when)).unwrap();
    }

    #[test]
    fn selects_only_older_than_60_minutes() {
        let home = temp_home("age");
        let root = handoff_pasteboard_root(&home);
        let old = root.join("old");
        let fresh = root.join("fresh");
        fs::write(&old, b"o").unwrap();
        fs::write(&fresh, b"f").unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000_000);
        set_mtime(&old, now - Duration::from_secs(61 * 60));
        set_mtime(&fresh, now - Duration::from_secs(30 * 60));

        let got = select_handoff_pasteboard(&home, now).unwrap();
        assert!(!got.truncated);
        assert_eq!(got.paths, vec![old]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn skips_symlink_leaf_and_symlink_root() {
        let home = temp_home("sym");
        let root = handoff_pasteboard_root(&home);
        let target = home.join("outside");
        fs::write(&target, b"x").unwrap();
        std::os::unix::fs::symlink(&target, root.join("link")).unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000_000);
        assert!(select_handoff_pasteboard(&home, now)
            .unwrap()
            .paths
            .is_empty());
        let _ = fs::remove_dir_all(&home);

        let home2 = temp_home("symroot");
        let real = home2.join("real-pb");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("old"), b"x").unwrap();
        set_mtime(
            &real.join("old"),
            SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        );
        let link_root = handoff_pasteboard_root(&home2);
        let _ = fs::remove_dir_all(&link_root);
        std::os::unix::fs::symlink(&real, &link_root).unwrap();
        assert!(select_handoff_pasteboard(&home2, SystemTime::now())
            .unwrap()
            .paths
            .is_empty());
        let _ = fs::remove_dir_all(&home2);
    }

    #[test]
    fn missing_root_empty_unreadable_errors() {
        let bare = std::env::temp_dir().join(format!("vole-handoff-noroot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&bare);
        fs::create_dir_all(&bare).unwrap();
        assert!(select_handoff_pasteboard(&bare, SystemTime::now())
            .unwrap()
            .paths
            .is_empty());
        let _ = fs::remove_dir_all(&bare);

        let home = temp_home("denied");
        let root = handoff_pasteboard_root(&home);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
        let got = select_handoff_pasteboard(&home, SystemTime::now());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(got, Err(HandoffScanError::RootInaccessible));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn cap_sets_truncated() {
        let home = temp_home("cap");
        let root = handoff_pasteboard_root(&home);
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000_000);
        let old = now - Duration::from_secs(61 * 60);
        for i in 0..(MAX_HANDOFF_LEAVES + 5) {
            let p = root.join(format!("f{i:04}"));
            fs::write(&p, b"x").unwrap();
            set_mtime(&p, old);
        }
        let got = select_handoff_pasteboard(&home, now).unwrap();
        assert!(got.truncated);
        assert_eq!(got.paths.len(), MAX_HANDOFF_LEAVES);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn recheck_rejects_fresh_mtime_and_outside_root() {
        let home = temp_home("recheck");
        let root = handoff_pasteboard_root(&home);
        let old = root.join("old");
        fs::write(&old, b"o").unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000_000);
        set_mtime(&old, now - Duration::from_secs(61 * 60));
        assert!(recheck_handoff_pasteboard_entry(&old, &home, now));

        set_mtime(&old, now - Duration::from_secs(10 * 60));
        assert!(!recheck_handoff_pasteboard_entry(&old, &home, now));

        let outside = home
            .join("Library/Group Containers/group.com.apple.coreservices.useractivityd/other");
        fs::create_dir_all(outside.parent().unwrap()).unwrap();
        fs::write(&outside, b"x").unwrap();
        set_mtime(&outside, now - Duration::from_secs(61 * 60));
        assert!(!recheck_handoff_pasteboard_entry(&outside, &home, now));
        let _ = fs::remove_dir_all(&home);
    }
}
