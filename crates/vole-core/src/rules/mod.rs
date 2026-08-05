//! 声明式清理规则与封闭策略集（设计 6.1）。

mod candidate;
mod custom_handlers;
mod glob;
mod load;
mod process_guard;
mod schema;
mod strategy;

pub use candidate::{Candidate, RuleCandidate};
pub use custom_handlers::{select_custom, CustomDegrade, CustomSelectResult};
pub use glob::{collect_path_candidates, expand_rule_path, GlobError};
pub use load::{
    default_rules_dir, load_rules_from_dir, load_rules_from_file, load_rules_from_str, LoadError,
};
pub use process_guard::{
    should_skip_for_cmdline, should_skip_for_guards, should_skip_for_not_running, FakeProcessProbe,
    PgrepProcessProbe, ProcessProbe, ProcessState,
};
pub use schema::{
    BrokenSymlinkAction, GuardsConfig, Rule, RulesFile, StrategyConfig, StrategyKind,
};
pub use strategy::{
    resolve_strategy, All, Custom, KeepNamed, KeepNewestByMtime, KeepNewestByVersion,
    OlderThanDays, PathEntry, ResolvedStrategy, Strategy, StrategyBuildError,
};

/// 仅在路径开头展开 `~` 为 `$HOME`（设计 6.2）。
///
/// 不支持 `~user`；其余 glob 语义在 Task 8 落实。
pub fn expand_home(path: &str) -> String {
    expand_home_with(
        path,
        std::env::var_os("HOME").and_then(|h| h.into_string().ok()),
    )
}

fn expand_home_with(path: &str, home: Option<String>) -> String {
    if let Some(home) = home {
        if let Some(rest) = path.strip_prefix("~/") {
            return format!("{home}/{rest}");
        }
        if path == "~" {
            return home;
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_home_prefix_only() {
        assert_eq!(
            expand_home_with("~/Library/Caches", Some("/Users/test".to_string())),
            "/Users/test/Library/Caches"
        );
        assert_eq!(
            expand_home_with("/tmp/~not-expanded", Some("/Users/test".to_string())),
            "/tmp/~not-expanded"
        );
        assert_eq!(
            expand_home_with("~", Some("/Users/test".to_string())),
            "/Users/test"
        );
        assert_eq!(expand_home_with("~/x", None), "~/x");
    }
}
