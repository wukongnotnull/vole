//! 从 TOML 字符串或文件加载规则。

use std::fs;
use std::path::{Path, PathBuf};

use crate::rules::schema::{Rule, RulesFile};

/// 解析 TOML 字符串为规则列表。
pub fn load_rules_from_str(toml: &str) -> Result<Vec<Rule>, LoadError> {
    let file: RulesFile = toml::from_str(toml)?;
    Ok(file.rule)
}

/// 从文件路径读取并解析规则。
pub fn load_rules_from_file(path: impl AsRef<Path>) -> Result<Vec<Rule>, LoadError> {
    let content = fs::read_to_string(path.as_ref())?;
    load_rules_from_str(&content)
}

/// Candidates relative to the directory that contains the `vole` binary.
///
/// Homebrew / release layout installs rules at `../share/vole/rules` from `bin/`.
pub(crate) fn rules_dir_candidates_from_exe_parent(parent: &Path) -> [PathBuf; 2] {
    [
        parent.join("data/rules"),
        parent.join("../share/vole/rules"),
    ]
}

/// 默认规则数据目录：开发构建用 `data/rules`，安装布局或 `VOLE_RULES_DIR` 可覆盖。
pub fn default_rules_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("VOLE_RULES_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for candidate in rules_dir_candidates_from_exe_parent(parent) {
                if candidate.is_dir() {
                    return candidate;
                }
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/rules")
}

/// 加载目录下全部 `*.toml` 规则文件（按文件名排序后拼接）。
pub fn load_rules_from_dir(dir: impl AsRef<Path>) -> Result<Vec<Rule>, LoadError> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir.as_ref())?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();

    let mut rules = Vec::new();
    for path in paths {
        rules.extend(load_rules_from_file(path)?);
    }
    Ok(rules)
}

/// 加载错误。
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::schema::{BrokenSymlinkAction, StrategyKind};

    const EXAMPLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/rules/example.toml"
    ));

    #[test]
    fn loads_embedded_example() {
        let rules = load_rules_from_str(EXAMPLE).expect("parse example");
        assert!(rules.len() >= 2);
        let chrome = rules
            .iter()
            .find(|r| r.id == "chrome-cache")
            .expect("chrome-cache rule");
        assert_eq!(chrome.strategy.kind, StrategyKind::All);
        assert!(!chrome.disabled);
    }

    #[test]
    fn loads_strategy_and_guards() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/rules/ai-agents.toml"
        );
        let rules = load_rules_from_file(path).expect("parse ai-agents");
        let claude = rules
            .iter()
            .find(|r| r.id == "claude-code-old-versions")
            .expect("claude rule");
        assert_eq!(claude.strategy.kind, StrategyKind::KeepNewestByMtime);
        assert_eq!(claude.strategy.keep, Some(1));
        assert_eq!(
            claude.guards.on_broken_symlink,
            Some(BrokenSymlinkAction::SkipRule)
        );
        assert_eq!(claude.guards.not_running, vec!["claude"]);
    }

    #[test]
    fn loads_all_rules_from_dir() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/rules");
        let rules = load_rules_from_dir(dir).expect("load dir");
        assert!(rules.iter().any(|r| r.id == "chrome-cache"));
        assert!(rules.iter().any(|r| r.id == "codex-stale-runtimes"));
    }

    #[test]
    fn orphaned_rule_loads_last_among_enabled() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/rules");
        let rules = load_rules_from_dir(dir).expect("load dir");
        let enabled: Vec<_> = rules.iter().filter(|r| !r.disabled).collect();
        let last = enabled.last().expect("rules");
        assert_eq!(last.id, "orphaned-app-data");
        assert_eq!(last.strategy.handler.as_deref(), Some("orphaned_app_data"));
    }

    #[test]
    fn default_rules_dir_finds_share_layout_relative_to_exe() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bin = tmp.path().join("bin");
        let rules = tmp.path().join("share/vole/rules");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&rules).unwrap();
        std::fs::write(rules.join("probe.toml"), "rule = []\n").unwrap();

        let candidates = rules_dir_candidates_from_exe_parent(&bin);
        let share = &candidates[1];
        assert!(
            share.is_dir(),
            "Homebrew Cellar relative layout must resolve: {}",
            share.display()
        );
        assert!(
            share.canonicalize().unwrap().ends_with("share/vole/rules"),
            "canonical path should end with share/vole/rules"
        );
        let loaded = load_rules_from_dir(share).expect("load share rules");
        assert!(loaded.is_empty(), "probe.toml has empty rule list");
    }
}
