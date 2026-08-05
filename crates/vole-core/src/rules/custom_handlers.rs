//! `custom` 策略 handler 注册表（设计 6.1 逃逸出口）。

use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::orphan::{
    orphan_age_days_from_env, select_orphaned_paths, OrphanDeps,
};
use crate::protection::ProtectionCatalog;
use crate::rules::schema::Rule;
use crate::rules::strategy::PathEntry;

/// 按 handler id 执行自定义策略筛选。
pub fn select_custom(
    handler: &str,
    entries: &[PathEntry],
    home: &Path,
    rule: &Rule,
    orphan_deps: &dyn OrphanDeps,
) -> Vec<PathBuf> {
    match handler {
        "claude_desktop_bundled_versions" => claude_desktop_bundled_versions(entries, home, rule),
        "codex_stale_runtimes" => codex_stale_runtimes(entries),
        "final_cut_pro_generated_caches" => final_cut_pro_generated_caches(entries),
        "jianyingpro_generated_caches" => jianyingpro_generated_caches(entries),
        "jetbrains_toolbox_old_versions" => jetbrains_toolbox_old_versions(entries, rule),
        "orphaned_app_data" => orphaned_app_data(entries, home, orphan_deps),
        _ => Vec::new(),
    }
}

fn orphaned_app_data(
    entries: &[PathEntry],
    home: &Path,
    orphan_deps: &dyn OrphanDeps,
) -> Vec<PathBuf> {
    match select_orphaned_paths(
        entries,
        home,
        &ProtectionCatalog::embedded(),
        orphan_deps,
        orphan_age_days_from_env(),
        SystemTime::now(),
    ) {
        Ok(paths) => paths,
        Err(_) => Vec::new(),
    }
}

/// JetBrains Toolbox: under each `apps/<product>/ch-*`, keep newest `keep` version
/// dirs (by mtime), drop older ones. Aligns mole `clean_dev_jetbrains_toolbox`.
fn jetbrains_toolbox_old_versions(entries: &[PathEntry], rule: &Rule) -> Vec<PathBuf> {
    let keep = resolve_keep(rule);
    let mut selected = Vec::new();
    for entry in entries {
        collect_toolbox_old_versions(&entry.path, keep, &mut selected);
    }
    selected
}

fn collect_toolbox_old_versions(product_dir: &Path, keep: usize, out: &mut Vec<PathBuf>) {
    let Ok(channels) = fs::read_dir(product_dir) else {
        return;
    };
    for channel in channels.flatten() {
        let channel_path = channel.path();
        let Some(name) = channel_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("ch-") || !channel_path.is_dir() {
            continue;
        }
        collect_channel_old_versions(&channel_path, keep, out);
    }
}

fn collect_channel_old_versions(channel_dir: &Path, keep: usize, out: &mut Vec<PathBuf>) {
    let current_real = resolve_toolbox_current(channel_dir);

    let Ok(entries) = fs::read_dir(channel_dir) else {
        return;
    };

    let mut versions: Vec<(SystemTime, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == "current" || name.starts_with('.') {
            continue;
        }
        if name == "plugins" || name == "plugins-lib" || name == "plugins-libs" {
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        if let Some(ref current) = current_real {
            if &path == current {
                continue;
            }
        }
        if !name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        let mtime = fs::symlink_metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        versions.push((mtime, path));
    }

    versions.sort_by_key(|(mtime, _)| Reverse(*mtime));
    for (_, path) in versions.into_iter().skip(keep) {
        out.push(path);
    }
}

fn resolve_toolbox_current(channel_dir: &Path) -> Option<PathBuf> {
    let current = channel_dir.join("current");
    let meta = fs::symlink_metadata(&current).ok()?;
    if meta.file_type().is_symlink() {
        let target = fs::read_link(&current).ok()?;
        let resolved = if target.is_absolute() {
            target
        } else {
            channel_dir.join(target)
        };
        return Some(resolved);
    }
    if meta.is_dir() {
        return Some(current);
    }
    None
}

fn final_cut_pro_generated_caches(entries: &[PathEntry]) -> Vec<PathBuf> {
    let mut selected = Vec::new();
    for entry in entries {
        let library = &entry.path;
        if !is_fcpbundle_library(library) {
            continue;
        }
        collect_fcp_generated_targets(library, &mut selected);
    }
    selected
}

fn is_fcpbundle_library(library: &Path) -> bool {
    let Some(name) = library.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !name.ends_with(".fcpbundle") {
        return false;
    }
    // Align mole: only libraries under a Movies directory (paths glob enforces ~/Movies).
    let under_movies = library.ancestors().any(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "Movies")
    });
    if !under_movies {
        return false;
    }
    let Ok(meta) = fs::symlink_metadata(library) else {
        return false;
    };
    meta.is_dir() && !meta.file_type().is_symlink()
}

