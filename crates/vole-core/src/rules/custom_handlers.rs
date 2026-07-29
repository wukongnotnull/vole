//! `custom` 策略 handler 注册表（设计 6.1 逃逸出口）。

use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};

use crate::rules::schema::Rule;
use crate::rules::strategy::PathEntry;

/// 按 handler id 执行自定义策略筛选。
pub fn select_custom(
    handler: &str,
    entries: &[PathEntry],
    home: &Path,
    rule: &Rule,
) -> Vec<PathBuf> {
    match handler {
        "claude_desktop_bundled_versions" => claude_desktop_bundled_versions(entries, home, rule),
        "codex_stale_runtimes" => codex_stale_runtimes(entries),
        _ => Vec::new(),
    }
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
}
