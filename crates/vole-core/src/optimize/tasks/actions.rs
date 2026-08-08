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
    let output = Command::new("sqlite3").arg(db).arg(sql).output().ok()?;
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
            for entry in jwalk::WalkDir::new(&mail)
                .skip_hidden(false)
                .into_iter()
                .flatten()
            {
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
    matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes")
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
                    path: home.join("Library/Preferences/com.apple.frameworks.diskimages.plist"),
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

pub fn plan_coreduet_cleanup(
    home: &Path,
    catalog: &ProtectionCatalog,
) -> Option<OptimizeCandidate> {
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
    action_sentinel(
        home,
        "launch_services_rebuild",
        "Rebuild LaunchServices database",
    )
}

pub fn plan_system_maintenance(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "system_maintenance", "DNS & Spotlight Check")
}

pub fn plan_network_optimization(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "network_optimization", "Network Cache Refresh")
}

pub fn plan_memory_pressure_relief(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "memory_pressure_relief", "Memory Optimization")
}

pub fn plan_network_stack_optimize(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "network_stack_optimize", "Network Stack Refresh")
}

pub fn plan_disk_permissions_repair(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "disk_permissions_repair", "Permission Repair")
}

pub fn plan_periodic_maintenance(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "periodic_maintenance", "Periodic Maintenance")
}

fn env_flag_tri(name: &str) -> Option<bool> {
    let Ok(v) = std::env::var(name) else {
        return None;
    };
    match v.trim() {
        "1" | "true" | "TRUE" | "yes" | "YES" => Some(true),
        "0" | "false" | "FALSE" | "no" | "NO" => Some(false),
        _ => None,
    }
}

/// 对齐 Mole `has_active_vpn_interface`；`VOLE_TEST_VPN_ACTIVE=1|0` 强制。
pub fn has_active_vpn() -> bool {
    if let Some(v) = env_flag_tri("VOLE_TEST_VPN_ACTIVE") {
        return v;
    }
    if Command::new("scutil")
        .args(["--nc", "list"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.contains("* (Connected)"))
        })
        .unwrap_or(false)
    {
        return true;
    }
    let Ok(out) = Command::new("route")
        .args(["-n", "get", "default"])
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(iface) = trimmed.strip_prefix("interface:") {
            let iface = iface.trim();
            if iface.starts_with("utun")
                && iface
                    .as_bytes()
                    .get(4..)
                    .is_some_and(|rest| !rest.is_empty() && rest.iter().all(|b| b.is_ascii_digit()))
            {
                return true;
            }
        }
    }
    false
}

