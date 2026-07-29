//! EDR/MDM 代理的 Darwin 缓存路径（对齐 mole `is_endpoint_security_cache_path`）。

use std::path::Path;

use super::critical::path_is_within_existing_root;

const BUNDLE_PREFIXES: &[&str] = &[
    "com.crowdstrike.",
    "com.sentinelone.",
    "com.sentinel-labs.",
    "com.eset.",
    "com.jamf.",
    "com.jamfsoftware.",
    "com.paloaltonetworks.",
    "com.cisco.anyconnect",
    "com.cisco.secureclient",
];

fn in_var_folders_scope(path: &str) -> bool {
    if path.starts_with("/private/var/folders/") || path.starts_with("/var/folders/") {
        return true;
    }
    path_is_within_existing_root(Path::new(path), Path::new("/private/var/folders"))
}

/// 大小写不敏感子串匹配。
pub fn is_endpoint_security_cache_path(path: &str) -> bool {
    if !in_var_folders_scope(path) {
        return false;
    }
    let lower = path.to_ascii_lowercase();
    BUNDLE_PREFIXES
        .iter()
        .any(|p| lower.contains(&p.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_edr_caches() {
        assert!(is_endpoint_security_cache_path(
            "/private/var/folders/9d/abc/C/com.crowdstrike.falcon.App/com.apple.metalfe"
        ));
        assert!(is_endpoint_security_cache_path(
            "/private/var/folders/aa/bb/C/com.sentinelone.agent/com.apple.metal"
        ));
    }

    #[test]
    fn rejects_normal_app_cache() {
        assert!(!is_endpoint_security_cache_path(
            "/private/var/folders/aa/bb/C/com.example.App/com.apple.metalfe"
        ));
    }
}
