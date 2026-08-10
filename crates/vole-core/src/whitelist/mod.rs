//! Mole 兼容的 clean / optimize whitelist 配置与管理菜单纯逻辑。

mod catalog;

pub use catalog::{CleanWhitelistItem, CLEAN_WHITELIST_CATALOG, DEFAULT_CLEAN_WHITELIST_PATTERNS};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::optimize::optimize_catalog;

const HEADER: &str = "# Mole Whitelist - Protected paths won't be deleted\n# Default protections: Playwright browsers, HuggingFace models, Maven repo, Ollama models, Surge Mac, R renv, Finder metadata\n# Add one pattern per line to keep items safe.";

const OPTIMIZE_HEADER: &str =
    "# Mole Optimize Whitelist - Listed tasks are skipped\n# One task id per line\n";

const FINDER_METADATA_SENTINEL: &str = "FINDER_METADATA";

fn config_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".config/mole/whitelist"))
        .unwrap_or_else(|| PathBuf::from(".config/mole/whitelist"))
}

fn home_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Expand `$HOME` / `~` for exact string compare (mole `patterns_equivalent`).
pub fn expand_pattern(pattern: &str, home: &Path) -> String {
    let home_s = home.to_string_lossy();
    let pat = pattern.trim();
    if pat == FINDER_METADATA_SENTINEL {
        return FINDER_METADATA_SENTINEL.to_string();
    }
    let after_dollar = if let Some(rest) = pat.strip_prefix("$HOME") {
        format!("{home_s}{rest}")
    } else {
        pat.to_string()
    };
    if after_dollar == "~" {
        return home_s.into_owned();
    }
    if let Some(rest) = after_dollar.strip_prefix("~/") {
        return format!("{home_s}/{rest}");
    }
    after_dollar
}

pub fn patterns_equivalent(a: &str, b: &str, home: &Path) -> bool {
    expand_pattern(a, home) == expand_pattern(b, home)
}

/// Portable form written to config (`~/…` or sentinel).
pub fn to_portable_pattern(pattern: &str, home: &Path) -> String {
    let expanded = expand_pattern(pattern, home);
    if expanded == FINDER_METADATA_SENTINEL {
        return FINDER_METADATA_SENTINEL.to_string();
    }
    let home_s = home.to_string_lossy();
    if expanded == home_s.as_ref() {
        return "~".to_string();
    }
    let prefix = format!("{home_s}/");
    if let Some(rest) = expanded.strip_prefix(&prefix) {
        return format!("~/{rest}");
    }
    expanded
}

pub fn clean_config_exists() -> bool {
    config_path().exists()
}

pub fn clean_config_display_path() -> String {
    let path = config_path();
    let home = home_path();
    to_portable_pattern(&path.to_string_lossy(), &home)
}

/// Defaults used only when the manage session has no config file (mole load_whitelist).
pub fn default_clean_patterns(home: &Path) -> Vec<String> {
    DEFAULT_CLEAN_WHITELIST_PATTERNS
        .iter()
        .map(|p| to_portable_pattern(p, home))
        .collect()
}

