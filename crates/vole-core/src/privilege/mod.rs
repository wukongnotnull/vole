//! CLI 非交互提权（`sudo -n`）与 allowlist。
//!
//! 桌面 SMAppService 另开接缝实现，本期仅 `SudoNoninteractive` / `NoPrivilege`。

mod sudo;

pub use sudo::{NoPrivilege, RecordingPrivilege, SudoNoninteractive};

use crate::safety::{
    is_adobe_system_log_clean_target, is_icon_services_system_cache,
    is_private_var_db_diagnostic_pipeline_clean_target, is_private_var_db_diagnostics_clean_target,
    is_private_var_db_memory_limit_violations_clean_target,
    is_private_var_db_powerlog_clean_target, is_private_var_log_clean_target,
    is_rosetta_update_bundle, is_system_diagnostic_report_leaf, ADOBEGC_LOG_LIVE, ADOBE_LOGS_LIVE,
    ADOBE_SYSTEM_LOGS_MAX_DEPTH, CREATIVE_CLOUD_LOGS_LIVE, PRIVATE_VAR_DB_DIAGNOSTICS_LIVE,
    PRIVATE_VAR_DB_DIAGNOSTICS_MAX_DEPTH, PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_LIVE,
    PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_MAX_DEPTH, PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_LIVE,
    PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_MAX_DEPTH, PRIVATE_VAR_DB_POWERLOG_LIVE,
    PRIVATE_VAR_DB_POWERLOG_MAX_DEPTH, PRIVATE_VAR_LOG_LIVE, PRIVATE_VAR_LOG_MAX_DEPTH,
};

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

/// `rosetta-2-cache` 规则 id（1.12.0）。
pub const ROSETTA_CACHE_RULE_ID: &str = "rosetta-2-cache";

/// `icon-services-system-cache` 规则 id（1.13.0）。
pub const ICON_SERVICES_SYSTEM_CACHE_RULE_ID: &str = "icon-services-system-cache";

/// `diagnostic-reports-system` 规则 id（1.14.0）。
pub const DIAGNOSTIC_REPORTS_SYSTEM_RULE_ID: &str = "diagnostic-reports-system";

/// `private-var-log` 规则 id（1.15.0）。
pub const PRIVATE_VAR_LOG_RULE_ID: &str = "private-var-log";

/// `private-var-db-diagnostics` 规则 id（1.16.0）。
pub const PRIVATE_VAR_DB_DIAGNOSTICS_RULE_ID: &str = "private-var-db-diagnostics";

/// `private-var-db-diagnostic-pipeline` 规则 id（1.17.0）。
pub const PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_RULE_ID: &str = "private-var-db-diagnostic-pipeline";

/// `private-var-db-powerlog` 规则 id（1.18.0）。
pub const PRIVATE_VAR_DB_POWERLOG_RULE_ID: &str = "private-var-db-powerlog";

/// `private-var-db-memory-limit-violations` 规则 id（1.19.0）。
pub const PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_RULE_ID: &str =
    "private-var-db-memory-limit-violations";

/// `adobe-system-logs` 规则 id（1.20.0）。
pub const ADOBE_SYSTEM_LOGS_RULE_ID: &str = "adobe-system-logs";

/// 系统 DiagnosticReports 年龄阈（对齐 Mole `MOLE_CRASH_REPORT_AGE_DAYS`）。
pub const DIAGNOSTIC_REPORTS_SYSTEM_AGE_DAYS: u32 = 7;

/// `/private/var/log` 年龄阈（对齐 Mole `MOLE_LOG_AGE_DAYS`）。
pub const PRIVATE_VAR_LOG_AGE_DAYS: u32 = 7;

/// `/private/var/db/diagnostics` 非 `.tracev3` 年龄阈。
pub const PRIVATE_VAR_DB_DIAGNOSTICS_AGE_DAYS: u32 = 7;

/// `/private/var/db/diagnostics` `.tracev3` 年龄阈（对齐 Mole 第二刀）。
pub const PRIVATE_VAR_DB_DIAGNOSTICS_TRACEV3_AGE_DAYS: u32 = 30;

