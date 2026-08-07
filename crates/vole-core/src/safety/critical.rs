//! 删除策略关键路径判定（对齐 mole `_mole_is_critical_deletion_path`）。

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// 折叠 `//`、去掉末尾 `/`（对齐 `_mole_normalize_deletion_policy_path`）。
pub fn normalize_policy_path(path: &str) -> String {
    let mut out = path.to_string();
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

fn same_existing_file(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (fs::symlink_metadata(a), fs::symlink_metadata(b)) else {
        return false;
    };
    ma.dev() == mb.dev() && ma.ino() == mb.ino()
}

pub(crate) fn path_is_within_existing_root(path: &Path, protected_root: &Path) -> bool {
    if !protected_root.exists() {
        return false;
    }
    let mut probe = path.to_path_buf();
    loop {
        if same_existing_file(&probe, protected_root) {
            return true;
        }
        if probe == Path::new("/") {
            break;
        }
        probe = probe.parent().unwrap_or(Path::new("/")).to_path_buf();
    }
    false
}

/// 仅删除策略；应用数据保护在 `protection` 模块。
pub fn is_critical_deletion_path(path: &str) -> bool {
    let path = normalize_policy_path(path);

    // Homebrew 子路径可删，根本身仍受保护。
    if path.starts_with("/usr/local/") || path.starts_with("/opt/homebrew/") {
        return false;
    }

    if matches!(
        path.as_str(),
        "/" | "/bin"
            | "/dev"
            | "/sbin"
            | "/usr"
            | "/System"
            | "/Library"
            | "/Library/Apple"
            | "/Library/Application Support"
            | "/Library/Extensions"
            | "/Library/Keychains"
            | "/Applications"
            | "/Applications/Finder.app"
            | "/Applications/Safari.app"
            | "/Volumes"
            | "/opt"
            | "/opt/homebrew"
            | "/Users"
            | "/Users/Shared"
            | "/Users/Guest"
            | "/private"
            | "/private/tmp"
            | "/etc"
            | "/private/etc"
            | "/var"
            | "/var/db"
            | "/var/audit"
            | "/var/root"
            | "/private/var"
            | "/private/var/tmp"
            | "/private/var/folders"
            | "/private/var/db"
            | "/private/var/audit"
            | "/private/var/root"
    ) || path.starts_with("/bin/")
        || path.starts_with("/dev/")
        || path.starts_with("/sbin/")
        || path.starts_with("/usr/")
        || path.starts_with("/System/")
        || path.starts_with("/Library/Apple/")
        || path.starts_with("/Library/Extensions/")
        || path.starts_with("/Library/Keychains/")
        || path.starts_with("/Applications/Finder.app/")
        || path.starts_with("/Applications/Safari.app/")
        || path.starts_with("/Users/Guest/")
        || path.starts_with("/etc/")
        || path.starts_with("/private/etc/")
        || path.starts_with("/var/db/")
        || path.starts_with("/var/audit/")
        || path.starts_with("/private/var/db/")
        || path.starts_with("/private/var/audit/")
    {
        return true;
    }

    // `/Users/<name>` 单段 home 根。
    if let Some(rest) = path.strip_prefix("/Users/") {
        if !rest.is_empty() && !rest.contains('/') {
            return true;
        }
    }

    const EXACT_ROOTS: &[&str] = &[
        "/",
        "/Applications",
        "/Library",
        "/Volumes",
        "/Network",
        "/cores",
        "/etc",
        "/home",
        "/net",
        "/tmp",
        "/var",
        "/private",
        "/private/tmp",
        "/private/var",
        "/private/var/tmp",
        "/private/var/folders",
        "/Users",
        "/opt",
        "/opt/homebrew",
    ];
    for root in EXACT_ROOTS {
        if same_existing_file(Path::new(&path), Path::new(root)) {
            return true;
        }
    }

    const PROTECTED_TREES: &[&str] = &[
        "/bin",
        "/dev",
        "/sbin",
        "/usr",
        "/System",
        "/private/etc",
        "/private/var/audit",
        "/private/var/db",
        "/private/var/root",
        "/Library/Apple",
        "/Library/Extensions",
        "/Library/Keychains",
        "/Applications/Finder.app",
        "/Applications/Safari.app",
    ];
    for root in PROTECTED_TREES {
        if path_is_within_existing_root(Path::new(&path), Path::new(root)) {
            return true;
        }
    }

    if let Some(parent) = Path::new(&path).parent() {
        if same_existing_file(parent, Path::new("/Users")) {
            return true;
        }
    }

    false
}

/// `/private` 下已知可删路径（对齐 mole `validate_path_for_deletion` allowlist）。
pub fn is_private_allowlisted(path: &str) -> bool {
    let path = normalize_policy_path(path);
    path.starts_with("/private/tmp/")
        || path.starts_with("/private/var/tmp/")
        || path == "/private/var/log"
        || path.starts_with("/private/var/log/")
        || path.starts_with("/private/var/folders/")
        || path == "/private/var/db/diagnostics"
        || path.starts_with("/private/var/db/diagnostics/")
        || path == "/private/var/db/DiagnosticPipeline"
        || path.starts_with("/private/var/db/DiagnosticPipeline/")
        || path == "/private/var/db/powerlog"
        || path.starts_with("/private/var/db/powerlog/")
        || path == "/private/var/db/reportmemoryexception"
        || path.starts_with("/private/var/db/reportmemoryexception/")
        || (path.starts_with("/private/var/db/receipts/")
            && (path.ends_with(".bom") || path.ends_with(".plist")))
}

pub fn is_coresymbolicationd_cache(path: &str) -> bool {
    let path = normalize_policy_path(path);
    path == "/System/Library/Caches/com.apple.coresymbolicationd/data"
        || path.starts_with("/System/Library/Caches/com.apple.coresymbolicationd/data/")
}

/// Rosetta 更新包（1.12.0）：仅 exact，禁止 `/Library/Apple/**` 泛放。
pub const ROSETTA_UPDATE_BUNDLE_LIVE: &str =
    "/Library/Apple/usr/share/rosetta/rosetta_update_bundle";

pub fn is_rosetta_update_bundle(path: &str) -> bool {
    let path = normalize_policy_path(path);
    if path == ROSETTA_UPDATE_BUNDLE_LIVE {
        return true;
    }
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let mapped = Path::new(&base).join("Apple/usr/share/rosetta/rosetta_update_bundle");
        if let Some(s) = mapped.to_str() {
            return path == normalize_policy_path(s);
        }
    }
    false
}

