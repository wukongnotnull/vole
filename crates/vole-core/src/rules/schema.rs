//! 规则 TOML schema（设计 6.1）。

use serde::Deserialize;

/// 顶层 TOML 文件：`[[rule]]` 数组。
#[derive(Debug, Deserialize)]
pub struct RulesFile {
    pub rule: Vec<Rule>,
}

/// 单条清理规则。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    #[serde(default)]
    pub category: Option<String>,
    pub label: String,
    #[serde(default)]
    pub platform: Vec<String>,
    pub paths: Vec<String>,
    #[serde(default)]
    pub impact: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub last_verified: Option<String>,
    #[serde(default)]
    pub strategy: StrategyConfig,
    #[serde(default)]
    pub guards: GuardsConfig,
}

/// 封闭策略集 `kind` 枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrategyKind {
    #[default]
    All,
    KeepNewestByMtime,
    KeepNewestByVersion,
    OlderThanDays,
    KeepNamed,
    Custom,
}

/// `[rule.strategy]` 段。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StrategyConfig {
    #[serde(default)]
    pub kind: StrategyKind,
    /// `keep_newest_by_mtime` / `keep_newest_by_version`
    #[serde(default)]
    pub keep: Option<usize>,
    /// `keep_newest_by_mtime` 可选环境变量覆盖 `keep`
    #[serde(default)]
    pub env_override: Option<String>,
    /// `older_than_days`
    #[serde(default)]
    pub days: Option<u32>,
    /// `keep_named`
    #[serde(default)]
    pub names: Option<Vec<String>>,
    /// `custom` 逃逸出口 handler id
    #[serde(default)]
    pub handler: Option<String>,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            kind: StrategyKind::All,
            keep: None,
            env_override: None,
            days: None,
            names: None,
            handler: None,
        }
    }
}

/// 断裂 symlink 时的行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokenSymlinkAction {
    SkipRule,
}

/// `[rule.guards]` 段。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct GuardsConfig {
    /// Exact process names (`pgrep -x`).
    #[serde(default)]
    pub not_running: Vec<String>,
    /// Command-line substrings (`pgrep -f`).
    #[serde(default)]
    pub not_running_cmdline: Vec<String>,
    #[serde(default)]
    pub protect_symlink_target: Option<String>,
    #[serde(default)]
    pub on_broken_symlink: Option<BrokenSymlinkAction>,
    #[serde(default)]
    pub requires_app_absent: Option<String>,
    #[serde(default)]
    pub min_free_space: Option<String>,
}