/// `/private/var/db/DiagnosticPipeline` 年龄阈。
pub const PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_AGE_DAYS: u32 = 7;

/// `/private/var/db/powerlog` 年龄阈。
pub const PRIVATE_VAR_DB_POWERLOG_AGE_DAYS: u32 = 7;

/// MemoryLimitViolations 年龄阈（对齐 Mole `mtime +30`）。
pub const PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_AGE_DAYS: u32 = 30;

/// Adobe 系统日志年龄阈（对齐 Mole `MOLE_LOG_AGE_DAYS`）。
pub const ADOBE_SYSTEM_LOGS_AGE_DAYS: u32 = 7;

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

/// live 或测试映射下的 Icon Services 系统缓存路径。
pub fn icon_services_system_cache_path() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        return PathBuf::from(base).join("Caches/com.apple.iconservices.store");
    }
    PathBuf::from(crate::safety::ICON_SERVICES_SYSTEM_CACHE_LIVE)
}

/// plan 候选：路径存在时返回 exact（无 arch 门控）。
pub fn icon_services_system_plan_candidates() -> Vec<PathBuf> {
    let path = icon_services_system_cache_path();
    if path.exists() {
        vec![path]
    } else {
        Vec::new()
    }
}

/// live 或测试映射下的系统 DiagnosticReports 根目录。
pub fn diagnostic_reports_system_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        return PathBuf::from(base).join("Logs/DiagnosticReports");
    }
    PathBuf::from("/Library/Logs/DiagnosticReports")
}

/// plan 候选：根下可读的**文件**单层叶。
pub fn diagnostic_reports_system_plan_candidates() -> Vec<PathBuf> {
    let root = diagnostic_reports_system_root();
    let Ok(rd) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.file_type().is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_system_diagnostic_report_leaf(s) {
            out.push(path);
        }
    }
    out
}

/// live 或测试映射下的 `/private/var/log` 根目录。
pub fn private_var_log_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        if let Some(parent) = Path::new(&base).parent() {
            return parent.join("private/var/log");
        }
    }
    PathBuf::from(PRIVATE_VAR_LOG_LIVE)
}

fn walk_private_var_log_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > PRIVATE_VAR_LOG_MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if depth < PRIVATE_VAR_LOG_MAX_DEPTH {
                walk_private_var_log_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_private_var_log_clean_target(s) {
            out.push(path);
        }
    }
}

/// plan 候选：根下深度 ≤5、扩展名匹配的文件。
pub fn private_var_log_plan_candidates() -> Vec<PathBuf> {
    let root = private_var_log_root();
    let mut out = Vec::new();
    walk_private_var_log_files(&root, 1, &mut out);
    out
}

/// 诊断库文件应使用的年龄阈（`.tracev3` → 30，其它 → 7）。
pub fn private_var_db_diagnostics_age_days(path: &Path) -> u32 {
    if path.extension().and_then(|e| e.to_str()) == Some("tracev3") {
        PRIVATE_VAR_DB_DIAGNOSTICS_TRACEV3_AGE_DAYS
    } else {
        PRIVATE_VAR_DB_DIAGNOSTICS_AGE_DAYS
    }
}

/// live 或测试映射下的 `/private/var/db/diagnostics` 根目录。
pub fn private_var_db_diagnostics_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        if let Some(parent) = Path::new(&base).parent() {
            return parent.join("private/var/db/diagnostics");
        }
    }
    PathBuf::from(PRIVATE_VAR_DB_DIAGNOSTICS_LIVE)
}

fn walk_private_var_db_diagnostics_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > PRIVATE_VAR_DB_DIAGNOSTICS_MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if depth < PRIVATE_VAR_DB_DIAGNOSTICS_MAX_DEPTH {
                walk_private_var_db_diagnostics_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if !is_private_var_db_diagnostics_clean_target(s) {
            continue;
        }
        if path_mtime_older_than_days(&path, private_var_db_diagnostics_age_days(&path)) {
            out.push(path);
        }
    }
}