/// Icon Services 系统缓存（1.13.0）：仅 exact。
pub const ICON_SERVICES_SYSTEM_CACHE_LIVE: &str = "/Library/Caches/com.apple.iconservices.store";

pub fn is_icon_services_system_cache(path: &str) -> bool {
    let path = normalize_policy_path(path);
    if path == ICON_SERVICES_SYSTEM_CACHE_LIVE {
        return true;
    }
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let mapped = Path::new(&base).join("Caches/com.apple.iconservices.store");
        if let Some(s) = mapped.to_str() {
            return path == normalize_policy_path(s);
        }
    }
    false
}

/// 系统 DiagnosticReports 目录 marker（1.14.0）。
pub const DIAGNOSTIC_REPORTS_SYSTEM_MARKER_LIVE: &str = "/Library/Logs/DiagnosticReports/";

/// `/Library/Logs/DiagnosticReports/<leaf>` 单层叶（含测试 remap）。
pub fn is_system_diagnostic_report_leaf(path: &str) -> bool {
    let path = normalize_policy_path(path);
    if let Some(rest) = path.strip_prefix(DIAGNOSTIC_REPORTS_SYSTEM_MARKER_LIVE) {
        return !rest.is_empty() && !rest.contains('/');
    }
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let marker = format!(
            "{}/",
            Path::new(&base).join("Logs/DiagnosticReports").display()
        );
        let marker = normalize_policy_path(&marker);
        let marker = if marker.ends_with('/') {
            marker
        } else {
            format!("{marker}/")
        };
        if let Some(rest) = path.strip_prefix(&marker) {
            return !rest.is_empty() && !rest.contains('/');
        }
    }
    false
}

