//! `uninstall` plan：扫描 Applications → 保护策略 → leftovers → ProtoPlan。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::brew_cask::{
    detect_cask_name, encode_brew_cask_rule_id, BrewDeps, LiveBrewDeps, ZapMode,
};
use crate::login_items::{
    discover_login_item_helper_bundle_ids, encode_login_helper_rule_id,
    encode_login_item_name_rule_id, login_name_collides,
};
use crate::protection::{
    find_app_leftovers, find_bundle_siblings, official_uninstaller_vendor, read_bundle_id,
    read_display_name, should_protect_from_uninstall, AppIdentity, AppProtection,
    ProtectionCatalog, UninstallPathProtection,
};
use crate::safety::{capture_plan_entry_identity, validate_path_for_deletion};
use crate::system_leftovers::{encode_system_leftover_rule_id, find_system_leftovers};
use crate::vole_proto::{Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, SCHEMA_VERSION};

use super::plan::DEFAULT_PLAN_TTL;
use super::OpsError;

pub struct UninstallPlanOptions<'a> {
    pub applications_dirs: &'a [PathBuf],
    pub home: &'a Path,
    pub target_bundle_or_name: Option<&'a str>,
    pub ttl_secs: u64,
}

pub fn default_applications_dirs(home: &Path) -> Vec<PathBuf> {
    vec![PathBuf::from("/Applications"), home.join("Applications")]
}

pub fn scan_applications(dirs: &[PathBuf]) -> Result<Vec<AppIdentity>, OpsError> {
    let mut apps = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            let Some(bundle_id) = read_bundle_id(&path) else {
                continue;
            };
            let display_name = read_display_name(&path).unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("app")
                    .to_string()
            });
            apps.push(AppIdentity {
                app_path: path,
                bundle_id,
                display_name,
            });
        }
    }
    apps.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(apps)
}

pub fn build_uninstall_plan(
    catalog: &ProtectionCatalog,
    protection: &AppProtection,
    opts: &UninstallPlanOptions<'_>,
) -> Result<ProtoPlan, OpsError> {
    build_uninstall_plan_with_brew(catalog, protection, opts, &LiveBrewDeps)
}

pub fn build_uninstall_plan_with_brew(
    catalog: &ProtectionCatalog,
    protection: &AppProtection,
    opts: &UninstallPlanOptions<'_>,
    brew: &dyn BrewDeps,
) -> Result<ProtoPlan, OpsError> {
    let apps = scan_applications(opts.applications_dirs)?;
    let target = opts
        .target_bundle_or_name
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());

    let mut skipped_filter = 0u64;
    let apps: Vec<AppIdentity> = match target {
        Some(ref t) => apps
            .into_iter()
            .filter(|app| {
                if app_matches_target(app, t) {
                    true
                } else {
                    skipped_filter += 1;
                    false
                }
            })
            .collect(),
        None => apps,
    };

    build_uninstall_plan_for_apps_with_brew_inner(
        catalog,
        protection,
        opts,
        &apps,
        brew,
        skipped_filter,
    )
}

pub fn build_uninstall_plan_for_apps(
    catalog: &ProtectionCatalog,
    protection: &AppProtection,
    opts: &UninstallPlanOptions<'_>,
    apps: &[AppIdentity],
) -> Result<ProtoPlan, OpsError> {
    build_uninstall_plan_for_apps_with_brew(catalog, protection, opts, apps, &LiveBrewDeps)
}

pub fn build_uninstall_plan_for_apps_with_brew(
    catalog: &ProtectionCatalog,
    protection: &AppProtection,
    opts: &UninstallPlanOptions<'_>,
    apps: &[AppIdentity],
    brew: &dyn BrewDeps,
) -> Result<ProtoPlan, OpsError> {
    build_uninstall_plan_for_apps_with_brew_inner(catalog, protection, opts, apps, brew, 0)
}

