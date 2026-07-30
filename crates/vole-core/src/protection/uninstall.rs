//! Uninstall 专用保护策略（对齐 mole `should_protect_from_uninstall` /
//! `official_uninstaller_vendor`）。

use std::path::Path;

use super::data::ProtectionCatalog;
use super::glob_match::bundle_matches_pattern;

/// `true` = 禁止卸载（system-critical 且不在 Apple 可卸 allowlist）。
pub fn should_protect_from_uninstall(bundle_id: &str, catalog: &ProtectionCatalog) -> bool {
    if bundle_id.is_empty() {
        return false;
    }
    if catalog.matches_apple_uninstallable(bundle_id) {
        return false;
    }
    catalog.matches_system_critical(bundle_id)
}

/// 命中则必须走官方卸载器；返回厂商名。
pub fn official_uninstaller_vendor(
    bundle_id: &str,
    display_name: &str,
    app_path: &str,
    catalog: &ProtectionCatalog,
) -> Option<String> {
    let normalized_bundle = bundle_id.to_ascii_lowercase();
    let normalized_name = display_name.to_ascii_lowercase();
    let basename = Path::new(app_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    for rule in catalog.official_uninstaller_rules() {
        for prefix in &rule.bundle_prefixes {
            let p = prefix.to_ascii_lowercase();
            if !p.is_empty() && normalized_bundle.starts_with(&p) {
                return Some(rule.vendor.clone());
            }
        }
        for fragment in &rule.name_fragments {
            let f = fragment.to_ascii_lowercase();
            if f.is_empty() {
                continue;
            }
            if normalized_name.contains(&f) || basename.contains(&f) {
                return Some(rule.vendor.clone());
            }
        }
    }
    None
}

/// 供 path.rs uninstall step 6：Apple 可卸 allowlist 用 path 匹配。
pub fn path_matches_apple_uninstallable(path: &str, catalog: &ProtectionCatalog) -> bool {
    catalog
        .apple_uninstallable_patterns()
        .iter()
        .any(|pat| bundle_matches_pattern(path, pat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_safari_but_not_xcode() {
        let cat = ProtectionCatalog::embedded();
        assert!(should_protect_from_uninstall("com.apple.Safari", &cat));
        assert!(!should_protect_from_uninstall("com.apple.dt.Xcode", &cat));
        assert!(!should_protect_from_uninstall("com.example.ThirdParty", &cat));
    }

    #[test]
    fn official_vendor_blocks_crowdstrike_and_jamf() {
        let cat = ProtectionCatalog::embedded();
        assert_eq!(
            official_uninstaller_vendor(
                "com.crowdstrike.falcon.UserAgent",
                "Falcon",
                "/Applications/Falcon.app",
                &cat
            )
            .as_deref(),
            Some("CrowdStrike")
        );
        assert_eq!(
            official_uninstaller_vendor(
                "com.jamf.management.Jamf",
                "Jamf Connect",
                "/Applications/Jamf Connect.app",
                &cat
            )
            .as_deref(),
            Some("Jamf")
        );
        assert!(official_uninstaller_vendor(
            "com.example.ok",
            "Example",
            "/Applications/Example.app",
            &cat
        )
        .is_none());
    }
}