/// `/private/var/log`（1.15.0）：深度 ≤5、`.log`/`.gz`/`.asl`。
pub const PRIVATE_VAR_LOG_LIVE: &str = "/private/var/log";
pub const PRIVATE_VAR_LOG_MAX_DEPTH: usize = 5;

fn private_var_log_mapped_root() -> Option<PathBuf> {
    let base = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY")?;
    Some(PathBuf::from(base).parent()?.join("private/var/log"))
}

/// 是否为本规则允许的 clean 目标路径形状（不检查存在性 / 年龄）。
pub fn is_private_var_log_clean_target(path: &str) -> bool {
    let path = normalize_policy_path(path);
    let roots: Vec<String> = {
        let mut v = vec![PRIVATE_VAR_LOG_LIVE.to_string()];
        if let Some(mapped) = private_var_log_mapped_root() {
            if let Some(s) = mapped.to_str() {
                v.push(normalize_policy_path(s));
            }
        }
        v
    };
    for root in roots {
        let prefix = if root.ends_with('/') {
            root.clone()
        } else {
            format!("{root}/")
        };
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            return false;
        }
        let comps: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() || comps.len() > PRIVATE_VAR_LOG_MAX_DEPTH {
            return false;
        }
        if comps.iter().any(|c| *c == ".." || c.is_empty()) {
            return false;
        }
        let name = *comps.last().expect("non-empty");
        return name.ends_with(".log") || name.ends_with(".gz") || name.ends_with(".asl");
    }
    false
}

/// `/private/var/db/diagnostics`（1.16.0）：深度 ≤5 任意文件叶。
pub const PRIVATE_VAR_DB_DIAGNOSTICS_LIVE: &str = "/private/var/db/diagnostics";
pub const PRIVATE_VAR_DB_DIAGNOSTICS_MAX_DEPTH: usize = 5;

fn private_var_db_diagnostics_mapped_root() -> Option<PathBuf> {
    let base = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY")?;
    Some(
        PathBuf::from(base)
            .parent()?
            .join("private/var/db/diagnostics"),
    )
}

/// 是否为本规则允许的 clean 目标路径形状（不检查存在性 / 年龄 / 扩展名）。
pub fn is_private_var_db_diagnostics_clean_target(path: &str) -> bool {
    let path = normalize_policy_path(path);
    let roots: Vec<String> = {
        let mut v = vec![PRIVATE_VAR_DB_DIAGNOSTICS_LIVE.to_string()];
        if let Some(mapped) = private_var_db_diagnostics_mapped_root() {
            if let Some(s) = mapped.to_str() {
                v.push(normalize_policy_path(s));
            }
        }
        v
    };
    for root in roots {
        let prefix = if root.ends_with('/') {
            root.clone()
        } else {
            format!("{root}/")
        };
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            return false;
        }
        let comps: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() || comps.len() > PRIVATE_VAR_DB_DIAGNOSTICS_MAX_DEPTH {
            return false;
        }
        if comps.iter().any(|c| *c == ".." || c.is_empty()) {
            return false;
        }
        return true;
    }
    false
}

/// `/private/var/db/DiagnosticPipeline`（1.17.0）：深度 ≤5 任意文件叶。
pub const PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_LIVE: &str = "/private/var/db/DiagnosticPipeline";
pub const PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_MAX_DEPTH: usize = 5;

fn private_var_db_diagnostic_pipeline_mapped_root() -> Option<PathBuf> {
    let base = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY")?;
    Some(
        PathBuf::from(base)
            .parent()?
            .join("private/var/db/DiagnosticPipeline"),
    )
}