fn build_uninstall_plan_for_apps_with_brew_inner(
    catalog: &ProtectionCatalog,
    protection: &AppProtection,
    opts: &UninstallPlanOptions<'_>,
    apps: &[AppIdentity],
    brew: &dyn BrewDeps,
    skipped_filter: u64,
) -> Result<ProtoPlan, OpsError> {
    let mut entries = Vec::new();
    let mut skipped_protected = 0u64;
    let mut skipped_official = 0u64;
    let mut sibling_notes = 0u64;
    let mut brew_cask = 0u64;
    let mut login_items = 0u64;
    let mut system_leftovers = 0u64;

    let uninstall_protect = UninstallPathProtection::new(protection);
    let search_roots = opts.applications_dirs.to_vec();

    for app in apps {
        if should_protect_from_uninstall(&app.bundle_id, catalog) {
            skipped_protected += 1;
            continue;
        }
        if official_uninstaller_vendor(
            &app.bundle_id,
            &app.display_name,
            &app.app_path.display().to_string(),
            catalog,
        )
        .is_some()
        {
            skipped_official += 1;
            continue;
        }

        let siblings = find_bundle_siblings(&app.bundle_id, &app.app_path, &search_roots);
        if siblings.has_siblings() {
            sibling_notes += 1;
        }

        let sibling_names = sibling_display_names(&siblings);
        let name_collides = login_name_collides(&app.display_name, &sibling_names);

        // Login Item / helper 侧车先于本体，便于 apply 在删包前执行（且 PathVanished 仍可动作）。
        if !name_collides {
            if let Some(rule) = encode_login_item_name_rule_id(&app.display_name) {
                let label = format!("Login Item: {}", app.display_name);
                if let Some(entry) =
                    try_plan_entry_sized(&app.app_path, &label, &rule, &uninstall_protect, 0)
                {
                    entries.push(entry);
                    login_items += 1;
                }
            }
        }
        if !siblings.has_siblings() {
            for (_helper_path, helper_id) in discover_login_item_helper_bundle_ids(&app.app_path) {
                let rule = encode_login_helper_rule_id(&helper_id);
                let label = format!("LoginItems helper: {helper_id}");
                if let Some(entry) =
                    try_plan_entry_sized(&app.app_path, &label, &rule, &uninstall_protect, 0)
                {
                    entries.push(entry);
                    login_items += 1;
                }
            }
        }

        let leftovers = find_app_leftovers(app, opts.home, &siblings);
        let (rule_app, label) = if let Some(token) = detect_cask_name(brew, &app.app_path) {
            brew_cask += 1;
            let mode = if siblings.has_siblings() {
                ZapMode::NoZap
            } else {
                ZapMode::Zap
            };
            let rule = encode_brew_cask_rule_id(mode, &token);
            let label = format!("{} [Brew:{token}]", app.display_name);
            (rule, label)
        } else {
            (
                format!("uninstall:{}", app.bundle_id),
                app.display_name.clone(),
            )
        };

        if let Some(entry) = try_plan_entry(&app.app_path, &label, &rule_app, &uninstall_protect) {
            entries.push(entry);
        }

        for hit in leftovers {
            let rule = format!("uninstall:leftover:{}", app.bundle_id);
            if let Some(entry) = try_plan_entry(&hit.path, &hit.label, &rule, &uninstall_protect) {
                entries.push(entry);
            }
        }

        for hit in find_system_leftovers(app, &siblings) {
            let rule = encode_system_leftover_rule_id(hit.kind, &app.bundle_id, &hit.path);
            let label = format!("System leftover: {}", hit.label);
            if let Some(entry) = try_plan_entry(&hit.path, &label, &rule, &uninstall_protect) {
                entries.push(entry);
                system_leftovers += 1;
            }
        }
    }

    let coverage_note = Some(format!(
        "vole uninstall: skipped protected={skipped_protected}, official_uninstaller={skipped_official}, filter_miss={skipped_filter}, sibling_leftovers_suppressed={sibling_notes}, brew_cask={brew_cask}, login_items={login_items}, system_leftovers={system_leftovers}. \
Long-tail not covered: broad /Library system leftovers (Frameworks/kext/Plug-Ins/…) beyond LaunchDaemons/Agents/PHT and exact leaves."
    ));

    Ok(ProtoPlan {
        schema_version: SCHEMA_VERSION,
        created_at: SystemTime::now(),
        ttl_secs: if opts.ttl_secs == 0 {
            DEFAULT_PLAN_TTL.as_secs()
        } else {
            opts.ttl_secs
        },
        entries,
        coverage_note,
    })
}