fn collect_fcp_generated_targets(library: &Path, out: &mut Vec<PathBuf>) {
    walk_fcp_library(library, library, out);
}

fn walk_fcp_library(library: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if matches!(
            name.as_ref(),
            "Original Media"
                | "Analysis Files"
                | "Motion Templates"
                | "Final Cut Pro Backups"
                | "CurrentVersion.flexolibrary"
                | "CurrentVersion.plist"
                | "Settings.plist"
        ) {
            continue;
        }
        if is_safe_fcp_generated_target(library, &path) {
            out.push(path);
            continue;
        }
        walk_fcp_library(library, &path, out);
    }
}

fn is_safe_fcp_generated_target(library: &Path, target: &Path) -> bool {
    let Ok(rel) = target.strip_prefix(library) else {
        return false;
    };
    let parts: Vec<_> = rel.components().collect();
    // …/Render Files/High Quality Media
    if parts.len() >= 2 {
        let a = parts[parts.len() - 2].as_os_str();
        let b = parts[parts.len() - 1].as_os_str();
        if a == "Render Files" && b == "High Quality Media" {
            return true;
        }
        if a == "Transcoded Media" && b == "Proxy Media" {
            return true;
        }
    }
    false
}

/// Mole `clean_jianying_pro_generated_caches` regenerable whitelist.
const JIANYINGPRO_REGENERABLE_SUBDIRS: &[&str] = &[
    "recognize",
    "frameThumbnail",
    "audioWave",
    "AlgorithmCache",
    "ILASDKDB",
    "RemuxCache",
    "prerender",
    "segmentPrerenderCache",
    "MotionBlurCache",
    "ttsTemp",
    "tmp",
];

fn jianyingpro_generated_caches(entries: &[PathEntry]) -> Vec<PathBuf> {
    let mut selected = Vec::new();
    for entry in entries {
        let cache_root = &entry.path;
        if !is_jianyingpro_cache_root(cache_root) {
            continue;
        }
        for name in JIANYINGPRO_REGENERABLE_SUBDIRS {
            let sub = cache_root.join(name);
            let Ok(meta) = fs::symlink_metadata(&sub) else {
                continue;
            };
            if meta.is_dir() && !meta.file_type().is_symlink() {
                selected.push(sub);
            }
        }
    }
    selected
}

fn is_jianyingpro_cache_root(cache_root: &Path) -> bool {
    // Expect …/Movies/JianyingPro/User Data/Cache (align mole default root).
    let components: Vec<_> = cache_root
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    let Some(cache_idx) = components.iter().rposition(|c| *c == "Cache") else {
        return false;
    };
    if cache_idx < 3 {
        return false;
    }
    if components[cache_idx - 1] != "User Data"
        || components[cache_idx - 2] != "JianyingPro"
        || components[cache_idx - 3] != "Movies"
    {
        return false;
    }
    if cache_idx + 1 != components.len() {
        return false;
    }
    let Ok(meta) = fs::symlink_metadata(cache_root) else {
        return false;
    };
    meta.is_dir() && !meta.file_type().is_symlink()
}

fn claude_desktop_bundled_versions(
    entries: &[PathEntry],
    home: &Path,
    rule: &Rule,
) -> Vec<PathBuf> {
    let Some(sdk_version) = read_claude_desktop_sdk_version(home) else {
        return Vec::new();
    };
    if entries.len() <= 1 {
        return Vec::new();
    }

    let Some(versions_root) = entries.first().and_then(|e| e.path.parent()) else {
        return Vec::new();
    };
    let active_path = versions_root.join(&sdk_version);
    if !active_path.exists() {
        return Vec::new();
    }

    let keep = resolve_keep(rule);
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| Reverse(e.mtime));

    let mut kept = 0;
    let mut selected = Vec::new();
    for entry in sorted {
        if entry.path == active_path {
            continue;
        }
        if kept < keep {
            kept += 1;
            continue;
        }
        selected.push(entry.path.clone());
    }
    selected
}

fn codex_stale_runtimes(entries: &[PathEntry]) -> Vec<PathBuf> {
    entries
        .iter()
        .filter(|e| is_codex_runtime_stale(&e.path))
        .map(|e| e.path.clone())
        .collect()
}

fn is_codex_runtime_active(runtime_dir: &Path) -> bool {
    if !runtime_dir.is_dir() {
        return false;
    }
    let runtime_json = runtime_dir.join("runtime.json");
    if !runtime_json.is_file() {
        return false;
    }
    runtime_dir.join("dependencies/node").is_dir()
        || runtime_dir.join("dependencies/python").is_dir()
}

