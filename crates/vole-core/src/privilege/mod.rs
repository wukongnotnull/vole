//! CLI 非交互提权（`sudo -n`）与 allowlist。
//!
//! 桌面 SMAppService 另开接缝实现，本期仅 `SudoNoninteractive` / `NoPrivilege`。

mod sudo;

pub use sudo::{NoPrivilege, RecordingPrivilege, SudoNoninteractive};

use crate::safety::{
    is_adobe_system_log_clean_target, is_code_sign_clone_clean_target,
    is_endpoint_security_cache_path, is_gpu_metal_cache_clean_target,
    is_icon_services_system_cache, is_idleassetsd_cfnetwork_tmp_clean_target,
    is_library_caches_temp_clean_target, is_private_tmp_clean_target,
    is_private_var_db_diagnostic_pipeline_clean_target, is_private_var_db_diagnostics_clean_target,
    is_private_var_db_memory_limit_violations_clean_target,
    is_private_var_db_powerlog_clean_target, is_private_var_log_clean_target,
    is_rosetta_update_bundle, is_system_diagnostic_report_leaf, ADOBEGC_LOG_LIVE, ADOBE_LOGS_LIVE,
    ADOBE_SYSTEM_LOGS_MAX_DEPTH, CODE_SIGN_CLONE_MAX_DEPTH, CREATIVE_CLOUD_LOGS_LIVE,
    GPU_METAL_CACHE_LOCATE_MAX_DEPTH, IDLEASSETSD_CFNETWORK_TMP_MAX_DEPTH, IDLEASSETSD_DIR_NAME,
    IDLEASSETSD_LOCATE_MAX_DEPTH, LIBRARY_CACHES_LIVE, LIBRARY_CACHES_TEMP_MAX_DEPTH,
    PRIVATE_TMP_LIVE, PRIVATE_TMP_MAX_DEPTH, PRIVATE_VAR_DB_DIAGNOSTICS_LIVE,
    PRIVATE_VAR_DB_DIAGNOSTICS_MAX_DEPTH, PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_LIVE,
    PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_MAX_DEPTH, PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_LIVE,
    PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_MAX_DEPTH, PRIVATE_VAR_DB_POWERLOG_LIVE,
    PRIVATE_VAR_DB_POWERLOG_MAX_DEPTH, PRIVATE_VAR_FOLDERS_LIVE, PRIVATE_VAR_LOG_LIVE,
    PRIVATE_VAR_LOG_MAX_DEPTH, PRIVATE_VAR_TMP_LIVE,
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

/// `private-tmp` 规则 id（1.21.0）。
pub const PRIVATE_TMP_RULE_ID: &str = "private-tmp";

/// `library-caches-temp` 规则 id（1.22.0）。
pub const LIBRARY_CACHES_TEMP_RULE_ID: &str = "library-caches-temp";

/// `idleassetsd-cfnetwork-tmp` 规则 id（1.23.0）。
pub const IDLEASSETSD_CFNETWORK_TMP_RULE_ID: &str = "idleassetsd-cfnetwork-tmp";

/// `code-sign-clone` 规则 id（1.24.0）。
pub const CODE_SIGN_CLONE_RULE_ID: &str = "code-sign-clone";

/// `gpu-metal-caches` 规则 id（1.25.0）。
pub const GPU_METAL_CACHES_RULE_ID: &str = "gpu-metal-caches";

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

/// `/private/tmp` + `/private/var/tmp` 年龄阈（对齐 Mole `MOLE_TEMP_FILE_AGE_DAYS`）。
pub const PRIVATE_TMP_AGE_DAYS: u32 = 7;

/// `/Library/Caches` `*.cache`/`*.tmp` 年龄阈（对齐 Mole `MOLE_TEMP_FILE_AGE_DAYS`）。
pub const LIBRARY_CACHES_TEMP_AGE_DAYS: u32 = 7;

/// `/Library/Caches` `*.log` 年龄阈（对齐 Mole `MOLE_LOG_AGE_DAYS`）。
pub const LIBRARY_CACHES_LOG_AGE_DAYS: u32 = 7;

/// idleassetsd `CFNetworkDownload_*.tmp` 年龄阈（对齐 Mole `MOLE_TEMP_FILE_AGE_DAYS`）。
pub const IDLEASSETSD_CFNETWORK_TMP_AGE_DAYS: u32 = 7;

/// GPU Metal caches 新鲜保留窗（对齐 Mole `MOLE_GPU_CACHE_AGE_DAYS`；目录内无更新于此窗内文件才 stale）。
pub const GPU_METAL_CACHE_AGE_DAYS: u32 = 1;

/// `install-macos-apps` 规则 id（1.27.0）。
pub const INSTALL_MACOS_APPS_RULE_ID: &str = "install-macos-apps";

/// Install macOS\*.app 年龄阈（对齐 Mole system.sh 14 天）。
pub const INSTALL_MACOS_APP_AGE_DAYS: u32 = 14;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivilegeError {
    Unavailable,
    Refused,
    CommandFailed(String),
}

pub trait PrivilegeBackend: Send + Sync {
    fn probe_noninteractive(&self) -> bool;
    /// 尝试交互缓存凭证（如 `sudo -v`）。默认 no-op。
    fn acquire_interactive(&self) -> bool {
        false
    }
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

fn private_tmp_roots() -> Vec<PathBuf> {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        if let Some(parent) = Path::new(&base).parent() {
            return vec![parent.join("private/tmp"), parent.join("private/var/tmp")];
        }
    }
    vec![
        PathBuf::from(PRIVATE_TMP_LIVE),
        PathBuf::from(PRIVATE_VAR_TMP_LIVE),
    ]
}

fn walk_private_tmp_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > PRIVATE_TMP_MAX_DEPTH {
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
            if depth < PRIVATE_TMP_MAX_DEPTH {
                walk_private_tmp_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_private_tmp_clean_target(s) {
            out.push(path);
        }
    }
}

/// plan 候选：两根下深度 1 的普通文件。
pub fn private_tmp_plan_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in private_tmp_roots() {
        walk_private_tmp_files(&root, 1, &mut out);
    }
    out
}