/// 是否为本规则允许的 clean 目标路径形状（不检查存在性 / 年龄）。
pub fn is_private_var_db_diagnostic_pipeline_clean_target(path: &str) -> bool {
    let path = normalize_policy_path(path);
    let roots: Vec<String> = {
        let mut v = vec![PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_LIVE.to_string()];
        if let Some(mapped) = private_var_db_diagnostic_pipeline_mapped_root() {
            if let Some(s) = mapped.to_str() {
                v.push(normalize_policy_path(s));
            }
        }
        v
    };
    for root in roots {
        let prefix = if root.ends_with('/') {
            root.clone()
        } else {
            format!("{root}/")
        };
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            return false;
        }
        let comps: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() || comps.len() > PRIVATE_VAR_DB_DIAGNOSTIC_PIPELINE_MAX_DEPTH {
            return false;
        }
        if comps.iter().any(|c| *c == ".." || c.is_empty()) {
            return false;
        }
        return true;
    }
    false
}

/// `/private/var/db/powerlog`（1.18.0）：深度 ≤5 任意文件叶。
pub const PRIVATE_VAR_DB_POWERLOG_LIVE: &str = "/private/var/db/powerlog";
pub const PRIVATE_VAR_DB_POWERLOG_MAX_DEPTH: usize = 5;

fn private_var_db_powerlog_mapped_root() -> Option<PathBuf> {
    let base = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY")?;
    Some(
        PathBuf::from(base)
            .parent()?
            .join("private/var/db/powerlog"),
    )
}

/// 是否为本规则允许的 clean 目标路径形状（不检查存在性 / 年龄）。
pub fn is_private_var_db_powerlog_clean_target(path: &str) -> bool {
    let path = normalize_policy_path(path);
    let roots: Vec<String> = {
        let mut v = vec![PRIVATE_VAR_DB_POWERLOG_LIVE.to_string()];
        if let Some(mapped) = private_var_db_powerlog_mapped_root() {
            if let Some(s) = mapped.to_str() {
                v.push(normalize_policy_path(s));
            }
        }
        v
    };
    for root in roots {
        let prefix = if root.ends_with('/') {
            root.clone()
        } else {
            format!("{root}/")
        };
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            return false;
        }
        let comps: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() || comps.len() > PRIVATE_VAR_DB_POWERLOG_MAX_DEPTH {
            return false;
        }
        if comps.iter().any(|c| *c == ".." || c.is_empty()) {
            return false;
        }
        return true;
    }
    false
}

/// `/private/var/db/reportmemoryexception/MemoryLimitViolations`（1.19.0）：深度 ≤5。
pub const PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_LIVE: &str =
    "/private/var/db/reportmemoryexception/MemoryLimitViolations";
pub const PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_MAX_DEPTH: usize = 5;

fn private_var_db_memory_limit_violations_mapped_root() -> Option<PathBuf> {
    let base = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY")?;
    Some(
        PathBuf::from(base)
            .parent()?
            .join("private/var/db/reportmemoryexception/MemoryLimitViolations"),
    )
}

/// 是否为本规则允许的 clean 目标路径形状（不检查存在性 / 年龄）。
pub fn is_private_var_db_memory_limit_violations_clean_target(path: &str) -> bool {
    let path = normalize_policy_path(path);
    let roots: Vec<String> = {
        let mut v = vec![PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_LIVE.to_string()];
        if let Some(mapped) = private_var_db_memory_limit_violations_mapped_root() {
            if let Some(s) = mapped.to_str() {
                v.push(normalize_policy_path(s));
            }
        }
        v
    };
    for root in roots {
        let prefix = if root.ends_with('/') {
            root.clone()
        } else {
            format!("{root}/")
        };
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            return false;
        }
        let comps: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() || comps.len() > PRIVATE_VAR_DB_MEMORY_LIMIT_VIOLATIONS_MAX_DEPTH {
            return false;
        }
        if comps.iter().any(|c| *c == ".." || c.is_empty()) {
            return false;
        }
        return true;
    }
    false
}

/// Adobe 系统日志（1.20.0）：Logs/Adobe、Logs/CreativeCloud 深度 ≤5，或 exact adobegc.log。
pub const ADOBE_SYSTEM_LOGS_MAX_DEPTH: usize = 5;
pub const ADOBE_LOGS_LIVE: &str = "/Library/Logs/Adobe";
pub const CREATIVE_CLOUD_LOGS_LIVE: &str = "/Library/Logs/CreativeCloud";
pub const ADOBEGC_LOG_LIVE: &str = "/Library/Logs/adobegc.log";