fn is_codex_runtime_stale(runtime_dir: &Path) -> bool {
    if !runtime_dir.is_dir() {
        return false;
    }
    if is_codex_runtime_active(runtime_dir) {
        return false;
    }

    let name = runtime_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if matches_codex_stale_name(name) {
        return true;
    }

    !runtime_dir.join("runtime.json").exists() && !runtime_dir.join("dependencies").exists()
}

fn matches_codex_stale_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("tmp")
        || lower.starts_with("temp")
        || lower.ends_with(".tmp")
        || lower.starts_with("incomplete")
        || lower.ends_with(".incomplete")
        || lower.ends_with("-incomplete")
        || lower.starts_with("partial")
        || lower.ends_with(".partial")
}

fn read_claude_desktop_sdk_version(home: &Path) -> Option<String> {
    let sdk_file = home.join("Library/Application Support/Claude/claude-code-vm/.sdk-version");
    let content = fs::read_to_string(&sdk_file).ok()?;
    let sdk_version = content
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    sdk_version_is_safe(&sdk_version).then_some(sdk_version)
}

fn sdk_version_is_safe(sdk_version: &str) -> bool {
    !sdk_version.is_empty()
        && !sdk_version.starts_with('.')
        && !sdk_version.contains('/')
        && !sdk_version.contains("..")
        && sdk_version
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
}

