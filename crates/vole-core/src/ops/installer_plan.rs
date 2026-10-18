//! `installer` plan：扫描安装包 → ProtoPlan（`rule_id` 前缀 `installer:`）。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::protection::AppProtection;
use crate::safety::{capture_plan_entry_identity, validate_path_for_deletion, PathProtection};
use crate::vole_proto::{Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, SCHEMA_VERSION};

/// Mole `INSTALLER_SCAN_MAX_DEPTH_DEFAULT`.
pub const DEFAULT_INSTALLER_SCAN_MAX_DEPTH: usize = 2;

/// Mole `MAX_ZIP_ENTRIES`.
const MAX_ZIP_ENTRIES: usize = 50;

/// 直接候选扩展名（小写、无点）。
const DIRECT_EXTS: &[&str] = &["dmg", "pkg", "mpkg", "iso", "xip"];

/// Mole `INSTALLER_SCAN_PATHS`：相对 `$HOME` 的片段；以 `/` 开头表示绝对。
const DEFAULT_SCAN_RELS: &[&str] = &[
    "Downloads",
    "Desktop",
    "Documents",
    "Public",
    "Library/Downloads",
    "/Users/Shared",
    "/Users/Shared/Downloads",
    "Library/Caches/Homebrew",
    "Library/Mobile Documents/com~apple~CloudDocs/Downloads",
    "Library/Containers/com.apple.mail/Data/Library/Mail Downloads",
    "Library/Application Support/Telegram Desktop",
    "Downloads/Telegram Desktop",
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstallerPlanError {
    #[error("HOME not usable: {0}")]
    Home(String),
}

pub struct InstallerPlanOptions<'a> {
    pub home: &'a Path,
    pub ttl_secs: u64,
    /// 测试注入：覆盖扫描根；`None` 则用 Mole 风格默认表。
    pub scan_roots: Option<&'a [PathBuf]>,
    pub max_depth: usize,
    pub now: SystemTime,
}