/// Patterns for the interactive manager: file contents, or defaults if missing.
pub fn load_clean_for_manage() -> io::Result<Vec<String>> {
    if !clean_config_exists() {
        return Ok(default_clean_patterns(&home_path()));
    }
    load_clean()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitelistMenuEntry {
    pub label: String,
    /// Portable pattern (`~/…` or sentinel) aligned with save format.
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitelistMenuBuild {
    pub entries: Vec<WhitelistMenuEntry>,
    pub preselected: Vec<usize>,
    pub custom_patterns: Vec<String>,
}

/// Build mole-style whitelist menu: selected first, then remaining; preselect selected.
pub fn build_clean_whitelist_menu(current: &[String], home: &Path) -> WhitelistMenuBuild {
    let mut selected = Vec::new();
    let mut remaining = Vec::new();

    for item in CLEAN_WHITELIST_CATALOG {
        let portable = to_portable_pattern(item.pattern, home);
        let entry = WhitelistMenuEntry {
            label: item.display_name.to_string(),
            pattern: portable.clone(),
        };
        if current
            .iter()
            .any(|c| patterns_equivalent(c, &portable, home))
        {
            selected.push(entry);
        } else {
            remaining.push(entry);
        }
    }

    let mut custom_patterns = Vec::new();
    for cur in current {
        let is_predefined = CLEAN_WHITELIST_CATALOG
            .iter()
            .any(|item| patterns_equivalent(cur, &to_portable_pattern(item.pattern, home), home));
        if !is_predefined
            && !custom_patterns
                .iter()
                .any(|c: &String| patterns_equivalent(c, cur, home))
        {
            custom_patterns.push(to_portable_pattern(cur, home));
        }
    }

    let preselected: Vec<usize> = (0..selected.len()).collect();
    let mut entries = selected;
    entries.extend(remaining);

    WhitelistMenuBuild {
        entries,
        preselected,
        custom_patterns,
    }
}

/// Merge confirmed catalog indices with custom patterns (mole save path).
pub fn merge_whitelist_selection(
    entries: &[WhitelistMenuEntry],
    selected_indices: &[usize],
    custom_patterns: &[String],
) -> Vec<String> {
    let mut out = Vec::new();
    for &idx in selected_indices {
        if let Some(entry) = entries.get(idx) {
            if !out.iter().any(|p| p == &entry.pattern) {
                out.push(entry.pattern.clone());
            }
        }
    }
    for custom in custom_patterns {
        if !out.iter().any(|p| p == custom) {
            out.push(custom.clone());
        }
    }
    out
}

pub fn load_clean() -> io::Result<Vec<String>> {
    let path = config_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = fs::read_to_string(&path)?;
    Ok(parse_lines(&text))
}

pub fn add_clean(pattern: &str) -> io::Result<()> {
    let pat = pattern.trim();
    if pat.is_empty() || pat.starts_with('#') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "无效的白名单 pattern",
        ));
    }
    let mut patterns = load_clean()?;
    if patterns.iter().any(|p| p == pat) {
        return Ok(());
    }
    patterns.push(pat.to_string());
    save_clean(&patterns)
}

pub fn remove_clean(pattern: &str) -> io::Result<bool> {
    let pat = pattern.trim();
    let mut patterns = load_clean()?;
    let before = patterns.len();
    patterns.retain(|p| p != pat);
    if patterns.len() == before {
        return Ok(false);
    }
    save_clean(&patterns)?;
    Ok(true)
}

pub fn save_clean(patterns: &[String]) -> io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut unique: Vec<String> = Vec::new();
    for p in patterns {
        if !unique.iter().any(|u| u == p) {
            unique.push(p.clone());
        }
    }
    let mut out = String::from(HEADER);
    out.push('\n');
    if !unique.is_empty() {
        out.push('\n');
    }
    for p in &unique {
        out.push_str(p);
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

fn parse_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// 简化 glob：精确相等，或 pattern 为 `prefix*` 前缀匹配。
pub fn is_match(path: &Path, patterns: &[String]) -> bool {
    let s = path.to_string_lossy();
    for pat in patterns {
        if pat.ends_with('*') {
            let prefix = &pat[..pat.len() - 1];
            if s.starts_with(prefix) {
                return true;
            }
        } else if s == pat.as_str() {
            return true;
        }
    }
    false
}

fn optimize_config_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".config/mole/whitelist_optimize"))
        .unwrap_or_else(|| PathBuf::from(".config/mole/whitelist_optimize"))
}

pub fn optimize_config_display_path() -> String {
    let path = optimize_config_path();
    let home = home_path();
    to_portable_pattern(&path.to_string_lossy(), &home)
}

pub fn load_optimize() -> io::Result<Vec<String>> {
    let path = optimize_config_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = fs::read_to_string(&path)?;
    Ok(parse_lines(&text))
}

