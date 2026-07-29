//! 从 TOML 字符串或文件加载规则。

use std::fs;
use std::path::Path;

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
        let rules = load_rules_from_str(EXAMPLE).expect("parse");
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
}
