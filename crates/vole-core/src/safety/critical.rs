//! 删除策略关键路径判定（对齐 mole `_mole_is_critical_deletion_path`）。

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

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
}
