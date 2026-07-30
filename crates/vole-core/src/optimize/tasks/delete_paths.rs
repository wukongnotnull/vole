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

/// Mole optimize intentionally targets a few Apple-owned regenerable DBs.
/// `should_protect_path` would otherwise block them via `com.apple.*` filename guards
/// (Mole bats stub `should_protect_path` for the same reason).
pub(crate) fn is_optimize_protect_exempt(path: &str) -> bool {
    path.ends_with("/Library/Preferences/com.apple.LaunchServices.QuarantineEventsV2")
        || path.contains("/Application Support/Knowledge/knowledgeC.db")
        || path.contains("/com.apple.notificationcenter/db2/db")
}

fn protected_for_optimize(path: &str, catalog: &ProtectionCatalog) -> bool {
    if is_optimize_protect_exempt(path) {
        return false;
    }
    should_protect_path(path, catalog, ProtectionMode::Cleanup)
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
        if protected_for_optimize(&path_str, catalog) {
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
        if protected_for_optimize(&path_str, catalog) {
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

fn preference_plist_is_protected(filename: &str, protect_loginwindow: bool) -> bool {
    if filename.starts_with("com.apple.") || filename.starts_with(".GlobalPreferences") {
        return true;
    }
    if filename == "loginwindow.plist" {
        return protect_loginwindow;
    }
    false
}

fn plutil_lint_ok(path: &Path) -> bool {
    std::process::Command::new("plutil")
        .arg("-lint")
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

fn collect_plists_shallow(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("plist") && path.is_file() {
            out.push(path);
        }
    }
    out
}

fn collect_plists_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in jwalk::WalkDir::new(dir)
        .skip_hidden(false)
        .into_iter()
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("plist") {
            continue;
        }
        if entry.file_type().is_file() {
            out.push(path);
        }
    }
    out
}

/// Discover corrupted non-Apple preference plists (Mole `fix_broken_preferences` discover phase).
pub fn discover_fix_broken_configs(
    home: &Path,
    catalog: &ProtectionCatalog,
) -> Vec<OptimizeCandidate> {
    let prefs = home.join("Library/Preferences");
    let by_host = prefs.join("ByHost");
    let mut out = Vec::new();

    let scans: [(&Path, bool, bool); 2] = [
        (prefs.as_path(), true, true),     // shallow, protect loginwindow
        (by_host.as_path(), false, false), // recursive, don't protect loginwindow
    ];

    for (dir, shallow, protect_login) in scans {
        if !dir.is_dir() {
            continue;
        }
        let paths = if shallow {
            collect_plists_shallow(dir)
        } else {
            collect_plists_recursive(dir)
        };
        for path in paths {
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_owned(),
                None => continue,
            };
            if preference_plist_is_protected(&name, protect_login) {
                continue;
            }
            if plutil_lint_ok(&path) {
                continue;
            }
            let path_str = path.display().to_string();
            if protected_for_optimize(&path_str, catalog) {
                continue;
            }
            out.push(OptimizeCandidate {
                size: candidate_size(&path),
                path,
                label: format!("Broken preference ({name})"),
                task_id: "fix_broken_configs",
                kind: OptimizeTaskKind::Delete,
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn launch_agent_program(plist_path: &Path) -> Option<String> {
    let data = fs::read(plist_path).ok()?;
    let value = plist::Value::from_reader(std::io::Cursor::new(data)).ok()?;
    let dict = value.as_dictionary()?;
    if let Some(args) = dict.get("ProgramArguments").and_then(|v| v.as_array()) {
        if let Some(first) = args.first().and_then(|v| v.as_string()) {
            return Some(first.to_string());
        }
    }
    dict.get("Program")
        .and_then(|v| v.as_string())
        .map(str::to_string)
}

fn launch_agent_volume_mounted(binary: &str) -> bool {
    let Some(rest) = binary.strip_prefix("/Volumes/") else {
        return true;
    };
    let vol = rest.split('/').next().unwrap_or("");
    if vol.is_empty() {
        return true;
    }
    Path::new("/Volumes").join(vol).is_dir()
}

pub fn discover_launch_agents_cleanup(
    home: &Path,
    catalog: &ProtectionCatalog,
) -> Vec<OptimizeCandidate> {
    let agents = home.join("Library/LaunchAgents");
    let Ok(entries) = fs::read_dir(&agents) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("plist") {
            continue;
        }
        let Some(binary) = launch_agent_program(&path) else {
            continue;
        };
        if !binary.starts_with('/') || Path::new(&binary).exists() {
            continue;
        }
        if !launch_agent_volume_mounted(&binary) {
            continue;
        }
        let path_str = path.display().to_string();
        if protected_for_optimize(&path_str, catalog) {
            continue;
        }
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "agent.plist".into());
        out.push(OptimizeCandidate {
            size: candidate_size(&path),
            path,
            label: format!("Broken LaunchAgent ({name})"),
            task_id: "launch_agents_cleanup",
            kind: OptimizeTaskKind::Delete,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
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

    #[test]
    fn fix_broken_configs_lists_corrupt_non_apple_plist() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let prefs = home.join("Library/Preferences");
        fs::create_dir_all(&prefs).unwrap();
        let bad = prefs.join("com.example.broken.plist");
        fs::write(&bad, b"<?xml").unwrap();
        let apple = prefs.join("com.apple.Safari.plist");
        fs::write(&apple, b"<?xml").unwrap();

        let catalog = ProtectionCatalog::embedded();
        let hits = discover_fix_broken_configs(home, &catalog);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("com.example.broken.plist"));
    }

    #[test]
    fn launch_agents_cleanup_detects_missing_absolute_program() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let agents = home.join("Library/LaunchAgents");
        fs::create_dir_all(&agents).unwrap();
        let plist = agents.join("com.example.missing.plist");
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>ProgramArguments</key>
<array><string>/tmp/vole-optimize-missing-bin-xyz</string></array>
</dict></plist>"#;
        fs::write(&plist, body).unwrap();

        let catalog = ProtectionCatalog::embedded();
        let hits = discover_launch_agents_cleanup(home, &catalog);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("com.example.missing.plist"));
    }
}