fn app_matches_target(app: &AppIdentity, target: &str) -> bool {
    let name_l = app.display_name.to_ascii_lowercase();
    let bundle_l = app.bundle_id.to_ascii_lowercase();
    let stem = app
        .app_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    bundle_l.contains(target) || name_l.contains(target) || stem.contains(target)
}

fn try_plan_entry(
    path: &Path,
    label: &str,
    rule_id: &str,
    protection: &UninstallPathProtection<'_>,
) -> Option<ProtoPlanEntry> {
    let size = path_size(path);
    try_plan_entry_sized(path, label, rule_id, protection, size)
}

fn try_plan_entry_sized(
    path: &Path,
    label: &str,
    rule_id: &str,
    protection: &UninstallPathProtection<'_>,
    size: u64,
) -> Option<ProtoPlanEntry> {
    let path_str = path.display().to_string();
    validate_path_for_deletion(&path_str, protection).ok()?;
    let identity = capture_plan_entry_identity(path).ok()?;
    Some(ProtoPlanEntry {
        id: format!("{rule_id}:{}", path.display()),
        path: path.to_path_buf(),
        label: label.to_string(),
        size,
        rule_id: rule_id.to_string(),
        skip_reason: None,
        dev: identity.dev,
        ino: identity.ino,
        mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
    })
}