/// `/Library/Caches` 文件应使用的年龄阈（`.log` → LOG，其它 → TEMP）。
pub fn library_caches_temp_age_days(path: &Path) -> u32 {
    if path.extension().and_then(|e| e.to_str()) == Some("log") {
        LIBRARY_CACHES_LOG_AGE_DAYS
    } else {
        LIBRARY_CACHES_TEMP_AGE_DAYS
    }
}

/// live 或测试映射下的 `/Library/Caches` 根目录。
pub fn library_caches_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        return PathBuf::from(base).join("Caches");
    }
    PathBuf::from(LIBRARY_CACHES_LIVE)
}

fn walk_library_caches_temp_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > LIBRARY_CACHES_TEMP_MAX_DEPTH {
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
            if depth < LIBRARY_CACHES_TEMP_MAX_DEPTH {
                walk_library_caches_temp_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_library_caches_temp_clean_target(s) {
            out.push(path);
        }
    }
}

/// plan 候选：`/Library/Caches` 下深度 ≤5、扩展名匹配的普通文件。
pub fn library_caches_temp_plan_candidates() -> Vec<PathBuf> {
    let root = library_caches_root();
    let mut out = Vec::new();
    walk_library_caches_temp_files(&root, 1, &mut out);
    out
}

fn private_var_folders_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        if let Some(parent) = Path::new(&base).parent() {
            return parent.join("private/var/folders");
        }
    }
    PathBuf::from(PRIVATE_VAR_FOLDERS_LIVE)
}

fn walk_locate_idleassetsd_dirs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > IDLEASSETSD_LOCATE_MAX_DEPTH {
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
        if !meta.file_type().is_dir() {
            continue;
        }
        let is_idle = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == IDLEASSETSD_DIR_NAME);
        if is_idle {
            // Mole: `-path "*/T/*" -name com.apple.idleassetsd`
            if path.to_str().is_some_and(|s| {
                s.contains("/T/com.apple.idleassetsd") || s.ends_with("/T/com.apple.idleassetsd")
            }) {
                out.push(path);
            }
            continue;
        }
        if depth < IDLEASSETSD_LOCATE_MAX_DEPTH {
            walk_locate_idleassetsd_dirs(&path, depth + 1, out);
        }
    }
}

