//! Action-shaped optimize planners (quarantine, vacuum, defaults, dock, …).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::delete_paths::{is_optimize_protect_exempt, OptimizeCandidate};
use crate::delete::{measure_path_size_kb, PathSizeKb};
use crate::optimize::OptimizeTaskKind;
use crate::protection::{should_protect_path, ProtectionCatalog, ProtectionMode};

fn candidate_size(path: &Path) -> u64 {
    match measure_path_size_kb(&path.display().to_string()) {
        PathSizeKb::Known(kb) => kb.saturating_mul(1024),
        PathSizeKb::Unknown => 0,
    }
}

fn protected_for_optimize(path: &str, catalog: &ProtectionCatalog) -> bool {
    if is_optimize_protect_exempt(path) {
        return false;
    }
    should_protect_path(path, catalog, ProtectionMode::Cleanup)
}

fn action_sentinel(home: &Path, task_id: &'static str, label: &str) -> OptimizeCandidate {
    OptimizeCandidate {
        path: home.join(format!(".vole-optimize-action/{task_id}")),
        label: label.to_string(),
        size: 0,
        task_id,
        kind: OptimizeTaskKind::Action,
    }
}

fn sqlite3_available() -> bool {
    Command::new("sqlite3")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn sqlite_count(db: &Path, sql: &str) -> Option<u64> {
    let output = Command::new("sqlite3")
        .arg(db)
        .arg(sql)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim().parse().ok()
}

/// QuarantineEventsV2 cleanup when rows exist.
pub fn plan_quarantine_cleanup(
    home: &Path,
    catalog: &ProtectionCatalog,
) -> Option<OptimizeCandidate> {
    if !sqlite3_available() {
        return None;
    }
    let db = home.join("Library/Preferences/com.apple.LaunchServices.QuarantineEventsV2");
    if !db.is_file() {
        return None;
    }
    let path_str = db.display().to_string();
    if protected_for_optimize(&path_str, catalog) {
        return None;
    }
    let count = sqlite_count(&db, "SELECT COUNT(*) FROM LSQuarantineEvent;")?;
    if count == 0 {
        return None;
    }
    Some(OptimizeCandidate {
        size: candidate_size(&db),
        path: db,
        label: format!("Quarantine history ({count} entries)"),
        task_id: "quarantine_cleanup",
        kind: OptimizeTaskKind::Action,
    })
}

fn app_running(name: &str) -> bool {
    Command::new("pgrep")
        .args(["-x", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn plan_sqlite_vacuum(home: &Path, catalog: &ProtectionCatalog) -> Vec<OptimizeCandidate> {
    if !sqlite3_available() {
        return Vec::new();
    }
    let mut out = Vec::new();

    let fixed = [
        ("Messages", "Library/Messages/chat.db"),
        ("Safari", "Library/Safari/History.db"),
        ("Safari", "Library/Safari/TopSites.db"),
    ];
    for (app, rel) in fixed {
        if app_running(app) {
            continue;
        }
        let path = home.join(rel);
        if !path.is_file() {
            continue;
        }
        let path_str = path.display().to_string();
        if protected_for_optimize(&path_str, catalog) {
            continue;
        }
        out.push(OptimizeCandidate {
            size: candidate_size(&path),
            path,
            label: format!("Vacuum {app} database"),
            task_id: "sqlite_vacuum",
            kind: OptimizeTaskKind::Action,
        });
    }

    // Mail Envelope Index* under Library/Mail/V*/MailData/
    if !app_running("Mail") {
        let mail = home.join("Library/Mail");
        if mail.is_dir() {
            for entry in jwalk::WalkDir::new(&mail).skip_hidden(false).into_iter().flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with("Envelope Index") || !entry.file_type().is_file() {
                    continue;
                }
                if path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    != Some("MailData")
                {
                    continue;
                }
                let path_str = path.display().to_string();
                if protected_for_optimize(&path_str, catalog) {
                    continue;
                }
                out.push(OptimizeCandidate {
                    size: candidate_size(&path),
                    path,
                    label: "Vacuum Mail Envelope Index".into(),
                    task_id: "sqlite_vacuum",
                    kind: OptimizeTaskKind::Action,
                });
            }
        }
    }

    out
}

fn defaults_read(domain: &str, key: &str) -> Option<String> {
    let output = Command::new("defaults")
        .args(["read", domain, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn defaults_is_truthy(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}

pub fn plan_prevent_network_dsstore(home: &Path) -> Option<OptimizeCandidate> {
    let domain = "com.apple.desktopservices";
    let keys = ["DSDontWriteNetworkStores", "DSDontWriteUSBStores"];
    let needs = keys.iter().any(|key| {
        defaults_read(domain, key)
            .map(|v| !defaults_is_truthy(&v))
            .unwrap_or(true)
    });
    if !needs {
        return None;
    }
    Some(action_sentinel(
        home,
        "prevent_network_dsstore",
        "Enable .DS_Store prevention on network & USB volumes",
    ))
}

pub fn plan_legacy_overrides_audit(home: &Path) -> Vec<OptimizeCandidate> {
    let mut out = Vec::new();
    if let Some(v) = defaults_read("-g", "NSAppSleepDisabled") {
        if defaults_is_truthy(&v) {
            out.push(OptimizeCandidate {
                path: home.join("Library/Preferences/.GlobalPreferences.plist"),
                label: "Remove NSAppSleepDisabled override".into(),
                size: 0,
                task_id: "legacy_overrides_audit",
                kind: OptimizeTaskKind::Action,
            });
        }
    }
    for key in ["skip-verify", "skip-verify-locked", "skip-verify-remote"] {
        if let Some(v) = defaults_read("com.apple.frameworks.diskimages", key) {
            if defaults_is_truthy(&v) {
                out.push(OptimizeCandidate {
                    path: home
                        .join("Library/Preferences/com.apple.frameworks.diskimages.plist"),
                    label: format!("Remove diskimages override ({key})"),
                    size: 0,
                    task_id: "legacy_overrides_audit",
                    kind: OptimizeTaskKind::Action,
                });
            }
        }
    }
    out
}

const NOTIFICATION_MIN_BYTES: u64 = 50 * 1024 * 1024;
const COREDUET_MIN_BYTES: u64 = 100 * 1024 * 1024;

fn darwin_user_dir() -> Option<PathBuf> {
    let output = Command::new("getconf")
        .arg("DARWIN_USER_DIR")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    }
}

pub fn plan_notification_cleanup(
    home: &Path,
    catalog: &ProtectionCatalog,
) -> Option<OptimizeCandidate> {
    let _ = home;
    if !sqlite3_available() {
        return None;
    }
    let db = darwin_user_dir()?.join("com.apple.notificationcenter/db2/db");
    if !db.is_file() {
        return None;
    }
    let size = candidate_size(&db);
    if size < NOTIFICATION_MIN_BYTES {
        return None;
    }
    let path_str = db.display().to_string();
    if protected_for_optimize(&path_str, catalog) {
        return None;
    }
    Some(OptimizeCandidate {
        path: db,
        label: format!("Notification Center DB ({size} bytes)"),
        size,
        task_id: "notification_cleanup",
        kind: OptimizeTaskKind::Action,
    })
}

pub fn plan_coreduet_cleanup(home: &Path, catalog: &ProtectionCatalog) -> Option<OptimizeCandidate> {
    if !sqlite3_available() {
        return None;
    }
    let db = home.join("Library/Application Support/Knowledge/knowledgeC.db");
    if !db.is_file() {
        return None;
    }
    let mut total = candidate_size(&db);
    for suffix in ["-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{suffix}", db.display()));
        if p.is_file() {
            total = total.saturating_add(candidate_size(&p));
        }
    }
    if total < COREDUET_MIN_BYTES {
        return None;
    }
    let path_str = db.display().to_string();
    if protected_for_optimize(&path_str, catalog) {
        return None;
    }
    Some(OptimizeCandidate {
        path: db,
        label: format!("Knowledge / CoreDuet DB ({total} bytes)"),
        size: total,
        task_id: "coreduet_cleanup",
        kind: OptimizeTaskKind::Action,
    })
}

pub fn plan_dock_refresh(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "dock_refresh", "Refresh Dock")
}

pub fn plan_launch_services_rebuild(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "launch_services_rebuild", "Rebuild LaunchServices database")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protection::ProtectionCatalog;

    #[test]
    fn quarantine_plans_when_rows_exist() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let prefs = home.join("Library/Preferences");
        fs::create_dir_all(&prefs).unwrap();
        let db = prefs.join("com.apple.LaunchServices.QuarantineEventsV2");
        let status = Command::new("sqlite3")
            .arg(&db)
            .arg(
                "CREATE TABLE LSQuarantineEvent (LSQuarantineEventIdentifier TEXT); \
                 INSERT INTO LSQuarantineEvent VALUES ('a');",
            )
            .status()
            .expect("sqlite3");
        assert!(status.success());

        let catalog = ProtectionCatalog::embedded();
        let hit = plan_quarantine_cleanup(home, &catalog).expect("planned");
        assert_eq!(hit.task_id, "quarantine_cleanup");
        assert_eq!(hit.kind, OptimizeTaskKind::Action);
    }

    #[test]
    fn dock_and_ls_sentinels() {
        let home = Path::new("/tmp");
        assert_eq!(plan_dock_refresh(home).task_id, "dock_refresh");
        assert_eq!(
            plan_launch_services_rebuild(home).task_id,
            "launch_services_rebuild"
        );
    }

    #[test]
    fn defaults_truthy_helper() {
        assert!(defaults_is_truthy("1"));
        assert!(defaults_is_truthy("TRUE"));
        assert!(!defaults_is_truthy("0"));
    }
}
