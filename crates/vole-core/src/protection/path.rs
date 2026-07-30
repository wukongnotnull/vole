//! `should_protect_path`（对齐 mole `app_protection.sh`）。

use std::path::Path;

use super::bundle::should_protect_data;
use super::data::ProtectionCatalog;
use crate::safety::is_endpoint_security_cache_path;

/// cleanup = 日常清理；uninstall = Mole `MOLE_UNINSTALL_MODE=1`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionMode {
    Cleanup,
    Uninstall,
}

fn ci_contains(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn is_orbstack_runtime_path(path: &str) -> bool {
    ci_contains(path, "Library/Group Containers/") && ci_contains(path, "dev.orbstack")
        || path.contains("/.orbstack")
        || path.ends_with("/.orbstack")
}

fn extract_container_bundle_id(path: &str) -> Option<String> {
    let markers = ["/Library/Containers/", "/Library/Group Containers/"];
    for marker in markers {
        if let Some(rest) = path.split(marker).nth(1) {
            if let Some(bundle) = rest.split('/').next() {
                return Some(bundle.to_string());
            }
        }
    }
    None
}

fn is_container_cache_or_tmp(path: &str) -> bool {
    path.contains("/Data/Library/Caches/") || path.contains("/Data/tmp/")
}

/// 路径保护。`Cleanup` 对齐现网；`Uninstall` 对齐 mole `MOLE_UNINSTALL_MODE=1`
///（不因 data-protected 拦截；仍拦 system-critical / EDR / 关键路径）。
pub fn should_protect_path(path: &str, catalog: &ProtectionCatalog, mode: ProtectionMode) -> bool {
    if path.is_empty() {
        return false;
    }

    if is_orbstack_runtime_path(path) {
        return true;
    }

    // 1. Keyword-based system UI components
    if ci_contains(path, "systemsettings")
        || ci_contains(path, "systempreferences")
        || ci_contains(path, "controlcenter")
        || ci_contains(path, "com.apple.settings")
        || ci_contains(path, "com.apple.notes")
    {
        return true;
    }

    // 2. System UI caches & containers
    if ci_contains(path, "com.apple.systempreferences.cache")
        || ci_contains(path, "com.apple.settings.cache")
        || ci_contains(path, "com.apple.controlcenter.cache")
        || ci_contains(path, "com.apple.finder.cache")
        || ci_contains(path, "com.apple.dock.cache")
        || path.contains("/Library/Containers/com.apple.Settings")
        || path.contains("/Library/Containers/com.apple.SystemSettings")
        || path.contains("/Library/Containers/com.apple.controlcenter")
        || path.contains("/Library/Group Containers/com.apple.systempreferences")
        || path.contains("/Library/Group Containers/com.apple.Settings")
        || path.contains("/com.apple.sharedfilelist/")
            && (ci_contains(path, "com.apple.settings")
                || ci_contains(path, "com.apple.systemsettings")
                || ci_contains(path, "systempreferences"))
    {
        return true;
    }

    // 3. Sandbox bundle IDs
    let mut container_cache = false;
    if let Some(bundle_id) = extract_container_bundle_id(path) {
        if is_container_cache_or_tmp(path) {
            container_cache = true;
        } else if mode == ProtectionMode::Cleanup && should_protect_data(&bundle_id, catalog) {
            return true;
        }
    }

    // 4. Hardcoded critical patterns
    if path.contains("com.apple.Settings")
        || path.contains("com.apple.SystemSettings")
        || path.contains("com.apple.controlcenter")
        || path.contains("com.apple.finder")
        || path.contains("com.apple.dock")
    {
        return true;
    }

    if is_endpoint_security_cache_path(path) {
        return true;
    }

    // 5. Preferences, Codex, iCloud, denylist caches
    if matches_critical_user_paths(path) {
        return true;
    }

    // 6. Full-path pattern lists
    if !container_cache && !is_explicit_clean_cache_path(path) {
        let matched = match mode {
            ProtectionMode::Cleanup => catalog.matches_cleanup_pattern(path),
            ProtectionMode::Uninstall => {
                if crate::protection::uninstall::path_matches_apple_uninstallable(path, catalog) {
                    false
                } else {
                    catalog.matches_system_critical(path)
                }
            }
        };
        if matched {
            return true;
        }
    }

    // 7. Filename fallback — cleanup only（uninstall：用户已显式选择卸载）。
    if mode == ProtectionMode::Cleanup && !container_cache {
        if let Some(name) = Path::new(path).file_name().and_then(|n| n.to_str()) {
            if !is_explicit_clean_cache_path(path) && should_protect_data(name, catalog) {
                return true;
            }
        }
    }

    false
}

/// Paths that explicit clean rules may target: user caches/logs and Electron cache dirs.
/// Broad bundle guards (protection.toml) still apply to Application Support data.
fn is_explicit_clean_cache_path(path: &str) -> bool {
    if path.contains("/.cache/") {
        return true;
    }
    // User home Library/Caches & Logs (not system /Library/...).
    if (path.contains("/Library/Caches/") && !path.starts_with("/Library/Caches/"))
        || (path.contains("/Library/Logs/") && !path.starts_with("/Library/Logs/"))
    {
        return true;
    }
    // mole user.sh `_clean_recent_items`: fixed Recent*.sfl(2) + recentitems.plist.
    // Filename is `com.apple.*`, so step-7 bundle guards would otherwise block them.
    if is_explicit_recent_items_path(path) {
        return true;
    }
    // mole `clean_dev_jetbrains_toolbox`: version dirs under Toolbox/apps.
    if is_explicit_jetbrains_toolbox_apps_path(path) {
        return true;
    }
    const CACHE_SEGMENTS: &[&str] = &[
        "/Cache/",
        "/Code Cache/",
        "/GPUCache/",
        "/CachedData/",
        "/CachedExtensionVSIXs/",
        "/sentry/",
        "/DawnGraphiteCache/",
        "/DawnWebGPUCache/",
    ];
    CACHE_SEGMENTS.iter().any(|seg| path.contains(seg))
}

fn is_explicit_recent_items_path(path: &str) -> bool {
    if path.ends_with("/Library/Preferences/com.apple.recentitems.plist") {
        return true;
    }
    let Some(name) = Path::new(path).file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !(name.ends_with(".sfl") || name.ends_with(".sfl2")) {
        return false;
    }
    if !path.contains("/Library/Application Support/com.apple.sharedfilelist/") {
        return false;
    }
    name.starts_with("com.apple.LSSharedFileList.RecentApplications.")
        || name.starts_with("com.apple.LSSharedFileList.RecentDocuments.")
        || name.starts_with("com.apple.LSSharedFileList.RecentServers.")
        || name.starts_with("com.apple.LSSharedFileList.RecentHosts.")
}

fn is_explicit_jetbrains_toolbox_apps_path(path: &str) -> bool {
    path.contains("/Library/Application Support/JetBrains/Toolbox/apps/")
}

fn matches_critical_user_paths(path: &str) -> bool {
    if path.ends_with("/Library/Preferences/com.apple.dock.plist")
        || path.ends_with("/Library/Preferences/com.apple.finder.plist")
    {
        return true;
    }
    if path.contains("/Library/Logs/mole") {
        return true;
    }
    if path.contains("/Library/Application Support/Codex")
        || path.contains("/Library/Logs/com.openai.codex")
        || path.contains("/.codex/sessions")
        || path.contains("/.codex/auth.json")
        || path.contains("/.codex/history.jsonl")
        || path.contains("/.codex/state_")
        || path.contains("/.codex/logs_")
        || path.contains("/.codex/session_index.jsonl")
        || path.contains("/.codex/cache/session_index.jsonl")
        || path.contains("/.codex/cache/codex_app_directory")
    {
        return true;
    }
    if path.contains("/ByHost/com.apple.bluetooth.") || path.contains("/ByHost/com.apple.wifi.") {
        return true;
    }
    if path.contains("/Library/Preferences/com.apple.networkextension") && path.ends_with(".plist")
    {
        return true;
    }
    if path.contains("/Library/Mobile Documents") || path.contains("/Mobile Documents") {
        return true;
    }
    if path.contains("/Library/Accounts")
        || path.contains("/Library/Keychains")
        || path.contains("/Library/Mail")
        || path.contains("/Library/Calendars")
        || path.contains("/Library/Contacts")
    {
        return true;
    }
    if path.starts_with("/Library/Audio/Plug-Ins/")
        || path.contains("/Library/Application Support/iZotope")
        || path.contains("/Library/Application Support/LaserSoft Imaging")
    {
        return true;
    }
    if path.contains("/Library/Preferences/com.native-instruments")
        || path.contains("/Library/Preferences/com.avid.mediacomposer")
        || path.contains("/Library/Preferences/com.fabfilter.")
        || path.contains("/Library/Preferences/com.paceap.")
    {
        return true;
    }
    if path.starts_with("/private/var/folders/")
        && (path.contains("/C/com.native-instruments")
            || path.contains("/C/com.avid.mediacomposer")
            || path.contains("/C/com.paceap.eden.iLokLicenseManager"))
    {
        return true;
    }
    if path.contains("/Library/Caches/ms-playwright")
        || path.contains("/Library/Caches/com.apple.homed")
        || path.contains("/Library/Caches/com.apple.containermanagerd")
        || path.contains("/Library/Caches/com.apple.ap.adprivacyd")
        || path.contains("/Library/Caches/FamilyCircle")
        || path.contains("/Library/Caches/com.apple.HomeKit")
        || path.contains(
            "/Library/Caches/com.apple.WorkflowKit.BackgroundShortcutRunner.ShortcutsSandboxCache",
        )
        || path.contains("/Library/Caches/com.apple.siriactionsd.ShortcutsSandboxCache")
        || path.contains("/Library/Caches/app.cotypist.Cotypist")
        || path.contains("/Library/Caches/com.displaylink.DisplayLinkUserAgent")
        || path.contains("/Library/Caches/com.lasersoft-imaging.")
        || path.contains("/Library/Caches/Adobe ")
        || path.contains("/Library/Caches/") && path.contains(" Adobe")
    {
        return true;
    }
    if ci_contains(path, "com.apple.coreaudio")
        || ci_contains(path, "com.apple.audio.")
        || ci_contains(path, "coreaudiod")
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> ProtectionCatalog {
        ProtectionCatalog::embedded()
    }

    #[test]
    fn explicit_cache_allows_protected_bundle_cache_dirs() {
        let c = cat();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/test".into());
        assert!(!should_protect_path(
            &format!("{home}/Library/Caches/com.navicat.premium"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(!should_protect_path(
            &format!("{home}/Library/Caches/com.dbeaver.DBeaver"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(!should_protect_path(
            &format!("{home}/Library/Caches/com.postmanlabs.mac/item"),
            &c,
            ProtectionMode::Cleanup
        ));
    }

    #[test]
    fn explicit_recent_items_lists_are_allowed_but_settings_lists_stay_protected() {
        let c = cat();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/test".into());
        let shared = format!("{home}/Library/Application Support/com.apple.sharedfilelist");
        assert!(!should_protect_path(
            &format!("{shared}/com.apple.LSSharedFileList.RecentApplications.sfl2"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(!should_protect_path(
            &format!("{home}/Library/Preferences/com.apple.recentitems.plist"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(should_protect_path(
            &format!("{shared}/com.apple.LSSharedFileList.FavoriteVolumes.sfl2"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(should_protect_path(
            &format!("{shared}/com.apple.settings.sfl2"),
            &c,
            ProtectionMode::Cleanup
        ));
    }

    #[test]
    fn high_risk_cleanup_denylist() {
        let c = cat();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/test".into());
        assert!(should_protect_path(
            &format!("{home}/Library/Caches/ms-playwright/chromium-123"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(should_protect_path(
            &format!("{home}/Library/Caches/com.apple.homed/state"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(should_protect_path(
            &format!("{home}/Library/Group Containers/group.com.apple.notes/NoteStore.sqlite"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(should_protect_path(
            &format!("{home}/Library/Preferences/com.paceap.eden.iLokLicenseManager.plist"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(should_protect_path(
            "/private/var/folders/aa/bb/C/com.native-instruments.NativeAccess/license",
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(should_protect_path(
            "/Library/Audio/Plug-Ins/VST3/Example.vst3",
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(!should_protect_path(
            &format!("{home}/Library/Application Support/Example/Cache/item"),
            &c,
            ProtectionMode::Cleanup
        ));
    }

    #[test]
    fn protects_edr_caches() {
        let c = cat();
        assert!(should_protect_path(
            "/private/var/folders/9d/abc123/C/com.crowdstrike.falcon.App/com.apple.metalfe",
            &c,
            ProtectionMode::Cleanup
        ));
    }

    #[test]
    fn protects_orbstack_paths() {
        let c = cat();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/test".into());
        assert!(should_protect_path(
            &format!("{home}/Library/Group Containers/HUAQ24HBR6.dev.orbstack/data/data.img.raw"),
            &c,
            ProtectionMode::Cleanup
        ));
        assert!(should_protect_path(
            &format!("{home}/.orbstack/state.db"),
            &c,
            ProtectionMode::Cleanup
        ));
    }

    #[test]
    fn uninstall_mode_allows_data_protected_user_cache() {
        let catalog = ProtectionCatalog::embedded();
        let bundle = "com.freemacsoft.AppCleaner";
        assert!(catalog.matches_data_protected(bundle));
        // Application Support（非 explicit cache 路径）在 cleanup 下受 data_protected 保护。
        let path = format!("/Users/test/Library/Application Support/{bundle}");
        assert!(should_protect_path(
            &path,
            &catalog,
            ProtectionMode::Cleanup
        ));
        assert!(!should_protect_path(
            &path,
            &catalog,
            ProtectionMode::Uninstall
        ));
    }

    #[test]
    fn uninstall_mode_still_blocks_system_critical_path_keywords() {
        let catalog = ProtectionCatalog::embedded();
        let path = "/Users/test/Library/Caches/com.apple.finder.cache";
        assert!(should_protect_path(
            path,
            &catalog,
            ProtectionMode::Uninstall
        ));
    }
}