pub fn build_installer_plan(
    protection: &AppProtection,
    opts: &InstallerPlanOptions<'_>,
) -> Result<ProtoPlan, InstallerPlanError> {
    if !opts.home.is_absolute() {
        return Err(InstallerPlanError::Home(opts.home.display().to_string()));
    }

    let roots = match opts.scan_roots {
        Some(r) => r.to_vec(),
        None => resolve_default_scan_roots(opts.home),
    };

    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();

    for root in &roots {
        if !root.is_dir() {
            continue;
        }
        for candidate in scan_installers_in_path(root, opts.max_depth) {
            let Ok(canon) = candidate.canonicalize() else {
                continue;
            };
            if !seen.insert(canon.clone()) {
                continue;
            }
            let ext = file_ext_lower(&canon).unwrap_or_default();
            let rule_id = format!("installer:{ext}");
            let label = display_name(&canon, &installer_source_display(opts.home, &canon));
            if let Some(entry) = try_plan_entry(&canon, &label, &rule_id, protection) {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(ProtoPlan {
        schema_version: SCHEMA_VERSION,
        created_at: opts.now,
        ttl_secs: opts.ttl_secs,
        entries,
        // Mole 的 fd 与 find 两条扫描分支结果一致（含隐藏文件、不读 ignore、
        // 不跟随 symlink），jwalk 单一实现即覆盖两者，没有跳过的长尾。
        coverage_note: None,
    })
}

pub fn resolve_default_scan_roots(home: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = BTreeSet::new();
    for rel in DEFAULT_SCAN_RELS {
        let p = if rel.starts_with('/') {
            PathBuf::from(rel)
        } else {
            home.join(rel)
        };
        if p.is_dir() && seen.insert(p.clone()) {
            roots.push(p);
        }
    }
    roots
}

fn scan_installers_in_path(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let walker = jwalk::WalkDir::new(root)
        .max_depth(max_depth)
        .skip_hidden(false);
    for entry in walker.into_iter().flatten() {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        if handle_candidate_file(&path) {
            out.push(path);
        }
    }
    out
}

fn handle_candidate_file(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if meta.file_type().is_symlink() {
        return false;
    }
    let Some(ext) = file_ext_lower(path) else {
        return false;
    };
    if DIRECT_EXTS.contains(&ext.as_str()) {
        return true;
    }
    if ext == "zip" {
        return is_installer_zip(path);
    }
    false
}

fn file_ext_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

fn is_installer_zip(path: &Path) -> bool {
    let listing = zip_list_entries(path);
    let Ok(text) = listing else {
        return false;
    };
    zip_listing_has_installer(&text)
}

/// Mole `head -n "$MAX_ZIP_ENTRIES" | awk …`：只看前 50 行条目。
fn zip_listing_has_installer(listing: &str) -> bool {
    listing
        .lines()
        .take(MAX_ZIP_ENTRIES)
        .any(|line| zip_line_looks_like_installer(&line.to_ascii_lowercase()))
}

fn zip_line_looks_like_installer(line: &str) -> bool {
    // Mole awk: /\.(app|pkg|dmg|xip)(\/|$)/
    for ext in ["app", "pkg", "dmg", "xip"] {
        let pat = format!(".{ext}");
        let mut start = 0;
        while let Some(rel) = line[start..].find(&pat) {
            let idx = start + rel;
            let end = idx + pat.len();
            if end == line.len() || line.as_bytes().get(end) == Some(&b'/') {
                return true;
            }
            start = idx + 1;
        }
    }
    false
}

fn zip_list_entries(path: &Path) -> Result<String, ()> {
    if let Ok(out) = Command::new("zipinfo").args(["-1"]).arg(path).output() {
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
    }
    if let Ok(out) = Command::new("unzip").args(["-Z", "-1"]).arg(path).output() {
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
    }
    Err(())
}

/// Mole `get_source_display`：按所在目录给出来源短名（Downloads、Homebrew、iCloud…）。
///
/// 计划里的路径已 canonicalize，`home` 可能是 symlink（如 `/var` → `/private/var`），
/// 所以同时按原样和 canonical 形式匹配。
pub fn installer_source_display(home: &Path, path: &Path) -> String {
    let dir = path.parent().unwrap_or(path);
    let mut homes = vec![home.to_path_buf()];
    if let Ok(canon) = home.canonicalize() {
        if canon != home {
            homes.push(canon);
        }
    }
    for h in &homes {
        if let Some(source) = source_under_home(h, dir) {
            return source.to_string();
        }
    }
    if dir.starts_with("/Users/Shared") {
        return "Shared".into();
    }
    if dir
        .components()
        .any(|c| c.as_os_str() == "Telegram Desktop")
    {
        return "Telegram".into();
    }
    dir.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn source_under_home(home: &Path, dir: &Path) -> Option<&'static str> {
    // 顺序与 Mole 的 case 分支一致：`~/Downloads/Telegram Desktop` 归入 Downloads。
    const SOURCES: &[(&str, &str)] = &[
        ("Downloads", "Downloads"),
        ("Desktop", "Desktop"),
        ("Documents", "Documents"),
        ("Public", "Public"),
        ("Library/Downloads", "Library"),
        ("Library/Caches/Homebrew", "Homebrew"),
        (
            "Library/Mobile Documents/com~apple~CloudDocs/Downloads",
            "iCloud",
        ),
        ("Library/Containers/com.apple.mail", "Mail"),
    ];
    let rel = dir.strip_prefix(home).ok()?;
    SOURCES
        .iter()
        .find(|(prefix, _)| rel.starts_with(prefix))
        .map(|(_, source)| *source)
}

fn display_name(path: &Path, source: &str) -> String {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("installer")
        .to_string();
    // Mole 只对 Homebrew 缓存去掉开头的 `<sha256>--`。
    if source == "Homebrew"
        && name.len() > 66
        && name.as_bytes().get(64) == Some(&b'-')
        && name.as_bytes().get(65) == Some(&b'-')
        && name[..64].chars().all(|c| c.is_ascii_hexdigit())
    {
        return name[66..].to_string();
    }
    name
}

fn try_plan_entry(
    path: &Path,
    label: &str,
    rule_id: &str,
    protection: &dyn PathProtection,
) -> Option<ProtoPlanEntry> {
    let path_str = path.display().to_string();
    validate_path_for_deletion(&path_str, protection).ok()?;
    let identity = capture_plan_entry_identity(path).ok()?;
    let size = fs::symlink_metadata(path).map(|m| m.len()).unwrap_or(0);
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
        blockers: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn plan_finds_dmg_and_pkg_skips_symlink() {
        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("Downloads");
        fs::create_dir_all(&downloads).unwrap();
        fs::write(downloads.join("App.dmg"), b"x").unwrap();
        fs::write(downloads.join("Setup.pkg"), b"y").unwrap();
        symlink(downloads.join("App.dmg"), downloads.join("link.dmg")).unwrap();
        fs::write(downloads.join("notes.txt"), b"z").unwrap();

        let protection = AppProtection::new();
        let roots = [downloads.clone()];
        let opts = InstallerPlanOptions {
            home: root.path(),
            ttl_secs: 900,
            scan_roots: Some(&roots),
            max_depth: 2,
            now: SystemTime::now(),
        };
        let plan = build_installer_plan(&protection, &opts).unwrap();
        let paths: Vec<_> = plan.entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(paths.len(), 2, "entries={:?}", plan.entries);
        assert!(paths.iter().any(|p| p.ends_with("App.dmg")));
        assert!(paths.iter().any(|p| p.ends_with("Setup.pkg")));
        assert!(plan
            .entries
            .iter()
            .all(|e| e.rule_id.starts_with("installer:")));
    }

    #[test]
    fn plan_respects_max_depth() {
        let root = tempfile::tempdir().unwrap();
        let deep = root.path().join("Downloads/a/b");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("Deep.dmg"), b"x").unwrap();
        fs::write(root.path().join("Downloads/Shallow.dmg"), b"y").unwrap();

        let protection = AppProtection::new();
        let roots = [root.path().join("Downloads")];
        let opts = InstallerPlanOptions {
            home: root.path(),
            ttl_secs: 900,
            scan_roots: Some(&roots),
            max_depth: 2,
            now: SystemTime::now(),
        };
        let plan = build_installer_plan(&protection, &opts).unwrap();
        assert!(
            plan.entries.iter().any(|e| e.path.ends_with("Shallow.dmg")),
            "shallow should match"
        );
        assert!(
            !plan.entries.iter().any(|e| e.path.ends_with("Deep.dmg")),
            "depth 3 file should be skipped with max_depth=2; entries={:?}",
            plan.entries
        );
    }

    #[test]
    fn zip_line_matcher_accepts_app_bundle() {
        assert!(zip_line_looks_like_installer(
            "Payload/Foo.app/Contents/Info.plist"
        ));
        assert!(zip_line_looks_like_installer("Foo.app"));
        assert!(zip_line_looks_like_installer("pkg/Bar.pkg"));
        assert!(!zip_line_looks_like_installer("readme.txt"));
        assert!(!zip_line_looks_like_installer("foo.apple/doc"));
    }

    #[test]
    fn installer_zip_with_app_is_selected_when_zip_tools_exist() {
        if Command::new("zip").arg("-h").output().is_err() {
            return;
        }
        let has_lister = Command::new("zipinfo").arg("-h").output().is_ok()
            || Command::new("unzip")
                .args(["-Z", "-h"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        if !has_lister {
            return;
        }

        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("Downloads");
        fs::create_dir_all(downloads.join("bundle/Foo.app/Contents")).unwrap();
        fs::write(downloads.join("bundle/Foo.app/Contents/Info.plist"), b"x").unwrap();
        let zip_path = downloads.join("Foo.zip");
        let status = Command::new("zip")
            .args(["-r", "-q"])
            .arg(&zip_path)
            .arg("Foo.app")
            .current_dir(downloads.join("bundle"))
            .status()
            .unwrap();
        assert!(status.success());

        let junk = downloads.join("docs.zip");
        fs::create_dir_all(downloads.join("docs")).unwrap();
        fs::write(downloads.join("docs/readme.txt"), b"hi").unwrap();
        let status = Command::new("zip")
            .args(["-r", "-q"])
            .arg(&junk)
            .arg("readme.txt")
            .current_dir(downloads.join("docs"))
            .status()
            .unwrap();
        assert!(status.success());

        let protection = AppProtection::new();
        let roots = [downloads.clone()];
        let plan = build_installer_plan(
            &protection,
            &InstallerPlanOptions {
                home: root.path(),
                ttl_secs: 900,
                scan_roots: Some(&roots),
                max_depth: 2,
                now: SystemTime::now(),
            },
        )
        .unwrap();
        assert!(
            plan.entries.iter().any(|e| e.path.ends_with("Foo.zip")),
            "installer zip missing: {:?}",
            plan.entries
        );
        assert!(
            !plan.entries.iter().any(|e| e.path.ends_with("docs.zip")),
            "plain zip should be skipped: {:?}",
            plan.entries
        );
    }

    fn plan_for(home: &Path, roots: &[PathBuf]) -> ProtoPlan {
        build_installer_plan(
            &AppProtection::new(),
            &InstallerPlanOptions {
                home,
                ttl_secs: 900,
                scan_roots: Some(roots),
                max_depth: DEFAULT_INSTALLER_SCAN_MAX_DEPTH,
                now: SystemTime::now(),
            },
        )
        .unwrap()
    }

    fn file_names(plan: &ProtoPlan) -> Vec<String> {
        plan.entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    // Mole installer.bats / installer_fd.bats：fd 与 find 分支共用的扫描用例。
    #[test]
    fn plan_matches_mole_scan_cases() {
        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("Downloads");
        fs::create_dir_all(downloads.join("sub")).unwrap();
        for name in [
            "My App Installer.dmg",
            "App-v1.2.3_beta.pkg",
            "Suite.mpkg",
            "Disk.iso",
            "Xcode.xip",
            "UPPER.DMG",
            ".hidden.dmg",
            "document.pdf",
            "image.jpg",
            "archive.tar.gz",
        ] {
            fs::write(downloads.join(name), b"x").unwrap();
        }
        fs::write(downloads.join("sub/Nested.pkg"), b"x").unwrap();

        let roots = [downloads, root.path().join("Missing")];
        let mut names = file_names(&plan_for(root.path(), &roots));
        names.sort();
        assert_eq!(
            names,
            vec![
                ".hidden.dmg",
                "App-v1.2.3_beta.pkg",
                "Disk.iso",
                "My App Installer.dmg",
                "Nested.pkg",
                "Suite.mpkg",
                "UPPER.DMG",
                "Xcode.xip",
            ]
        );
    }

    #[test]
    fn plan_has_no_coverage_gap_and_empty_dir_yields_nothing() {
        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("Downloads");
        fs::create_dir_all(&downloads).unwrap();
        fs::write(downloads.join("notes.txt"), b"x").unwrap();
        let plan = plan_for(root.path(), &[downloads]);
        assert!(plan.entries.is_empty(), "entries={:?}", plan.entries);
        assert_eq!(plan.coverage_note, None);
    }

    #[test]
    fn corrupt_zip_is_skipped() {
        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("Downloads");
        fs::create_dir_all(&downloads).unwrap();
        fs::write(
            downloads.join("corrupt.zip"),
            b"This is not a valid ZIP file",
        )
        .unwrap();
        let plan = plan_for(root.path(), &[downloads]);
        assert!(plan.entries.is_empty(), "entries={:?}", plan.entries);
    }

    #[test]
    fn zip_listing_only_checks_first_fifty_entries() {
        let mut listing: Vec<String> = (1..=MAX_ZIP_ENTRIES)
            .map(|i| format!("deep-content/file{i}.txt"))
            .collect();
        listing.push("deep-content/MyApp.app".into());
        assert!(!zip_listing_has_installer(&listing.join("\n")));

        listing.insert(MAX_ZIP_ENTRIES - 1, "deep-content/Setup.PKG".into());
        assert!(zip_listing_has_installer(&listing.join("\n")));
    }

    #[test]
    fn source_display_matches_mole_names() {
        let home = Path::new("/Users/u");
        let cases = [
            ("/Users/u/Downloads/a.dmg", "Downloads"),
            ("/Users/u/Downloads/Telegram Desktop/a.dmg", "Downloads"),
            ("/Users/u/Desktop/a.dmg", "Desktop"),
            ("/Users/u/Documents/x/a.dmg", "Documents"),
            ("/Users/u/Public/a.dmg", "Public"),
            ("/Users/u/Library/Downloads/a.dmg", "Library"),
            ("/Users/Shared/Downloads/a.dmg", "Shared"),
            (
                "/Users/u/Library/Caches/Homebrew/downloads/a.dmg",
                "Homebrew",
            ),
            (
                "/Users/u/Library/Mobile Documents/com~apple~CloudDocs/Downloads/a.dmg",
                "iCloud",
            ),
            (
                "/Users/u/Library/Containers/com.apple.mail/Data/Library/Mail Downloads/a.dmg",
                "Mail",
            ),
            (
                "/Users/u/Library/Application Support/Telegram Desktop/a.dmg",
                "Telegram",
            ),
            ("/Volumes/Ext/stash/a.dmg", "stash"),
        ];
        for (path, want) in cases {
            assert_eq!(
                installer_source_display(home, Path::new(path)),
                want,
                "{path}"
            );
        }
    }

    #[test]
    fn homebrew_hash_prefix_stripped_only_in_homebrew_cache() {
        let root = tempfile::tempdir().unwrap();
        let hashed = format!("{}--Foo-1.0.dmg", "a".repeat(64));
        let brew = root.path().join("Library/Caches/Homebrew/downloads");
        let downloads = root.path().join("Downloads");
        fs::create_dir_all(&brew).unwrap();
        fs::create_dir_all(&downloads).unwrap();
        fs::write(brew.join(&hashed), b"x").unwrap();
        fs::write(downloads.join(&hashed), b"y").unwrap();

        let roots = [root.path().join("Library/Caches/Homebrew"), downloads];
        let plan = plan_for(root.path(), &roots);
        let mut labels: Vec<_> = plan.entries.iter().map(|e| e.label.clone()).collect();
        labels.sort();
        assert_eq!(labels, vec!["Foo-1.0.dmg".to_string(), hashed]);
    }
}
