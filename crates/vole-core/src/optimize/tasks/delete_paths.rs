//! Delete-shaped optimize discoverers (saved state, Finder caches, …).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::delete::{measure_path_size_kb, PathSizeKb};
use crate::optimize::OptimizeTaskKind;
use crate::protection::{should_protect_path, ProtectionCatalog, ProtectionMode};

/// Aligns with Mole `MOLE_SAVED_STATE_AGE_DAYS`.
pub const SAVED_STATE_AGE_DAYS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizeCandidate {
    pub path: PathBuf,
    pub label: String,
    pub size: u64,
    pub task_id: &'static str,
    pub kind: OptimizeTaskKind,
}

fn candidate_size(path: &Path) -> u64 {
    match measure_path_size_kb(&path.display().to_string()) {
        PathSizeKb::Known(kb) => kb.saturating_mul(1024),
        PathSizeKb::Unknown => 0,
    }
}

fn is_old_enough(path: &Path, age_days: u64) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let threshold = SystemTime::now()
        .checked_sub(Duration::from_secs(age_days.saturating_mul(86_400)))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    modified <= threshold
}

pub fn discover_saved_state_cleanup(
    home: &Path,
    catalog: &ProtectionCatalog,
) -> Vec<OptimizeCandidate> {
    let state_dir = home.join("Library/Saved Application State");
    let Ok(entries) = fs::read_dir(&state_dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".savedState") || !path.is_dir() {
            continue;
        }
        if !is_old_enough(&path, SAVED_STATE_AGE_DAYS) {
            continue;
        }
        let path_str = path.display().to_string();
        if should_protect_path(&path_str, catalog, ProtectionMode::Cleanup) {
            continue;
        }
        out.push(OptimizeCandidate {
            size: candidate_size(&path),
            path,
            label: format!("Old saved state ({name})"),
            task_id: "saved_state_cleanup",
            kind: OptimizeTaskKind::Delete,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

const CACHE_REFRESH_TARGETS: &[&str] = &[
    "Library/Caches/com.apple.QuickLook.thumbnailcache",
    "Library/Caches/com.apple.iconservices.store",
    "Library/Caches/com.apple.iconservices",
];

pub fn discover_cache_refresh(home: &Path, catalog: &ProtectionCatalog) -> Vec<OptimizeCandidate> {
    let mut out = Vec::new();
    for rel in CACHE_REFRESH_TARGETS {
        let path = home.join(rel);
        if !path.exists() {
            continue;
        }
        let path_str = path.display().to_string();
        if should_protect_path(&path_str, catalog, ProtectionMode::Cleanup) {
            continue;
        }
        let leaf = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| rel.to_string());
        out.push(OptimizeCandidate {
            size: candidate_size(&path),
            path,
            label: format!("Finder cache ({leaf})"),
            task_id: "cache_refresh",
            kind: OptimizeTaskKind::Delete,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;

    use crate::protection::ProtectionCatalog;

    fn set_mtime_days_ago(path: &Path, days: u64) {
        let modified = SystemTime::now()
            .checked_sub(Duration::from_secs(days.saturating_mul(86_400)))
            .unwrap();
        let times = FileTimes::new().set_modified(modified);
        fs::File::open(path).unwrap().set_times(times).unwrap();
    }

    #[test]
    fn saved_state_skips_recent_and_lists_old() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let state = home.join("Library/Saved Application State");
        fs::create_dir_all(&state).unwrap();
        let old = state.join("com.example.old.savedState");
        let recent = state.join("com.example.recent.savedState");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&recent).unwrap();
        fs::write(old.join("windows.plist"), b"x").unwrap();
        fs::write(recent.join("windows.plist"), b"y").unwrap();
        set_mtime_days_ago(&old, 40);
        set_mtime_days_ago(&recent, 1);

        let catalog = ProtectionCatalog::embedded();
        let hits = discover_saved_state_cleanup(home, &catalog);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("com.example.old.savedState"));
        assert_eq!(hits[0].task_id, "saved_state_cleanup");
        assert_eq!(hits[0].kind, OptimizeTaskKind::Delete);
    }

    #[test]
    fn cache_refresh_lists_existing_targets_only() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let target = home.join("Library/Caches/com.apple.iconservices");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("blob"), b"cache").unwrap();

        let catalog = ProtectionCatalog::embedded();
        let hits = discover_cache_refresh(home, &catalog);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("com.apple.iconservices"));
        assert_eq!(hits[0].task_id, "cache_refresh");
    }
}