pub fn save_optimize(ids: &[String]) -> io::Result<()> {
    let path = optimize_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut unique: Vec<String> = Vec::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty() || id.starts_with('#') {
            continue;
        }
        if !unique.iter().any(|u| u == id) {
            unique.push(id.to_string());
        }
    }
    let mut out = String::from(OPTIMIZE_HEADER);
    if !unique.is_empty() {
        out.push('\n');
    }
    for id in &unique {
        out.push_str(id);
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

fn known_optimize_task(id: &str) -> bool {
    optimize_catalog().iter().any(|t| t.id == id)
}

pub fn add_optimize(task_id: &str) -> io::Result<()> {
    let id = task_id.trim();
    if id.is_empty() || id.starts_with('#') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "无效的 optimize 任务 id",
        ));
    }
    if !known_optimize_task(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("未知 optimize 任务 id: {id}"),
        ));
    }
    let mut ids = load_optimize()?;
    if ids.iter().any(|p| p == id) {
        return Ok(());
    }
    ids.push(id.to_string());
    save_optimize(&ids)
}

pub fn remove_optimize(task_id: &str) -> io::Result<bool> {
    let id = task_id.trim();
    let mut ids = load_optimize()?;
    let before = ids.len();
    ids.retain(|p| p != id);
    if ids.len() == before {
        return Ok(false);
    }
    save_optimize(&ids)?;
    Ok(true)
}

pub fn is_task_whitelisted(task_id: &str, ids: &[String]) -> bool {
    ids.iter().any(|id| id == task_id)
}