fn walk_idleassetsd_cfnetwork_tmp_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > IDLEASSETSD_CFNETWORK_TMP_MAX_DEPTH {
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
            if depth < IDLEASSETSD_CFNETWORK_TMP_MAX_DEPTH {
                walk_idleassetsd_cfnetwork_tmp_files(&path, depth + 1, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_idleassetsd_cfnetwork_tmp_clean_target(s) {
            out.push(path);
        }
    }
}

/// plan 候选：folders 下定位 idleassetsd（*/T/*）后扫描 `CFNetworkDownload_*.tmp`。
pub fn idleassetsd_cfnetwork_tmp_plan_candidates() -> Vec<PathBuf> {
    let root = private_var_folders_root();
    let mut dirs = Vec::new();
    walk_locate_idleassetsd_dirs(&root, 1, &mut dirs);
    let mut out = Vec::new();
    for dir in dirs {
        walk_idleassetsd_cfnetwork_tmp_files(&dir, 1, &mut out);
    }
    out
}

fn walk_code_sign_clone_dirs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > CODE_SIGN_CLONE_MAX_DEPTH {
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
        if !meta.file_type().is_dir() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_code_sign_clone_clean_target(s) && !is_endpoint_security_cache_path(s) {
            out.push(path.clone());
            continue;
        }
        if depth < CODE_SIGN_CLONE_MAX_DEPTH {
            walk_code_sign_clone_dirs(&path, depth + 1, out);
        }
    }
}

/// plan 候选：folders 下 `*/X/*/*.code_sign_clone` 目录（排除 EDR）。
pub fn code_sign_clone_plan_candidates() -> Vec<PathBuf> {
    let root = private_var_folders_root();
    let mut out = Vec::new();
    walk_code_sign_clone_dirs(&root, 1, &mut out);
    out
}

/// 对齐 Mole `gpu_cache_dir_is_stale`：目录不是 symlink，且目录内无「mtime 落在最近 `age_days` 天」的普通文件。
pub fn gpu_metal_cache_is_stale(path: &Path, age_days: u32) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
        return false;
    }
    !dir_has_recent_regular_file(path, age_days)
}

fn dir_has_recent_regular_file(dir: &Path, age_days: u32) -> bool {
    let Some(cutoff) =
        SystemTime::now().checked_sub(Duration::from_secs(u64::from(age_days) * 86_400))
    else {
        return true;
    };
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if dir_has_recent_regular_file(&path, age_days) {
                return true;
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Ok(mtime) = meta.modified() else {
            continue;
        };
        if mtime >= cutoff {
            return true;
        }
    }
    false
}

fn walk_gpu_metal_cache_dirs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > GPU_METAL_CACHE_LOCATE_MAX_DEPTH {
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
        if !meta.file_type().is_dir() {
            continue;
        }
        let Some(s) = path.to_str() else {
            continue;
        };
        if is_gpu_metal_cache_clean_target(s)
            && !is_endpoint_security_cache_path(s)
            && gpu_metal_cache_is_stale(&path, GPU_METAL_CACHE_AGE_DAYS)
        {
            out.push(path.clone());
            continue;
        }
        if depth < GPU_METAL_CACHE_LOCATE_MAX_DEPTH {
            walk_gpu_metal_cache_dirs(&path, depth + 1, out);
        }
    }
}

/// plan 候选：folders 下重建型 GPU Metal 缓存目录（stale + 排除 EDR）。
pub fn gpu_metal_caches_plan_candidates() -> Vec<PathBuf> {
    let root = private_var_folders_root();
    let mut out = Vec::new();
    walk_gpu_metal_cache_dirs(&root, 1, &mut out);
    out
}

/// `/Applications` 或 `VOLE_TEST_APPLICATIONS`。
pub fn applications_root() -> PathBuf {
    if let Ok(base) = std::env::var("VOLE_TEST_APPLICATIONS") {
        return PathBuf::from(base);
    }
    PathBuf::from("/Applications")
}