fn adobe_system_log_tree_roots() -> Vec<String> {
    let mut v = vec![
        ADOBE_LOGS_LIVE.to_string(),
        CREATIVE_CLOUD_LOGS_LIVE.to_string(),
    ];
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let base = PathBuf::from(base);
        for leaf in ["Adobe", "CreativeCloud"] {
            if let Some(s) = base.join("Logs").join(leaf).to_str() {
                v.push(normalize_policy_path(s));
            }
        }
    }
    v
}

fn adobegc_log_paths() -> Vec<String> {
    let mut v = vec![ADOBEGC_LOG_LIVE.to_string()];
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let mapped = PathBuf::from(base).join("Logs/adobegc.log");
        if let Some(s) = mapped.to_str() {
            v.push(normalize_policy_path(s));
        }
    }
    v
}

fn path_under_tree_max_depth(path: &str, root: &str, max_depth: usize) -> bool {
    let prefix = if root.ends_with('/') {
        root.to_string()
    } else {
        format!("{root}/")
    };
    let Some(rest) = path.strip_prefix(&prefix) else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    let comps: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    if comps.is_empty() || comps.len() > max_depth {
        return false;
    }
    !comps.iter().any(|c| *c == ".." || c.is_empty())
}

/// Adobe/CreativeCloud 树叶或 exact `adobegc.log`（不检查存在性 / 年龄）。
pub fn is_adobe_system_log_clean_target(path: &str) -> bool {
    let path = normalize_policy_path(path);
    if adobegc_log_paths().contains(&path) {
        return true;
    }
    adobe_system_log_tree_roots()
        .iter()
        .any(|root| path_under_tree_max_depth(&path, root, ADOBE_SYSTEM_LOGS_MAX_DEPTH))
}

/// `/private/tmp` + `/private/var/tmp`（1.21.0）：仅相对根深度 1 普通文件叶。
///
/// 故意严于 Mole `safe_sudo_find_delete` 的默认 maxdepth 5，对齐 probe 的 maxdepth 1。
pub const PRIVATE_TMP_MAX_DEPTH: usize = 1;
pub const PRIVATE_TMP_LIVE: &str = "/private/tmp";
pub const PRIVATE_VAR_TMP_LIVE: &str = "/private/var/tmp";

fn private_tmp_mapped_roots() -> Vec<String> {
    let mut v = vec![
        PRIVATE_TMP_LIVE.to_string(),
        PRIVATE_VAR_TMP_LIVE.to_string(),
    ];
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let parent = PathBuf::from(base).parent().map(PathBuf::from);
        if let Some(parent) = parent {
            for leaf in ["private/tmp", "private/var/tmp"] {
                if let Some(s) = parent.join(leaf).to_str() {
                    v.push(normalize_policy_path(s));
                }
            }
        }
    }
    v
}