/// Build mole-style optimize whitelist menu: selected first, then remaining.
pub fn build_optimize_whitelist_menu(current: &[String]) -> WhitelistMenuBuild {
    let mut selected = Vec::new();
    let mut remaining = Vec::new();

    for task in optimize_catalog() {
        let entry = WhitelistMenuEntry {
            label: task.title.to_string(),
            pattern: task.id.to_string(),
        };
        if current.iter().any(|c| c == task.id) {
            selected.push(entry);
        } else {
            remaining.push(entry);
        }
    }

    let mut custom_patterns = Vec::new();
    for cur in current {
        let known = optimize_catalog().iter().any(|t| t.id == cur.as_str());
        if !known && !custom_patterns.iter().any(|c: &String| c == cur) {
            custom_patterns.push(cur.clone());
        }
    }

    let preselected: Vec<usize> = (0..selected.len()).collect();
    let mut entries = selected;
    entries.extend(remaining);

    WhitelistMenuBuild {
        entries,
        preselected,
        custom_patterns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;
    use std::io;
    use std::path::Path;

    #[test]
    fn roundtrip_patterns() {
        let _guard = test_env::lock();
        let home = std::env::temp_dir().join(format!("vole-wl-{}", std::process::id()));
        std::env::set_var("HOME", home.join("h"));
        save_clean(&["/tmp/a*".into(), "/tmp/b".into()]).unwrap();
        let loaded = load_clean().unwrap();
        assert!(loaded.contains(&"/tmp/a*".to_string()));
        assert!(loaded.contains(&"/tmp/b".to_string()));
        std::env::remove_var("HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn prefix_star_matches() {
        assert!(is_match(Path::new("/tmp/abc"), &["/tmp/a*".into()]));
        assert!(!is_match(Path::new("/other"), &["/tmp/a*".into()]));
    }

    #[test]
    fn add_remove_roundtrip() {
        let _guard = test_env::lock();
        let home = std::env::temp_dir().join(format!("vole-wl-ar-{}", std::process::id()));
        std::env::set_var("HOME", home.join("h"));
        add_clean("/tmp/keep*").unwrap();
        add_clean("/tmp/other").unwrap();
        assert!(!remove_clean("/tmp/missing").unwrap());
        let loaded = load_clean().unwrap();
        assert!(loaded.contains(&"/tmp/keep*".to_string()));
        assert!(loaded.contains(&"/tmp/other".to_string()));
        assert!(remove_clean("/tmp/keep*").unwrap());
        let loaded = load_clean().unwrap();
        assert!(!loaded.contains(&"/tmp/keep*".to_string()));
        assert!(loaded.contains(&"/tmp/other".to_string()));
        std::env::remove_var("HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn add_rejects_empty_pattern() {
        let err = add_clean("  ").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn patterns_equivalent_tilde_and_home() {
        let home = Path::new("/Users/demo");
        assert!(patterns_equivalent(
            "~/Library/Caches/foo",
            "$HOME/Library/Caches/foo",
            home
        ));
        assert!(patterns_equivalent(
            "/Users/demo/Library/Caches/foo",
            "~/Library/Caches/foo",
            home
        ));
        assert!(!patterns_equivalent(
            "~/Library/Caches/foo",
            "~/Library/Caches/bar",
            home
        ));
        assert!(patterns_equivalent(
            FINDER_METADATA_SENTINEL,
            "FINDER_METADATA",
            home
        ));
    }

    #[test]
    fn manage_defaults_when_config_missing() {
        let _guard = test_env::lock();
        let root = std::env::temp_dir().join(format!("vole-wl-def-{}", std::process::id()));
        let home = root.join("h");
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        assert!(!clean_config_exists());
        let loaded = load_clean_for_manage().unwrap();
        assert!(!loaded.is_empty());
        assert!(loaded.iter().any(|p| p.contains("ms-playwright")));
        assert!(loaded.iter().any(|p| p == FINDER_METADATA_SENTINEL));
        // scan path still empty when file missing
        assert!(load_clean().unwrap().is_empty());
        std::env::remove_var("HOME");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn menu_puts_selected_first_and_keeps_custom() {
        let home = Path::new("/Users/demo");
        let npm = to_portable_pattern("$HOME/.npm/_cacache/*", home);
        let custom = "~/my/custom/cache/*".to_string();
        let build = build_clean_whitelist_menu(&[npm.clone(), custom.clone()], home);
        assert_eq!(build.preselected, vec![0]);
        assert_eq!(build.entries[0].pattern, npm);
        assert!(build.custom_patterns.iter().any(|p| p == &custom));
        assert!(build.entries.iter().skip(1).all(|e| e.pattern != npm));
    }

    #[test]
    fn merge_keeps_custom_when_predefined_cleared() {
        let home = Path::new("/Users/demo");
        let build = build_clean_whitelist_menu(
            &[
                to_portable_pattern("$HOME/.npm/_cacache/*", home),
                "~/keep-me/*".into(),
            ],
            home,
        );
        let merged = merge_whitelist_selection(&build.entries, &[], &build.custom_patterns);
        assert_eq!(merged, vec!["~/keep-me/*".to_string()]);
    }

    #[test]
    fn catalog_is_non_empty() {
        assert!(CLEAN_WHITELIST_CATALOG.len() >= 70);
    }

    #[test]
    fn optimize_whitelist_roundtrip_and_menu() {
        let _guard = test_env::lock();
        let home = std::env::temp_dir().join(format!("vole-owl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("h")).unwrap();
        std::env::set_var("HOME", home.join("h"));

        assert!(load_optimize().unwrap().is_empty());
        add_optimize("dock_refresh").unwrap();
        let err = add_optimize("not_a_real_task").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        let loaded = load_optimize().unwrap();
        assert_eq!(loaded, vec!["dock_refresh".to_string()]);
        assert!(is_task_whitelisted("dock_refresh", &loaded));
        assert!(!is_task_whitelisted("cache_refresh", &loaded));

        let menu = build_optimize_whitelist_menu(&loaded);
        assert_eq!(menu.entries[0].pattern, "dock_refresh");
        assert_eq!(menu.preselected, vec![0]);
        assert!(menu.entries.iter().any(|e| e.pattern == "cache_refresh"));

        assert!(remove_optimize("dock_refresh").unwrap());
        assert!(load_optimize().unwrap().is_empty());

        std::env::remove_var("HOME");
        let _ = std::fs::remove_dir_all(&home);
    }
}