/// plan 候选：根下深度 ≤5、满足分龄的文件。
pub fn private_var_db_diagnostics_plan_candidates() -> Vec<PathBuf> {
    let root = private_var_db_diagnostics_root();
    let mut out = Vec::new();
    walk_private_var_db_diagnostics_files(&root, 1, &mut out);
    out
}

/// live 或测试映射下的 `/private/var/db/DiagnosticPipeline` 根目录。
pub fn private_var_db_diagnostic_pipeline_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        if let Some(parent) = Path::new(&base).parent() {
            return parent.join("private/var/db/DiagnosticPipeline");
        }
    }
    PathBuf::from(PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_LIVE)
}

fn walk_private_var_db_diagnostic_pipeline_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if depth < PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_MAX_DEPTH {
                walk_private_var_db_diagnostic_pipeline_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_private_var_db_diagnostic_pipeline_clean_target(s) {
            out.push(path);
        }
    }
}

/// plan 候选：根下深度 ≤5 的普通文件。
pub fn private_var_db_diagnostic_pipeline_plan_candidates() -> Vec<PathBuf> {
    let root = private_var_db_diagnostic_pipeline_root();
    let mut out = Vec::new();
    walk_private_var_db_diagnostic_pipeline_files(&root, 1, &mut out);
    out
}

/// live 或测试映射下的 `/private/var/db/powerlog` 根目录。
pub fn private_var_db_powerlog_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        if let Some(parent) = Path::new(&base).parent() {
            return parent.join("private/var/db/powerlog");
        }
    }
    PathBuf::from(PRIVATE_VAR_DB_POWERLOG_LIVE)
}

fn walk_private_var_db_powerlog_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > PRIVATE_VAR_DB_POWERLOG_MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if depth < PRIVATE_VAR_DB_POWERLOG_MAX_DEPTH {
                walk_private_var_db_powerlog_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_private_var_db_powerlog_clean_target(s) {
            out.push(path);
        }
    }
}

/// plan 候选：根下深度 ≤5 的普通文件。
pub fn private_var_db_powerlog_plan_candidates() -> Vec<PathBuf> {
    let root = private_var_db_powerlog_root();
    let mut out = Vec::new();
    walk_private_var_db_powerlog_files(&root, 1, &mut out);
    out
}

/// live 或测试映射下的 MemoryLimitViolations 根目录。
pub fn private_var_db_memory_limit_violations_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        if let Some(parent) = Path::new(&base).parent() {
            return parent.join("private/var/db/reportmemoryexception/MemoryLimitViolations");
        }
    }
    PathBuf::from(PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_LIVE)
}

fn walk_private_var_db_memory_limit_violations_files(
    dir: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
) {
    if depth > PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if depth < PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_MAX_DEPTH {
                walk_private_var_db_memory_limit_violations_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_private_var_db_memory_limit_violations_clean_target(s) {
            out.push(path);
        }
    }
}

/// plan 候选：根下深度 ≤5 的普通文件。
pub fn private_var_db_memory_limit_violations_plan_candidates() -> Vec<PathBuf> {
    let root = private_var_db_memory_limit_violations_root();
    let mut out = Vec::new();
    walk_private_var_db_memory_limit_violations_files(&root, 1, &mut out);
    out
}

fn adobe_system_log_tree_dirs() -> Vec<PathBuf> {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let base = PathBuf::from(base);
        return vec![base.join("Logs/Adobe"), base.join("Logs/CreativeCloud")];
    }
    vec![
        PathBuf::from(ADOBE_LOGS_LIVE),
        PathBuf::from(CREATIVE_CLOUD_LOGS_LIVE),
    ]
}

fn adobegc_log_path() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        return PathBuf::from(base).join("Logs/adobegc.log");
    }
    PathBuf::from(ADOBEGC_LOG_LIVE)
}

fn walk_adobe_system_log_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > ADOBE_SYSTEM_LOGS_MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if depth < ADOBE_SYSTEM_LOGS_MAX_DEPTH {
                walk_adobe_system_log_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_adobe_system_log_clean_target(s) {
            out.push(path);
        }
    }
}