fn sibling_display_names(siblings: &crate::protection::SiblingPresence) -> Vec<String> {
    siblings
        .other_app_paths
        .iter()
        .map(|p| {
            read_display_name(p).unwrap_or_else(|| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            })
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn path_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    // 目录：浅层合计（足够 plan 预览；与 mole 全量 du 有差距但可接受于 M1）
    let mut total = 0u64;
    let walker = jwalk::WalkDir::new(path).skip_hidden(false);
    for entry in walker.into_iter().flatten() {
        if let Ok(m) = entry.metadata() {
            if m.is_file() {
                total = total.saturating_add(m.len());
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brew_cask::{CaskInstallState, ZapMode};
    use std::fs;

    struct FakeBrew {
        available: bool,
        resolve: Option<PathBuf>,
    }

    impl BrewDeps for FakeBrew {
        fn brew_available(&self) -> bool {
            self.available
        }
        fn list_casks(&self) -> Option<Vec<String>> {
            Some(vec![])
        }
        fn cask_info(&self, _: &str) -> Option<String> {
            Some(String::new())
        }
        fn is_cask_installed(&self, _: &str) -> CaskInstallState {
            CaskInstallState::Unknown
        }
        fn uninstall_cask(&self, _: &str, _: ZapMode, _: Option<&Path>) -> Result<(), String> {
            Ok(())
        }
        fn resolve_path(&self, _: &Path) -> Option<PathBuf> {
            self.resolve.clone()
        }
        fn read_symlink(&self, _: &Path) -> Option<PathBuf> {
            None
        }
        fn find_caskroom_apps(&self, _: &str) -> Vec<PathBuf> {
            vec![]
        }
    }

    #[test]
    fn build_plan_includes_app_and_leftover_skips_safari() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        write_app(&apps.join("Foo.app"), "com.example.foo", "Foo");
        write_app(&apps.join("Safari.app"), "com.apple.Safari", "Safari");
        fs::create_dir_all(home.join("Library/Caches/com.example.foo")).unwrap();

        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: &[apps],
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan(&catalog, &protection, &opts).unwrap();
        assert!(plan
            .entries
            .iter()
            .any(|e| e.path.ends_with("Foo.app") && e.rule_id.starts_with("uninstall:")));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.path.ends_with("Library/Caches/com.example.foo")));
        assert!(!plan.entries.iter().any(|e| e.path.ends_with("Safari.app")));
        assert!(plan.coverage_note.as_ref().unwrap().contains("protected"));
    }

    #[test]
    fn plan_marks_brew_cask_with_zap() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        let foo = apps.join("Foo.app");
        write_app(&foo, "com.example.foo", "Foo");

        let brew = FakeBrew {
            available: true,
            resolve: Some(PathBuf::from("/opt/homebrew/Caskroom/foo/1.0/Foo.app")),
        };
        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: std::slice::from_ref(&apps),
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan_with_brew(&catalog, &protection, &opts, &brew).unwrap();
        let entry = plan
            .entries
            .iter()
            .find(|e| e.path.ends_with("Foo.app") && e.rule_id.starts_with("uninstall:brew-cask:"))
            .expect("foo app");
        assert_eq!(entry.rule_id, "uninstall:brew-cask:zap:foo");
        assert!(entry.label.contains("[Brew:foo]"));
    }

    #[test]
    fn plan_marks_nozap_when_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        write_app(&apps.join("Foo.app"), "com.example.foo", "Foo");
        write_app(&apps.join("Foo Copy.app"), "com.example.foo", "Foo Copy");

        let brew = FakeBrew {
            available: true,
            resolve: Some(PathBuf::from("/opt/homebrew/Caskroom/foo/1.0/Foo.app")),
        };
        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: std::slice::from_ref(&apps),
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan_with_brew(&catalog, &protection, &opts, &brew).unwrap();
        let entry = plan
            .entries
            .iter()
            .find(|e| e.path.ends_with("Foo.app") && e.rule_id.starts_with("uninstall:brew-cask:"))
            .expect("foo");
        assert_eq!(entry.rule_id, "uninstall:brew-cask:nozap:foo");
    }

    #[test]
    fn coverage_note_drops_brew_cask_long_tail() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        write_app(&apps.join("Foo.app"), "com.example.foo", "Foo");
        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: &[apps],
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan(&catalog, &protection, &opts).unwrap();
        let note = plan.coverage_note.unwrap();
        assert!(!note.to_ascii_lowercase().contains("brew cask zap"));
        assert!(!note.to_ascii_lowercase().contains("login items"));
        assert!(!note.contains("system LaunchDaemons"));
        assert!(note.contains("system_leftovers="));
        assert!(note.contains("Frameworks") || note.contains("Plug-Ins"));
        assert!(
            !note.to_ascii_lowercase().contains("mole"),
            "uninstall coverage_note must not mention Mole: {note}"
        );
    }

    #[test]
    fn plan_emits_system_leftover_sidecar() {
        let _guard = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        write_app(&apps.join("Foo.app"), "com.example.foo", "FooApp");

        let lib = home.join("Library");
        fs::create_dir_all(lib.join("LaunchDaemons")).unwrap();
        fs::write(lib.join("LaunchDaemons/com.example.foo.plist"), b"{}").unwrap();
        fs::create_dir_all(lib.join("Application Support/FooApp")).unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: std::slice::from_ref(&apps),
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan(&catalog, &protection, &opts).unwrap();
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id.starts_with("uninstall:system-leftover:launchd:")));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id.starts_with("uninstall:system-leftover:library:")));
        assert!(plan
            .coverage_note
            .as_ref()
            .unwrap()
            .contains("system_leftovers="));

        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
    }

    #[test]
    fn plan_skips_system_leftovers_when_sibling() {
        let _guard = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        write_app(&apps.join("Foo.app"), "com.example.foo", "FooApp");
        write_app(&apps.join("Foo Copy.app"), "com.example.foo", "Foo Copy");

        let lib = home.join("Library");
        fs::create_dir_all(lib.join("LaunchDaemons")).unwrap();
        fs::write(lib.join("LaunchDaemons/com.example.foo.plist"), b"{}").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: std::slice::from_ref(&apps),
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan(&catalog, &protection, &opts).unwrap();
        assert!(!plan
            .entries
            .iter()
            .any(|e| e.rule_id.starts_with("uninstall:system-leftover:")));

        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
    }

    #[test]
    fn plan_emits_login_item_and_helper_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        let foo = apps.join("Foo.app");
        write_app(&foo, "com.example.foo", "Foo");
        write_helper(
            &foo.join("Contents/Library/LoginItems/Helper.app"),
            "com.example.foo.helper",
        );

        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: std::slice::from_ref(&apps),
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan(&catalog, &protection, &opts).unwrap();
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id.starts_with("uninstall:login-item:name:")));
        assert!(plan
            .entries
            .iter()
            .any(|e| { e.rule_id == "uninstall:login-helper:com.example.foo.helper" }));
    }

    #[test]
    fn plan_skips_helper_when_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        let foo = apps.join("Foo.app");
        write_app(&foo, "com.example.foo", "Foo");
        write_helper(
            &foo.join("Contents/Library/LoginItems/Helper.app"),
            "com.example.foo.helper",
        );
        write_app(&apps.join("Foo Copy.app"), "com.example.foo", "Foo Copy");

        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: std::slice::from_ref(&apps),
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan(&catalog, &protection, &opts).unwrap();
        assert!(!plan
            .entries
            .iter()
            .any(|e| e.rule_id.starts_with("uninstall:login-helper:")));
    }

    #[test]
    fn plan_skips_login_name_when_display_collides() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        write_app(&apps.join("Foo.app"), "com.example.foo", "Foo");
        write_app(&apps.join("Foo Alternate.app"), "com.example.foo", "Foo");

        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let opts = UninstallPlanOptions {
            applications_dirs: std::slice::from_ref(&apps),
            home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan(&catalog, &protection, &opts).unwrap();
        assert!(!plan
            .entries
            .iter()
            .any(|e| e.rule_id.starts_with("uninstall:login-item:name:")));
    }

    #[test]
    fn plan_for_apps_only_includes_selected() {
        let dir = tempfile::tempdir().unwrap();
        let apps_dir = dir.path().join("Applications");
        fs::create_dir_all(&apps_dir).unwrap();
        write_app(
            &apps_dir.join("FixtureA.app"),
            "com.example.fixturea",
            "FixtureA",
        );
        write_app(
            &apps_dir.join("FixtureB.app"),
            "com.example.fixtureb",
            "FixtureB",
        );

        let scanned = scan_applications(std::slice::from_ref(&apps_dir)).unwrap();
        assert_eq!(scanned.len(), 2);
        let only = vec![scanned[0].clone()];

        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let home = dir.path().join("home");
        fs::create_dir_all(home.join("Library")).unwrap();
        let opts = UninstallPlanOptions {
            applications_dirs: &[],
            home: &home,
            target_bundle_or_name: None,
            ttl_secs: 900,
        };
        let plan = build_uninstall_plan_for_apps(&catalog, &protection, &opts, &only).unwrap();
        assert!(plan.entries.iter().any(|e| e.path == only[0].app_path));
        assert!(!plan.entries.iter().any(|e| e.path == scanned[1].app_path));
    }

    fn write_app(app: &Path, bundle_id: &str, name: &str) {
        let contents = app.join("Contents");
        fs::create_dir_all(&contents).unwrap();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{bundle_id}</string>
<key>CFBundleName</key><string>{name}</string>
</dict></plist>"#
        );
        fs::write(contents.join("Info.plist"), plist).unwrap();
    }

    fn write_helper(app: &Path, bundle_id: &str) {
        write_app(app, bundle_id, "Helper");
    }
}
