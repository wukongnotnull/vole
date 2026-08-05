//! Orphan 判定用的纯函数（年龄、敏感族、路径 → bundle id）。

use std::path::Path;

use super::{DEFAULT_ORPHAN_AGE_DAYS, MIN_ORPHAN_AGE_DAYS};

/// 读 `MOLE_ORPHAN_AGE_DAYS`；非法或低于下限则回退默认。
pub fn orphan_age_days_from_env() -> u32 {
    orphan_age_days_from_raw(std::env::var("MOLE_ORPHAN_AGE_DAYS").ok().as_deref())
}

pub fn orphan_age_days_from_raw(raw: Option<&str>) -> u32 {
    let Some(s) = raw else {
        return DEFAULT_ORPHAN_AGE_DAYS;
    };
    let Ok(n) = s.trim().parse::<u32>() else {
        return DEFAULT_ORPHAN_AGE_DAYS;
    };
    if n < MIN_ORPHAN_AGE_DAYS {
        return DEFAULT_ORPHAN_AGE_DAYS;
    }
    n
}

/// 对齐 Mole `ORPHAN_NEVER_DELETE_PATTERNS`（大小写不敏感）。
pub fn is_sensitive_orphan_bundle(bundle_id: &str) -> bool {
    let lower = bundle_id.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "1password",
        "keychain",
        "bitwarden",
        "lastpass",
        "keepass",
        "dashlane",
        "enpass",
        "ssh",
        "gpg",
        "gnupg",
    ];
    if NEEDLES.iter().any(|n| lower.contains(n)) {
        return true;
    }
    lower.starts_with("com.apple.keychain")
}

/// 对齐 Mole `is_bundle_orphaned` 系统组件 case。
pub fn is_system_component_bundle(bundle_id: &str) -> bool {
    matches!(
        bundle_id.to_ascii_lowercase().as_str(),
        "loginwindow"
            | "dock"
            | "systempreferences"
            | "systemsettings"
            | "settings"
            | "controlcenter"
            | "finder"
            | "safari"
    )
}

/// 仅 com / org / net / io 顶层名（刻意不含 dev.* / app.*）。
pub fn matches_orphan_name_prefix(name: &str) -> bool {
    let base = name.strip_suffix(".savedState").unwrap_or(name);
    let base = base.strip_suffix(".plist").unwrap_or(base);
    let base = base.strip_suffix(".binarycookies").unwrap_or(base);
    base.starts_with("com.")
        || base.starts_with("org.")
        || base.starts_with("net.")
        || base.starts_with("io.")
}

/// 从 orphan 候选路径抽出 bundle id。
pub fn bundle_id_from_orphan_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let mut id = name.to_string();
    for suffix in [".savedState", ".plist", ".binarycookies"] {
        if let Some(stripped) = id.strip_suffix(suffix) {
            id = stripped.to_string();
            break;
        }
    }
    if id.is_empty() || !matches_orphan_name_prefix(&id) {
        return None;
    }
    Some(id)
}

pub fn resource_kind_label(path: &Path) -> &'static str {
    let s = path.to_string_lossy();
    if s.contains("/Library/Caches/") || s.ends_with("/Library/Caches") {
        "Caches"
    } else if s.contains("/Library/Logs/") || s.ends_with("/Library/Logs") {
        "Logs"
    } else if s.contains("/Saved Application State/") {
        "States"
    } else {
        "Data"
    }
}

pub fn orphan_label(path: &Path) -> String {
    let kind = resource_kind_label(path);
    let id = bundle_id_from_orphan_path(path).unwrap_or_else(|| "unknown".into());
    format!("Orphaned {kind}: {id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn age_clamp_rejects_zero_and_garbage() {
        assert_eq!(orphan_age_days_from_raw(None), 30);
        assert_eq!(orphan_age_days_from_raw(Some("0")), 30);
        assert_eq!(orphan_age_days_from_raw(Some("6")), 30);
        assert_eq!(orphan_age_days_from_raw(Some("7")), 7);
        assert_eq!(orphan_age_days_from_raw(Some("30")), 30);
        assert_eq!(orphan_age_days_from_raw(Some("nope")), 30);
        assert_eq!(orphan_age_days_from_raw(Some("-1")), 30);
    }

    #[test]
    fn sensitive_and_system_denylists() {
        assert!(is_sensitive_orphan_bundle("com.1password.1password"));
        assert!(is_sensitive_orphan_bundle("com.apple.keychain"));
        assert!(is_sensitive_orphan_bundle("org.gpg.agent"));
        assert!(is_system_component_bundle("finder"));
        assert!(is_system_component_bundle("safari"));
        assert!(!is_sensitive_orphan_bundle("com.example.cache"));
    }

    #[test]
    fn bundle_id_and_prefix_from_path() {
        let p = Path::new("/tmp/Library/Caches/com.example.app");
        assert_eq!(
            bundle_id_from_orphan_path(p).as_deref(),
            Some("com.example.app")
        );
        assert!(matches_orphan_name_prefix("com.example.app"));
        assert!(!matches_orphan_name_prefix("dev.orbstack.OrbStack"));
        let s = Path::new("/tmp/Library/Saved Application State/com.foo.savedState");
        assert_eq!(bundle_id_from_orphan_path(s).as_deref(), Some("com.foo"));
        assert_eq!(orphan_label(p), "Orphaned Caches: com.example.app");
    }
}