/// plan 候选：Adobe / CreativeCloud 树叶 + exact adobegc.log（若为文件）。
pub fn adobe_system_logs_plan_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in adobe_system_log_tree_dirs() {
        walk_adobe_system_log_files(&root, 1, &mut out);
    }
    let gc = adobegc_log_path();
    if gc.is_file() {
        if let Some(s) = gc.to_str() {
            if is_adobe_system_log_clean_target(s) {
                out.push(gc);
            }
        }
    }
    out
}

/// 当前 mtime 是否早于 `days` 天（apply 年龄重验）。
pub fn path_mtime_older_than_days(path: &Path, days: u32) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    let Some(cutoff) = SystemTime::now().checked_sub(Duration::from_secs(u64::from(days) * 86_400))
    else {
        return false;
    };
    mtime < cutoff
}

/// 绝对路径、无 `..`，且：特权 exact/叶（含 adobe-system-logs）**或** 三树下单层叶。
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
    if is_rosetta_update_bundle(s)
        || is_icon_services_system_cache(s)
        || is_system_diagnostic_report_leaf(s)
        || is_private_var_log_clean_target(s)
        || is_private_var_db_diagnostics_clean_target(s)
        || is_private_var_db_diagnostic_pipeline_clean_target(s)
        || is_private_var_db_powerlog_clean_target(s)
        || is_private_var_db_memory_limit_violations_clean_target(s)
        || is_adobe_system_log_clean_target(s)
    {
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
    fn allowlist_accepts_icon_services_system_exact() {
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Caches/com.apple.iconservices.store"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Caches/com.apple.iconservices.store/x"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Caches/com.apple.other"
        )));
    }

    #[test]
    fn allowlist_accepts_diagnostic_reports_system_leaf() {
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Logs/DiagnosticReports/App.crash"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Logs/DiagnosticReports"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Logs/DiagnosticReports/sub/a.crash"
        )));
    }

    #[test]
    fn allowlist_accepts_private_var_log_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/log/system.log"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/log/a/b/c/d/e.log"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/log/a/b/c/d/e/f.log"
        )));
        assert!(!path_allowed_for_privilege(Path::new("/private/var/log")));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/log/notes.txt"
        )));
    }

    #[test]
    fn allowlist_accepts_private_var_db_diagnostics_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/db/diagnostics/log.data"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/db/diagnostics/a/b/c/d/e.tracev3"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/db/diagnostics/a/b/c/d/e/f.data"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/db/diagnostics"
        )));
    }

    #[test]
    fn allowlist_accepts_private_var_db_diagnostic_pipeline_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/db/DiagnosticPipeline/x.data"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/db/DiagnosticPipeline/a/b/c/d/e.data"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/db/DiagnosticPipeline/a/b/c/d/e/f.data"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/db/DiagnosticPipeline"
        )));
    }

    #[test]
    fn allowlist_accepts_private_var_db_powerlog_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/db/powerlog/x.data"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/db/powerlog/a/b/c/d/e.data"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/db/powerlog/a/b/c/d/e/f.data"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/db/powerlog"
        )));
    }

    #[test]
    fn allowlist_accepts_memory_limit_violations_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/db/reportmemoryexception/MemoryLimitViolations/x.data"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/db/reportmemoryexception/MemoryLimitViolations/a/b/c/d/e.data"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/db/reportmemoryexception/MemoryLimitViolations/a/b/c/d/e/f.data"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/db/reportmemoryexception/MemoryLimitViolations"
        )));
    }

    #[test]
    fn allowlist_accepts_adobe_system_logs_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Logs/Adobe/Installer/foo.log"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Logs/CreativeCloud/a/b/c/d/e.log"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Logs/CreativeCloud/a/b/c/d/e/f.log"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Logs/adobegc.log"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Logs/Adobe"
        )));
    }

    #[test]
    fn diagnostics_age_days_splits_tracev3() {
        assert_eq!(
            private_var_db_diagnostics_age_days(Path::new("/x/y.log")),
            PRIVATE_VAR_DB_DIAGNOSTICS_AGE_DAYS
        );
        assert_eq!(
            private_var_db_diagnostics_age_days(Path::new("/x/y.tracev3")),
            PRIVATE_VAR_DB_DIAGNOSTICS_TRACEV3_AGE_DAYS
        );
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