fn resolve_keep(rule: &Rule) -> usize {
    if let Some(var) = &rule.strategy.env_override {
        if let Ok(v) = std::env::var(var) {
            if let Ok(n) = v.parse::<usize>() {
                return n;
            }
        }
    }
    rule.strategy.keep.unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn entry(path: &str, secs: u64) -> PathEntry {
        PathEntry::new(path, SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }

    #[test]
    fn claude_desktop_keeps_active_and_one_previous() {
        let home = tempfile::tempdir().unwrap();
        let support = home.path().join("Library/Application Support/Claude");
        fs::create_dir_all(support.join("claude-code-vm")).unwrap();
        fs::write(support.join("claude-code-vm/.sdk-version"), "2.1.140\n").unwrap();
        fs::create_dir_all(support.join("claude-code/2.1.140")).unwrap();
        fs::create_dir_all(support.join("claude-code/2.1.142")).unwrap();
        fs::create_dir_all(support.join("claude-code/2.1.150")).unwrap();

        let rule = Rule {
            id: "t".into(),
            category: None,
            label: "t".into(),
            platform: vec![],
            paths: vec![],
            impact: None,
            disabled: false,
            last_verified: None,
            strategy: crate::rules::schema::StrategyConfig {
                kind: crate::rules::schema::StrategyKind::Custom,
                keep: Some(1),
                env_override: None,
                days: None,
                names: None,
                handler: Some("claude_desktop_bundled_versions".into()),
            },
            guards: Default::default(),
        };

        let entries = vec![
            entry(&support.join("claude-code/2.1.140").to_string_lossy(), 1),
            entry(&support.join("claude-code/2.1.142").to_string_lossy(), 2),
            entry(&support.join("claude-code/2.1.150").to_string_lossy(), 3),
        ];
        let selected = claude_desktop_bundled_versions(&entries, home.path(), &rule);
        assert_eq!(selected.len(), 1);
        assert!(selected[0].ends_with("2.1.142"));
    }

    #[test]
    fn fcp_generated_selects_only_safe_media_dirs() {
        let home = tempfile::tempdir().unwrap();
        let movies = home.path().join("Movies/Project.fcpbundle");
        let docs = home.path().join("Documents/Other.fcpbundle");
        for p in [
            movies.join("Event/Render Files/High Quality Media"),
            movies.join("Event/Transcoded Media/Proxy Media"),
            movies.join("Event/Transcoded Media/High Quality Media"),
            movies.join("Event/Original Media/Render Files/High Quality Media"),
            movies.join("Event/Analysis Files/Stabilization"),
            docs.join("Event/Render Files/High Quality Media"),
        ] {
            fs::create_dir_all(&p).unwrap();
        }

        let entries = vec![entry(&movies.to_string_lossy(), 1)];
        let selected = final_cut_pro_generated_caches(&entries);
        assert_eq!(selected.len(), 2);
        assert!(selected
            .iter()
            .any(|p| p.ends_with("Render Files/High Quality Media")));
        assert!(selected
            .iter()
            .any(|p| p.ends_with("Transcoded Media/Proxy Media")));
        assert!(!selected
            .iter()
            .any(|p| p.to_string_lossy().contains("Original Media")));
        assert!(!selected
            .iter()
            .any(|p| p.ends_with("Transcoded Media/High Quality Media")));
        // Documents libraries are rejected (must live under Movies/).
        let docs_only = final_cut_pro_generated_caches(&[entry(&docs.to_string_lossy(), 1)]);
        assert!(docs_only.is_empty());
    }

    #[test]
    fn jianyingpro_generated_selects_only_whitelisted_subdirs() {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join("Movies/JianyingPro/User Data/Cache");
        for name in [
            "recognize",
            "frameThumbnail",
            "audioWave",
            "AlgorithmCache",
            "effect",
            "music",
            "image",
            "importcache3",
            "AigcMaterailCache",
            "agencycache",
        ] {
            fs::create_dir_all(cache.join(name)).unwrap();
        }
        fs::create_dir_all(
            home.path()
                .join("Movies/JianyingPro/User Data/Projects/com.lveditor.draft/my-project"),
        )
        .unwrap();

        let selected = jianyingpro_generated_caches(&[entry(&cache.to_string_lossy(), 1)]);
        assert_eq!(selected.len(), 4);
        for name in ["recognize", "frameThumbnail", "audioWave", "AlgorithmCache"] {
            assert!(
                selected.iter().any(|p| p.ends_with(name)),
                "missing regenerable subdir {name}: {selected:?}"
            );
        }
        for name in [
            "effect",
            "music",
            "image",
            "importcache3",
            "AigcMaterailCache",
            "agencycache",
            "Projects",
        ] {
            assert!(
                !selected.iter().any(|p| p.to_string_lossy().contains(name)),
                "must not select protected path containing {name}: {selected:?}"
            );
        }

        // Non-Movies / non-Cache roots are rejected.
        let elsewhere = home.path().join("Documents/JianyingPro/User Data/Cache");
        fs::create_dir_all(elsewhere.join("recognize")).unwrap();
        let rejected = jianyingpro_generated_caches(&[entry(&elsewhere.to_string_lossy(), 1)]);
        assert!(rejected.is_empty());
    }

    #[test]
    fn codex_stale_detects_incomplete_runtime() {
        let tmp = tempfile::tempdir().unwrap();
        let active = tmp.path().join("codex-primary-runtime");
        let stale = tmp.path().join("incomplete-old");
        fs::create_dir_all(active.join("dependencies/python/bin")).unwrap();
        fs::write(active.join("runtime.json"), "{}").unwrap();
        fs::create_dir_all(&stale).unwrap();

        let entries = vec![
            entry(&active.to_string_lossy(), 1),
            entry(&stale.to_string_lossy(), 1),
        ];
        let selected = codex_stale_runtimes(&entries);
        assert_eq!(selected, vec![stale]);
    }

    #[test]
    fn jetbrains_toolbox_keeps_newest_and_skips_current() {
        let tmp = tempfile::tempdir().unwrap();
        let product = tmp.path().join("IntelliJIdea");
        let channel = product.join("ch-0");
        let v_old = channel.join("2023.1");
        let v_mid = channel.join("2024.1");
        let v_new = channel.join("2025.1");
        fs::create_dir_all(&v_old).unwrap();
        fs::create_dir_all(&v_mid).unwrap();
        fs::create_dir_all(&v_new).unwrap();
        std::os::unix::fs::symlink("2025.1", channel.join("current")).unwrap();

        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(200);
        let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(300);
        filetime::set_file_mtime(&v_old, filetime::FileTime::from_system_time(t0)).unwrap();
        filetime::set_file_mtime(&v_mid, filetime::FileTime::from_system_time(t1)).unwrap();
        filetime::set_file_mtime(&v_new, filetime::FileTime::from_system_time(t2)).unwrap();

        let rule = Rule {
            id: "jb".into(),
            category: None,
            label: "jb".into(),
            platform: vec![],
            paths: vec![],
            impact: None,
            disabled: false,
            last_verified: None,
            strategy: crate::rules::schema::StrategyConfig {
                kind: crate::rules::schema::StrategyKind::Custom,
                keep: Some(1),
                env_override: Some("MOLE_JETBRAINS_TOOLBOX_KEEP".into()),
                days: None,
                names: None,
                handler: Some("jetbrains_toolbox_old_versions".into()),
            },
            guards: Default::default(),
        };

        let selected =
            jetbrains_toolbox_old_versions(&[entry(&product.to_string_lossy(), 1)], &rule);
        assert_eq!(
            selected.len(),
            1,
            "keep=1 among non-current → only oldest: {selected:?}"
        );
        assert!(selected[0].ends_with("2023.1"));
        assert!(
            !selected
                .iter()
                .any(|p| p.ends_with("2024.1") || p.ends_with("2025.1")),
            "must keep newest non-current and current: {selected:?}"
        );
    }
}