/// 默认路由或 DNS 探测失败 → 需要 flush；`VOLE_TEST_NETWORK_STACK_UNHEALTHY=1|0` 强制。
pub fn network_stack_needs_flush() -> bool {
    if let Some(v) = env_flag_tri("VOLE_TEST_NETWORK_STACK_UNHEALTHY") {
        return v;
    }
    let route_ok = Command::new("route")
        .args(["-n", "get", "default"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let dns_ok = Command::new("dscacheutil")
        .args(["-q", "host", "-a", "name", "example.com"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    !(route_ok && dns_ok)
}

/// 对齐 Mole `needs_permissions_repair`；`VOLE_TEST_DISK_PERMISSIONS_NEED_REPAIR=1|0` 强制。
pub fn needs_disk_permissions_repair(home: &Path) -> bool {
    if let Some(v) = env_flag_tri("VOLE_TEST_DISK_PERMISSIONS_NEED_REPAIR") {
        return v;
    }
    if let Ok(out) = Command::new("stat").args(["-f", "%Su"]).arg(home).output() {
        if out.status.success() {
            let owner = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Ok(user) = std::env::var("USER") {
                if !owner.is_empty() && owner != user {
                    return true;
                }
            }
        }
    }
    for rel in ["", "Library", "Library/Preferences"] {
        let p = if rel.is_empty() {
            home.to_path_buf()
        } else {
            home.join(rel)
        };
        if p.exists() {
            let probe = p.join(".vole-perm-probe");
            match fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&probe)
            {
                Ok(_) => {
                    let _ = fs::remove_file(&probe);
                }
                Err(_) => return true,
            }
        }
    }
    false
}

fn periodic_command_available() -> bool {
    if let Some(v) = env_flag_tri("VOLE_TEST_PERIODIC_AVAILABLE") {
        return v;
    }
    Command::new("periodic")
        .arg("-h")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

fn periodic_log_path() -> PathBuf {
    if let Ok(p) = std::env::var("VOLE_TEST_PERIODIC_LOG") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    PathBuf::from("/var/log/daily.out")
}

/// `periodic` 可用且 daily 日志缺失或年龄 ≥7 天；`VOLE_TEST_PERIODIC_STALE=1|0` 强制。
pub fn periodic_needs_run() -> bool {
    if !periodic_command_available() {
        return false;
    }
    if let Some(v) = env_flag_tri("VOLE_TEST_PERIODIC_STALE") {
        return v;
    }
    let log = periodic_log_path();
    let Ok(meta) = fs::metadata(&log) else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return true;
    };
    let Ok(age) = std::time::SystemTime::now().duration_since(modified) else {
        return true;
    };
    age.as_secs() >= 7 * 86400
}

/// sentinel `home/.vole-optimize-action/<task>` → `home`
pub fn optimize_action_home(path: &Path) -> PathBuf {
    path.parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// 对齐 Mole `is_memory_pressure_high`：`memory_pressure -Q` 含 warning/critical。
/// `VOLE_TEST_MEMORY_PRESSURE=1|0` 强制高压/低压。
pub fn is_memory_pressure_high() -> bool {
    if let Some(v) = env_flag_tri("VOLE_TEST_MEMORY_PRESSURE") {
        return v;
    }
    let Ok(out) = Command::new("memory_pressure")
        .arg("-Q")
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
    text.contains("warning") || text.contains("critical")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizeActionError {
    Failed,
    Skipped,
    NeedsPrivilege,
}

/// Apply a single `optimize:action:*` entry.
pub fn apply_optimize_action(
    task_id: &str,
    path: &Path,
    privilege: Option<&dyn crate::privilege::PrivilegeBackend>,
    dns_flushed: &mut bool,
) -> Result<(), OptimizeActionError> {
    match task_id {
        "quarantine_cleanup" => apply_quarantine(path),
        "sqlite_vacuum" => apply_vacuum(path),
        "prevent_network_dsstore" => apply_prevent_dsstore(),
        "legacy_overrides_audit" => apply_legacy_override(path),
        "notification_cleanup" => apply_notification(path),
        "coreduet_cleanup" => apply_coreduet(path),
        "dock_refresh" => apply_dock(),
        "launch_services_rebuild" => apply_launch_services(),
        "system_maintenance" | "network_optimization" => {
            apply_dns_optimize(task_id, privilege, dns_flushed)
        }
        "memory_pressure_relief" => apply_memory_pressure_relief(privilege),
        "network_stack_optimize" => apply_network_stack_optimize(privilege),
        "disk_permissions_repair" => apply_disk_permissions_repair(path, privilege),
        "periodic_maintenance" => apply_periodic_maintenance(privilege),
        "login_items_audit" => apply_login_items_audit(path),
        "spotlight_orphan_rules_cleanup" => apply_spotlight_orphan_rules_cleanup(),
        _ => Err(OptimizeActionError::Failed),
    }
}

fn apply_login_items_audit(path: &Path) -> Result<(), OptimizeActionError> {
    use super::login_items_audit::is_unavailable_audit_path;

    if is_unavailable_audit_path(path) {
        return Err(OptimizeActionError::NeedsPrivilege);
    }
    // Report-only acknowledge; never delete login items / never touch osascript/sudo.
    Ok(())
}

fn apply_spotlight_orphan_rules_cleanup() -> Result<(), OptimizeActionError> {
    use super::spotlight_orphan_rules::{
        run_spotlight_orphan_rules_cleanup, LiveSpotlightOrphanDeps, SpotlightOrphanError,
    };

    match run_spotlight_orphan_rules_cleanup(&LiveSpotlightOrphanDeps) {
        Ok(()) => Ok(()),
        Err(SpotlightOrphanError::TestMode) => Err(OptimizeActionError::Skipped),
        Err(SpotlightOrphanError::Unavailable) => Err(OptimizeActionError::Failed),
    }
}

fn apply_memory_pressure_relief(
    privilege: Option<&dyn crate::privilege::PrivilegeBackend>,
) -> Result<(), OptimizeActionError> {
    use crate::privilege::PrivilegeError;

    if !is_memory_pressure_high() {
        return Ok(());
    }
    let Some(backend) = privilege else {
        return Err(OptimizeActionError::NeedsPrivilege);
    };
    match backend.purge_inactive_memory() {
        Ok(()) => Ok(()),
        Err(PrivilegeError::Unavailable) | Err(PrivilegeError::Refused) => {
            Err(OptimizeActionError::NeedsPrivilege)
        }
        Err(PrivilegeError::CommandFailed(_)) => Err(OptimizeActionError::Failed),
    }
}

fn map_privilege_result(
    r: Result<(), crate::privilege::PrivilegeError>,
) -> Result<(), OptimizeActionError> {
    use crate::privilege::PrivilegeError;
    match r {
        Ok(()) => Ok(()),
        Err(PrivilegeError::Unavailable) | Err(PrivilegeError::Refused) => {
            Err(OptimizeActionError::NeedsPrivilege)
        }
        Err(PrivilegeError::CommandFailed(_)) => Err(OptimizeActionError::Failed),
    }
}

fn apply_network_stack_optimize(
    privilege: Option<&dyn crate::privilege::PrivilegeBackend>,
) -> Result<(), OptimizeActionError> {
    if has_active_vpn() {
        return Ok(());
    }
    if !network_stack_needs_flush() {
        return Ok(());
    }
    let Some(backend) = privilege else {
        return Err(OptimizeActionError::NeedsPrivilege);
    };
    map_privilege_result(backend.flush_network_stack())
}

fn apply_disk_permissions_repair(
    path: &Path,
    privilege: Option<&dyn crate::privilege::PrivilegeBackend>,
) -> Result<(), OptimizeActionError> {
    let home = optimize_action_home(path);
    if !needs_disk_permissions_repair(&home) {
        return Ok(());
    }
    let Some(backend) = privilege else {
        return Err(OptimizeActionError::NeedsPrivilege);
    };
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(0);
    map_privilege_result(backend.reset_user_permissions(uid))
}

fn apply_periodic_maintenance(
    privilege: Option<&dyn crate::privilege::PrivilegeBackend>,
) -> Result<(), OptimizeActionError> {
    if !periodic_needs_run() {
        return Ok(());
    }
    let Some(backend) = privilege else {
        return Err(OptimizeActionError::NeedsPrivilege);
    };
    map_privilege_result(backend.run_periodic_maintenance())
}

fn apply_dns_optimize(
    task_id: &str,
    privilege: Option<&dyn crate::privilege::PrivilegeBackend>,
    dns_flushed: &mut bool,
) -> Result<(), OptimizeActionError> {
    use crate::privilege::PrivilegeError;

    if !*dns_flushed {
        let Some(backend) = privilege else {
            return Err(OptimizeActionError::NeedsPrivilege);
        };
        match backend.flush_dns_cache() {
            Ok(()) => *dns_flushed = true,
            Err(PrivilegeError::Unavailable) | Err(PrivilegeError::Refused) => {
                return Err(OptimizeActionError::NeedsPrivilege);
            }
            Err(PrivilegeError::CommandFailed(_)) => return Err(OptimizeActionError::Failed),
        }
    }
    if task_id == "system_maintenance" {
        let _ = Command::new("mdutil").args(["-s", "/"]).output();
    }
    Ok(())
}

fn apply_quarantine(db: &Path) -> Result<(), OptimizeActionError> {
    let status = Command::new("sqlite3")
        .arg(db)
        .arg("DELETE FROM LSQuarantineEvent; VACUUM;")
        .status()
        .map_err(|_| OptimizeActionError::Failed)?;
    if status.success() {
        Ok(())
    } else {
        Err(OptimizeActionError::Failed)
    }
}

fn apply_vacuum(db: &Path) -> Result<(), OptimizeActionError> {
    let integrity = Command::new("sqlite3")
        .arg(db)
        .arg("PRAGMA integrity_check;")
        .output()
        .map_err(|_| OptimizeActionError::Failed)?;
    if !integrity.status.success() {
        return Err(OptimizeActionError::Skipped);
    }
    let text = String::from_utf8_lossy(&integrity.stdout);
    if !text.trim().eq_ignore_ascii_case("ok") {
        return Err(OptimizeActionError::Skipped);
    }
    let status = Command::new("sqlite3")
        .arg(db)
        .arg("VACUUM;")
        .status()
        .map_err(|_| OptimizeActionError::Failed)?;
    if status.success() {
        Ok(())
    } else {
        Err(OptimizeActionError::Failed)
    }
}

fn apply_prevent_dsstore() -> Result<(), OptimizeActionError> {
    let domain = "com.apple.desktopservices";
    for key in ["DSDontWriteNetworkStores", "DSDontWriteUSBStores"] {
        let status = Command::new("defaults")
            .args(["write", domain, key, "-bool", "true"])
            .status()
            .map_err(|_| OptimizeActionError::Failed)?;
        if !status.success() {
            return Err(OptimizeActionError::Failed);
        }
    }
    Ok(())
}

fn apply_legacy_override(path: &Path) -> Result<(), OptimizeActionError> {
    let path_str = path.display().to_string();
    if path_str.ends_with(".GlobalPreferences.plist") {
        let status = Command::new("defaults")
            .args(["delete", "-g", "NSAppSleepDisabled"])
            .status()
            .map_err(|_| OptimizeActionError::Failed)?;
        return if status.success() {
            Ok(())
        } else {
            Err(OptimizeActionError::Skipped)
        };
    }
    if path_str.contains("com.apple.frameworks.diskimages") {
        for key in ["skip-verify", "skip-verify-locked", "skip-verify-remote"] {
            let _ = Command::new("defaults")
                .args(["delete", "com.apple.frameworks.diskimages", key])
                .status();
        }
        return Ok(());
    }
    Err(OptimizeActionError::Skipped)
}

fn apply_notification(db: &Path) -> Result<(), OptimizeActionError> {
    let status = Command::new("sqlite3")
        .arg(db)
        .arg("DELETE FROM record WHERE delivered_date < strftime('%s','now','-30 days'); VACUUM;")
        .status()
        .map_err(|_| OptimizeActionError::Failed)?;
    if status.success() {
        let _ = Command::new("killall").arg("NotificationCenter").status();
        Ok(())
    } else {
        Err(OptimizeActionError::Skipped)
    }
}

fn apply_coreduet(db: &Path) -> Result<(), OptimizeActionError> {
    for suffix in ["-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{suffix}", db.display()));
        let _ = fs::remove_file(p);
    }
    let status = Command::new("sqlite3")
        .arg(db)
        .arg(
            "DELETE FROM ZOBJECT WHERE ZCREATIONDATE < (strftime('%s','now','-90 days') - strftime('%s','2001-01-01')); VACUUM;",
        )
        .status()
        .map_err(|_| OptimizeActionError::Failed)?;
    if status.success() {
        Ok(())
    } else {
        Err(OptimizeActionError::Skipped)
    }
}

fn apply_dock() -> Result<(), OptimizeActionError> {
    let status = Command::new("killall")
        .arg("Dock")
        .status()
        .map_err(|_| OptimizeActionError::Failed)?;
    if status.success() {
        Ok(())
    } else {
        // Dock may already be restarting
        Ok(())
    }
}

fn lsregister_path() -> Option<PathBuf> {
    let candidates = [
        "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
        "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister",
    ];
    candidates
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

fn apply_launch_services() -> Result<(), OptimizeActionError> {
    let Some(ls) = lsregister_path() else {
        return Err(OptimizeActionError::Skipped);
    };
    let _ = Command::new(&ls).arg("-gc").status();
    let status = Command::new(&ls)
        .args([
            "-r", "-f", "-domain", "local", "-domain", "user", "-domain", "system",
        ])
        .status()
        .map_err(|_| OptimizeActionError::Failed)?;
    if status.success() {
        return Ok(());
    }
    let status = Command::new(&ls)
        .args(["-r", "-f", "-domain", "local", "-domain", "user"])
        .status()
        .map_err(|_| OptimizeActionError::Failed)?;
    if status.success() {
        Ok(())
    } else {
        Err(OptimizeActionError::Failed)
    }
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

        apply_optimize_action("quarantine_cleanup", &db, None, &mut false).unwrap();
        let count = sqlite_count(&db, "SELECT COUNT(*) FROM LSQuarantineEvent;").unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn dock_and_ls_sentinels() {
        let home = Path::new("/tmp");
        assert_eq!(plan_dock_refresh(home).task_id, "dock_refresh");
        assert_eq!(
            plan_launch_services_rebuild(home).task_id,
            "launch_services_rebuild"
        );
        assert_eq!(
            plan_memory_pressure_relief(home).task_id,
            "memory_pressure_relief"
        );
        assert_eq!(
            plan_network_stack_optimize(home).task_id,
            "network_stack_optimize"
        );
        assert_eq!(
            plan_disk_permissions_repair(home).task_id,
            "disk_permissions_repair"
        );
        assert_eq!(
            plan_periodic_maintenance(home).task_id,
            "periodic_maintenance"
        );
    }

    #[test]
    fn memory_pressure_env_override() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_MEMORY_PRESSURE", "1");
        assert!(is_memory_pressure_high());
        std::env::set_var("VOLE_TEST_MEMORY_PRESSURE", "0");
        assert!(!is_memory_pressure_high());
        std::env::remove_var("VOLE_TEST_MEMORY_PRESSURE");
    }

    #[test]
    fn apply_memory_noop_when_pressure_low() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_MEMORY_PRESSURE", "0");
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "memory_pressure_relief",
            Path::new("/tmp"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.purge_memory_calls.lock().unwrap(), 0);
        std::env::remove_var("VOLE_TEST_MEMORY_PRESSURE");
    }

    #[test]
    fn apply_memory_needs_privilege_when_high() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_MEMORY_PRESSURE", "1");
        let err = apply_optimize_action(
            "memory_pressure_relief",
            Path::new("/tmp"),
            Some(&crate::privilege::NoPrivilege),
            &mut false,
        )
        .unwrap_err();
        assert_eq!(err, OptimizeActionError::NeedsPrivilege);
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "memory_pressure_relief",
            Path::new("/tmp"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.purge_memory_calls.lock().unwrap(), 1);
        std::env::remove_var("VOLE_TEST_MEMORY_PRESSURE");
    }

    #[test]
    fn apply_network_stack_vpn_skips() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_VPN_ACTIVE", "1");
        std::env::set_var("VOLE_TEST_NETWORK_STACK_UNHEALTHY", "1");
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "network_stack_optimize",
            Path::new("/tmp/.vole-optimize-action/network_stack_optimize"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.network_stack_calls.lock().unwrap(), 0);
        std::env::remove_var("VOLE_TEST_VPN_ACTIVE");
        std::env::remove_var("VOLE_TEST_NETWORK_STACK_UNHEALTHY");
    }

    #[test]
    fn apply_network_stack_healthy_noop() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_VPN_ACTIVE", "0");
        std::env::set_var("VOLE_TEST_NETWORK_STACK_UNHEALTHY", "0");
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "network_stack_optimize",
            Path::new("/tmp/.vole-optimize-action/network_stack_optimize"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.network_stack_calls.lock().unwrap(), 0);
        std::env::remove_var("VOLE_TEST_VPN_ACTIVE");
        std::env::remove_var("VOLE_TEST_NETWORK_STACK_UNHEALTHY");
    }

    #[test]
    fn apply_network_stack_needs_privilege_when_unhealthy() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_VPN_ACTIVE", "0");
        std::env::set_var("VOLE_TEST_NETWORK_STACK_UNHEALTHY", "1");
        let err = apply_optimize_action(
            "network_stack_optimize",
            Path::new("/tmp/.vole-optimize-action/network_stack_optimize"),
            Some(&crate::privilege::NoPrivilege),
            &mut false,
        )
        .unwrap_err();
        assert_eq!(err, OptimizeActionError::NeedsPrivilege);
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "network_stack_optimize",
            Path::new("/tmp/.vole-optimize-action/network_stack_optimize"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.network_stack_calls.lock().unwrap(), 1);
        std::env::remove_var("VOLE_TEST_VPN_ACTIVE");
        std::env::remove_var("VOLE_TEST_NETWORK_STACK_UNHEALTHY");
    }

    #[test]
    fn apply_disk_permissions_noop_when_ok() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_DISK_PERMISSIONS_NEED_REPAIR", "0");
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "disk_permissions_repair",
            Path::new("/tmp/.vole-optimize-action/disk_permissions_repair"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.reset_permissions_calls.lock().unwrap(), 0);
        std::env::remove_var("VOLE_TEST_DISK_PERMISSIONS_NEED_REPAIR");
    }

    #[test]
    fn apply_disk_permissions_needs_privilege() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_DISK_PERMISSIONS_NEED_REPAIR", "1");
        let err = apply_optimize_action(
            "disk_permissions_repair",
            Path::new("/tmp/.vole-optimize-action/disk_permissions_repair"),
            Some(&crate::privilege::NoPrivilege),
            &mut false,
        )
        .unwrap_err();
        assert_eq!(err, OptimizeActionError::NeedsPrivilege);
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "disk_permissions_repair",
            Path::new("/tmp/.vole-optimize-action/disk_permissions_repair"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.reset_permissions_calls.lock().unwrap(), 1);
        std::env::remove_var("VOLE_TEST_DISK_PERMISSIONS_NEED_REPAIR");
    }

    #[test]
    fn apply_periodic_noop_when_fresh() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_PERIODIC_AVAILABLE", "1");
        std::env::set_var("VOLE_TEST_PERIODIC_STALE", "0");
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "periodic_maintenance",
            Path::new("/tmp/.vole-optimize-action/periodic_maintenance"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.periodic_calls.lock().unwrap(), 0);
        std::env::remove_var("VOLE_TEST_PERIODIC_AVAILABLE");
        std::env::remove_var("VOLE_TEST_PERIODIC_STALE");
    }

    #[test]
    fn apply_periodic_needs_privilege_when_stale() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_PERIODIC_AVAILABLE", "1");
        std::env::set_var("VOLE_TEST_PERIODIC_STALE", "1");
        let err = apply_optimize_action(
            "periodic_maintenance",
            Path::new("/tmp/.vole-optimize-action/periodic_maintenance"),
            Some(&crate::privilege::NoPrivilege),
            &mut false,
        )
        .unwrap_err();
        assert_eq!(err, OptimizeActionError::NeedsPrivilege);
        let backend = crate::privilege::RecordingPrivilege::allowing();
        apply_optimize_action(
            "periodic_maintenance",
            Path::new("/tmp/.vole-optimize-action/periodic_maintenance"),
            Some(&backend),
            &mut false,
        )
        .unwrap();
        assert_eq!(*backend.periodic_calls.lock().unwrap(), 1);
        std::env::remove_var("VOLE_TEST_PERIODIC_AVAILABLE");
        std::env::remove_var("VOLE_TEST_PERIODIC_STALE");
    }

    #[test]
    fn defaults_truthy_helper() {
        assert!(defaults_is_truthy("1"));
        assert!(defaults_is_truthy("TRUE"));
        assert!(!defaults_is_truthy("0"));
    }

    #[test]
    fn apply_login_items_audit_broken_noop() {
        let path = Path::new("/tmp/.vole-optimize-action/login_items_audit/Missing%20Helper");
        apply_optimize_action("login_items_audit", path, None, &mut false).unwrap();
    }

    #[test]
    fn apply_login_items_audit_unavailable_needs_privilege() {
        let path = Path::new("/tmp/.vole-optimize-action/login_items_audit");
        let err = apply_optimize_action("login_items_audit", path, None, &mut false).unwrap_err();
        assert_eq!(err, OptimizeActionError::NeedsPrivilege);
    }

    #[test]
    fn apply_spotlight_orphan_rules_cleanup_test_no_auth_skipped() {
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        let path = Path::new("/tmp/.vole-optimize-action/spotlight_orphan_rules_cleanup");
        let err = apply_optimize_action("spotlight_orphan_rules_cleanup", path, None, &mut false)
            .unwrap_err();
        assert_eq!(err, OptimizeActionError::Skipped);
        std::env::remove_var("VOLE_TEST_NO_AUTH");
    }
}