/// 是否为本规则允许的 clean 目标路径形状（不检查存在性 / 年龄）。
pub fn is_private_tmp_clean_target(path: &str) -> bool {
    let path = normalize_policy_path(path);
    private_tmp_mapped_roots()
        .iter()
        .any(|root| path_under_tree_max_depth(&path, root, PRIVATE_TMP_MAX_DEPTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_slashes() {
        assert_eq!(normalize_policy_path("//etc//passwd"), "/etc/passwd");
        assert_eq!(normalize_policy_path("/tmp/"), "/tmp");
    }

    #[test]
    fn rejects_system_roots() {
        assert!(is_critical_deletion_path("/"));
        assert!(is_critical_deletion_path("/System"));
        assert!(is_critical_deletion_path("/usr/bin"));
        assert!(is_critical_deletion_path("/Users"));
    }

    #[test]
    fn allows_homebrew_children() {
        assert!(!is_critical_deletion_path("/opt/homebrew/Cellar/foo"));
    }

    #[test]
    fn rejects_single_user_home() {
        assert!(is_critical_deletion_path("/Users/alice"));
        assert!(!is_critical_deletion_path("/Users/alice/Library/Caches"));
    }

    #[test]
    fn rosetta_update_bundle_exact_only() {
        assert!(is_rosetta_update_bundle(
            "/Library/Apple/usr/share/rosetta/rosetta_update_bundle"
        ));
        assert!(is_rosetta_update_bundle(
            "/Library/Apple/usr/share/rosetta/rosetta_update_bundle/"
        ));
        assert!(!is_rosetta_update_bundle(
            "/Library/Apple/usr/share/rosetta"
        ));
        assert!(!is_rosetta_update_bundle(
            "/Library/Apple/usr/share/rosetta/rosetta_update_bundle/extra"
        ));
        assert!(!is_rosetta_update_bundle("/Library/Apple/other"));
        // critical 仍认整树；豁免走独立谓词。
        assert!(is_critical_deletion_path(
            "/Library/Apple/usr/share/rosetta/rosetta_update_bundle"
        ));
    }

    #[test]
    fn icon_services_system_cache_exact_only() {
        assert!(is_icon_services_system_cache(
            "/Library/Caches/com.apple.iconservices.store"
        ));
        assert!(is_icon_services_system_cache(
            "/Library/Caches/com.apple.iconservices.store/"
        ));
        assert!(!is_icon_services_system_cache("/Library/Caches"));
        assert!(!is_icon_services_system_cache(
            "/Library/Caches/com.apple.iconservices.store/extra"
        ));
        assert!(!is_icon_services_system_cache(
            "/Library/Caches/com.apple.other"
        ));
        assert!(!is_critical_deletion_path(
            "/Library/Caches/com.apple.iconservices.store"
        ));
    }

    #[test]
    fn system_diagnostic_report_leaf_exact_shape() {
        assert!(is_system_diagnostic_report_leaf(
            "/Library/Logs/DiagnosticReports/App.crash"
        ));
        assert!(!is_system_diagnostic_report_leaf(
            "/Library/Logs/DiagnosticReports"
        ));
        assert!(!is_system_diagnostic_report_leaf(
            "/Library/Logs/DiagnosticReports/"
        ));
        assert!(!is_system_diagnostic_report_leaf(
            "/Library/Logs/DiagnosticReports/sub/a.crash"
        ));
        assert!(!is_system_diagnostic_report_leaf(
            "/Library/Logs/other/App.crash"
        ));
        assert!(!is_critical_deletion_path(
            "/Library/Logs/DiagnosticReports/App.crash"
        ));
    }

    #[test]
    fn private_var_log_clean_target_shape() {
        assert!(is_private_var_log_clean_target(
            "/private/var/log/system.log"
        ));
        assert!(is_private_var_log_clean_target(
            "/private/var/log/a/b/c/d/e.log"
        ));
        assert!(!is_private_var_log_clean_target(
            "/private/var/log/a/b/c/d/e/f.log"
        ));
        assert!(!is_private_var_log_clean_target("/private/var/log"));
        assert!(!is_private_var_log_clean_target(
            "/private/var/log/notes.txt"
        ));
        assert!(!is_private_var_log_clean_target("/var/log/system.log"));
        assert!(is_private_var_log_clean_target("/private/var/log/x.gz"));
        assert!(is_private_var_log_clean_target("/private/var/log/x.asl"));
    }

    #[test]
    fn private_var_db_diagnostics_clean_target_shape() {
        assert!(is_private_var_db_diagnostics_clean_target(
            "/private/var/db/diagnostics/log.data"
        ));
        assert!(is_private_var_db_diagnostics_clean_target(
            "/private/var/db/diagnostics/a/b/c/d/e.tracev3"
        ));
        assert!(!is_private_var_db_diagnostics_clean_target(
            "/private/var/db/diagnostics/a/b/c/d/e/f.data"
        ));
        assert!(!is_private_var_db_diagnostics_clean_target(
            "/private/var/db/diagnostics"
        ));
        assert!(!is_private_var_db_diagnostics_clean_target(
            "/private/var/db/other/x"
        ));
        assert!(!is_private_var_db_diagnostics_clean_target(
            "/var/db/diagnostics/x"
        ));
    }

    #[test]
    fn private_var_db_diagnostic_pipeline_clean_target_shape() {
        assert!(is_private_var_db_diagnostic_pipeline_clean_target(
            "/private/var/db/DiagnosticPipeline/x.data"
        ));
        assert!(is_private_var_db_diagnostic_pipeline_clean_target(
            "/private/var/db/DiagnosticPipeline/a/b/c/d/e.data"
        ));
        assert!(!is_private_var_db_diagnostic_pipeline_clean_target(
            "/private/var/db/DiagnosticPipeline/a/b/c/d/e/f.data"
        ));
        assert!(!is_private_var_db_diagnostic_pipeline_clean_target(
            "/private/var/db/DiagnosticPipeline"
        ));
        assert!(!is_private_var_db_diagnostic_pipeline_clean_target(
            "/private/var/db/diagnostics/x"
        ));
    }

    #[test]
    fn private_var_db_powerlog_clean_target_shape() {
        assert!(is_private_var_db_powerlog_clean_target(
            "/private/var/db/powerlog/x.data"
        ));
        assert!(is_private_var_db_powerlog_clean_target(
            "/private/var/db/powerlog/a/b/c/d/e.data"
        ));
        assert!(!is_private_var_db_powerlog_clean_target(
            "/private/var/db/powerlog/a/b/c/d/e/f.data"
        ));
        assert!(!is_private_var_db_powerlog_clean_target(
            "/private/var/db/powerlog"
        ));
        assert!(!is_private_var_db_powerlog_clean_target(
            "/private/var/db/diagnostics/x"
        ));
    }

    #[test]
    fn private_var_db_memory_limit_violations_clean_target_shape() {
        assert!(is_private_var_db_memory_limit_violations_clean_target(
            "/private/var/db/reportmemoryexception/MemoryLimitViolations/x.data"
        ));
        assert!(is_private_var_db_memory_limit_violations_clean_target(
            "/private/var/db/reportmemoryexception/MemoryLimitViolations/a/b/c/d/e.data"
        ));
        assert!(!is_private_var_db_memory_limit_violations_clean_target(
            "/private/var/db/reportmemoryexception/MemoryLimitViolations/a/b/c/d/e/f.data"
        ));
        assert!(!is_private_var_db_memory_limit_violations_clean_target(
            "/private/var/db/reportmemoryexception/MemoryLimitViolations"
        ));
        assert!(!is_private_var_db_memory_limit_violations_clean_target(
            "/private/var/db/powerlog/x"
        ));
    }

    #[test]
    fn adobe_system_log_clean_target_shape() {
        assert!(is_adobe_system_log_clean_target(
            "/Library/Logs/Adobe/Installer/foo.log"
        ));
        assert!(is_adobe_system_log_clean_target(
            "/Library/Logs/CreativeCloud/a/b/c/d/e.log"
        ));
        assert!(!is_adobe_system_log_clean_target(
            "/Library/Logs/CreativeCloud/a/b/c/d/e/f.log"
        ));
        assert!(!is_adobe_system_log_clean_target("/Library/Logs/Adobe"));
        assert!(is_adobe_system_log_clean_target(
            "/Library/Logs/adobegc.log"
        ));
        assert!(!is_adobe_system_log_clean_target(
            "/Library/Logs/adobegc.log.bak"
        ));
        assert!(!is_adobe_system_log_clean_target(
            "/Library/Logs/DiagnosticReports/App.crash"
        ));
    }

    #[test]
    fn private_tmp_clean_target_shape() {
        assert!(is_private_tmp_clean_target("/private/tmp/old.file"));
        assert!(is_private_tmp_clean_target("/private/var/tmp/old.file"));
        assert!(!is_private_tmp_clean_target("/private/tmp/sub/old.file"));
        assert!(!is_private_tmp_clean_target("/private/var/tmp/a/b"));
        assert!(!is_private_tmp_clean_target("/private/tmp"));
        assert!(!is_private_tmp_clean_target("/private/var/tmp"));
        assert!(!is_private_tmp_clean_target("/tmp/old.file"));
    }
}