/// Software Update plist 路径，或 `VOLE_TEST_SOFTWARE_UPDATE_PLIST`。
pub fn software_update_plist_path() -> PathBuf {
    if let Ok(p) = std::env::var("VOLE_TEST_SOFTWARE_UPDATE_PLIST") {
        return PathBuf::from(p);
    }
    PathBuf::from("/Library/Preferences/com.apple.SoftwareUpdate.plist")
}

/// 当前 macOS 主版本；`VOLE_TEST_MACOS_MAJOR` 优先，否则 `sw_vers -productVersion`。
pub fn current_macos_major() -> Option<String> {
    if let Ok(v) = std::env::var("VOLE_TEST_MACOS_MAJOR") {
        let t = v.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let ver = String::from_utf8_lossy(&output.stdout);
    let major = ver.trim().split('.').next()?.trim();
    if major.is_empty() {
        None
    } else {
        Some(major.to_string())
    }
}

/// Fail-closed：`true` = 不得清理 Installer（pending 或未知）。
/// 仅可读且 `RecommendedUpdates` 为空数组时返回 `false`。
pub fn software_update_pending_or_unknown(plist: &Path) -> bool {
    if !plist.is_file() {
        return true;
    }
    let Ok(value) = plist::Value::from_file(plist) else {
        return true;
    };
    let Some(dict) = value.as_dictionary() else {
        return true;
    };
    let Some(recommended) = dict.get("RecommendedUpdates") else {
        return true;
    };
    let Some(arr) = recommended.as_array() else {
        return true;
    };
    !arr.is_empty()
}

/// `{apps_root}/Install macOS*.app` 单层 bundle 形状（绝对路径、无 `..`）。
pub fn is_install_macos_app_bundle(path: &Path, apps_root: &Path) -> bool {
    if !path.is_absolute() || !apps_root.is_absolute() {
        return false;
    }
    if path.components().any(|c| matches!(c, Component::ParentDir))
        || apps_root
            .components()
            .any(|c| matches!(c, Component::ParentDir))
    {
        return false;
    }
    let Ok(rel) = path.strip_prefix(apps_root) else {
        return false;
    };
    let mut comps = rel.components();
    let Some(Component::Normal(name)) = comps.next() else {
        return false;
    };
    if comps.next().is_some() {
        return false;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    name.starts_with("Install macOS") && name.ends_with(".app")
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
        || is_private_tmp_clean_target(s)
        || is_library_caches_temp_clean_target(s)
        || is_idleassetsd_cfnetwork_tmp_clean_target(s)
        || (is_code_sign_clone_clean_target(s) && !is_endpoint_security_cache_path(s))
        || (is_gpu_metal_cache_clean_target(s) && !is_endpoint_security_cache_path(s))
        || is_install_macos_app_bundle(path, &applications_root())
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
    fn allowlist_accepts_private_tmp_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/tmp/old.file"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/tmp/old.file"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/tmp/sub/old.file"
        )));
        assert!(!path_allowed_for_privilege(Path::new("/private/tmp")));
        assert!(!path_allowed_for_privilege(Path::new("/private/var/tmp")));
    }

    #[test]
    fn allowlist_accepts_library_caches_temp_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Caches/foo.cache"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Caches/com.apple.foo/a.tmp"
        )));
        assert!(path_allowed_for_privilege(Path::new(
            "/Library/Caches/a/b/c/d/e.log"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Caches/a/b/c/d/e/f.log"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/Library/Caches/foo.dat"
        )));
        assert!(!path_allowed_for_privilege(Path::new("/Library/Caches")));
    }

    #[test]
    fn allowlist_accepts_idleassetsd_cfnetwork_tmp_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/T/com.apple.idleassetsd/CFNetworkDownload_abc.tmp"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/C/com.apple.idleassetsd/CFNetworkDownload_abc.tmp"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/T/com.apple.idleassetsd/other.tmp"
        )));
    }

    #[test]
    fn allowlist_accepts_code_sign_clone_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/X/Foo.app.code_sign_clone"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/C/Foo.app.code_sign_clone"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/X/com.crowdstrike.falcon.App.code_sign_clone"
        )));
    }

    #[test]
    fn allowlist_accepts_gpu_metal_cache_targets() {
        assert!(path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/C/com.example.App/com.apple.metal"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/T/com.example.App/com.apple.metal"
        )));
        assert!(!path_allowed_for_privilege(Path::new(
            "/private/var/folders/zz/uid/C/com.crowdstrike.falcon.App/com.apple.metal"
        )));
    }

    #[test]
    fn library_caches_temp_age_days_splits_log() {
        assert_eq!(
            library_caches_temp_age_days(Path::new("/x/y.cache")),
            LIBRARY_CACHES_TEMP_AGE_DAYS
        );
        assert_eq!(
            library_caches_temp_age_days(Path::new("/x/y.tmp")),
            LIBRARY_CACHES_TEMP_AGE_DAYS
        );
        assert_eq!(
            library_caches_temp_age_days(Path::new("/x/y.log")),
            LIBRARY_CACHES_LOG_AGE_DAYS
        );
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

    #[test]
    fn recording_acquire_flips_probe_when_ok() {
        let b = RecordingPrivilege {
            acquire_ok: true,
            ..RecordingPrivilege::denying()
        };
        assert!(!b.probe_noninteractive());
        assert!(b.acquire_interactive());
        assert_eq!(*b.acquire_calls.lock().unwrap(), 1);
        assert!(b.probe_noninteractive());
        assert!(b.acquire_interactive());
        assert_eq!(*b.acquire_calls.lock().unwrap(), 2);
    }

    #[test]
    fn recording_acquire_can_fail_without_flipping_probe() {
        let b = RecordingPrivilege::denying();
        assert!(!b.acquire_interactive());
        assert_eq!(*b.acquire_calls.lock().unwrap(), 1);
        assert!(!b.probe_noninteractive());
    }

    fn write_swu_plist(path: &Path, recommended: plist::Value) {
        let mut dict = plist::Dictionary::new();
        dict.insert("RecommendedUpdates".into(), recommended);
        plist::Value::Dictionary(dict)
            .to_file_xml(path)
            .expect("write swu plist");
    }

    #[test]
    fn swu_empty_array_not_pending() {
        let dir = tempfile::tempdir().unwrap();
        let plist = dir.path().join("com.apple.SoftwareUpdate.plist");
        write_swu_plist(&plist, plist::Value::Array(vec![]));
        assert!(!software_update_pending_or_unknown(&plist));
    }

    #[test]
    fn swu_missing_file_is_pending() {
        let missing = PathBuf::from("/tmp/vole-no-such-swu-plist-xyz");
        assert!(software_update_pending_or_unknown(&missing));
    }

    #[test]
    fn swu_nonempty_recommended_is_pending() {
        let dir = tempfile::tempdir().unwrap();
        let plist = dir.path().join("com.apple.SoftwareUpdate.plist");
        write_swu_plist(
            &plist,
            plist::Value::Array(vec![plist::Value::Dictionary(plist::Dictionary::new())]),
        );
        assert!(software_update_pending_or_unknown(&plist));
    }

    #[test]
    fn is_install_macos_app_bundle_shape() {
        let root = Path::new("/Applications");
        assert!(is_install_macos_app_bundle(
            Path::new("/Applications/Install macOS Sequoia.app"),
            root
        ));
        assert!(!is_install_macos_app_bundle(
            Path::new("/Applications/Safari.app"),
            root
        ));
        assert!(!is_install_macos_app_bundle(
            Path::new("/tmp/Install macOS Sequoia.app"),
            root
        ));
    }

    #[test]
    fn allowlist_accepts_install_macos_under_apps_root() {
        let _guard = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("Applications");
        std::fs::create_dir_all(&apps).unwrap();
        let app = apps.join("Install macOS Fixtures.app");
        std::fs::create_dir_all(&app).unwrap();
        std::env::set_var("VOLE_TEST_APPLICATIONS", &apps);
        assert!(path_allowed_for_privilege(&app));
        assert!(!path_allowed_for_privilege(Path::new(
            "/tmp/Install macOS Evil.app"
        )));
        std::env::remove_var("VOLE_TEST_APPLICATIONS");
    }
}
