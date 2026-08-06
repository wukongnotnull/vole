//! Group Containers Logs/Caches/tmp 叶清理（Mole `clean_group_container_caches` 同形）。
//! 本期零保护层改动；见 design 2026-08-06-1133。

mod select;

pub use select::{select_group_container_caches, GroupCacheScanError, GroupCacheSelectResult};

use std::path::Path;

use crate::protection::{should_protect_data, ProtectionCatalog};

pub const GROUP_CONTAINER_CACHE_RULE_ID: &str = "group-container-caches";
pub const MAX_LEAVES_PER_CANDIDATE: usize = 200;
pub const MAX_LEAVES_TOTAL: usize = 2000;

pub fn is_apple_group_container(id: &str) -> bool {
    id.starts_with("com.apple.")
        || id.starts_with("group.com.apple.")
        || id.starts_with("systemgroup.com.apple.")
}

/// 剥前导 TeamID（恰好 10 位 `[A-Z0-9]` + `.`）。不匹配则原样返回。
pub fn strip_team_id_prefix(id: &str) -> &str {
    let bytes = id.as_bytes();
    if bytes.len() > 11
        && bytes[10] == b'.'
        && bytes[..10]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        &id[11..]
    } else {
        id
    }
}

pub fn is_group_container_protected(id: &str, catalog: &ProtectionCatalog) -> bool {
    let no_group = id.strip_prefix("group.").unwrap_or(id);
    let no_team = strip_team_id_prefix(id);
    let no_team_no_group = no_team.strip_prefix("group.").unwrap_or(no_team);
    should_protect_data(id, catalog)
        || should_protect_data(no_group, catalog)
        || should_protect_data(no_team, catalog)
        || should_protect_data(no_team_no_group, catalog)
}

pub fn group_container_cache_label(path: &Path, home: &Path) -> String {
    let root = home.join("Library/Group Containers");
    let rel = path
        .strip_prefix(&root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });
    format!("Group container cache: {rel}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protection::ProtectionCatalog;
    use std::path::Path;

    #[test]
    fn apple_prefix_variants() {
        assert!(is_apple_group_container("com.apple.notes"));
        assert!(is_apple_group_container("group.com.apple.notes"));
        assert!(is_apple_group_container("systemgroup.com.apple.notes"));
        assert!(!is_apple_group_container("group.com.example.app"));
        assert!(!is_apple_group_container("com.macpaw.CleanMyMac"));
    }

    #[test]
    fn strip_team_id_only_ten_alnum() {
        assert_eq!(
            strip_team_id_prefix("HUAQ24HBR6.dev.orbstack"),
            "dev.orbstack"
        );
        assert_eq!(
            strip_team_id_prefix("S8EX82NJP6.com.tencent.xinWeChat"),
            "com.tencent.xinWeChat"
        );
        assert_eq!(
            strip_team_id_prefix("group.com.example"),
            "group.com.example"
        );
        assert_eq!(strip_team_id_prefix("ABCDEFGHIJ.com.x"), "com.x");
        assert_eq!(strip_team_id_prefix("short.com.x"), "short.com.x");
        // 小写不匹配 ^[A-Z0-9]{10}
        assert_eq!(strip_team_id_prefix("abcdefghij.com.x"), "abcdefghij.com.x");
    }

    #[test]
    fn protected_via_raw_id() {
        let c = ProtectionCatalog::embedded();
        assert!(is_group_container_protected("com.macpaw.CleanMyMac", &c));
    }

    #[test]
    fn protected_via_group_strip() {
        let c = ProtectionCatalog::embedded();
        assert!(is_group_container_protected(
            "group.com.macpaw.CleanMyMac",
            &c
        ));
    }

    #[test]
    fn protected_via_teamid_strip_for_macpaw() {
        let c = ProtectionCatalog::embedded();
        assert!(is_group_container_protected(
            "S8EX82NJP6.com.macpaw.CleanMyMac",
            &c
        ));
    }

    #[test]
    fn non_protected_example_app() {
        let c = ProtectionCatalog::embedded();
        assert!(!is_group_container_protected("group.com.example.app", &c));
        assert!(!is_group_container_protected("com.example.app", &c));
    }

    #[test]
    fn label_uses_relative_under_group_containers() {
        let home = Path::new("/Users/t");
        let p =
            Path::new("/Users/t/Library/Group Containers/group.com.example.app/Library/Caches/foo");
        assert_eq!(
            group_container_cache_label(p, home),
            "Group container cache: group.com.example.app/Library/Caches/foo"
        );
    }
}
