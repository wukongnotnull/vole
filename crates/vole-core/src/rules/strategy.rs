//! 封闭策略集与 `Strategy` trait。

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::rules::schema::{StrategyConfig, StrategyKind};

/// glob 展开后的目录项，供策略筛选。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEntry {
    pub path: PathBuf,
    pub mtime: SystemTime,
}

impl PathEntry {
    pub fn new(path: impl Into<PathBuf>, mtime: SystemTime) -> Self {
        Self {
            path: path.into(),
            mtime,
        }
    }

    #[cfg(test)]
    fn with_secs(path: impl Into<PathBuf>, secs: u64) -> Self {
        Self::new(
            path,
            std::time::UNIX_EPOCH + Duration::from_secs(secs),
        )
    }
}

/// 策略从候选条目中选出待删除路径。
pub trait Strategy {
    fn select(&self, entries: &[PathEntry]) -> Vec<PathBuf>;
}

/// 删除全部候选。
#[derive(Debug, Clone, Copy, Default)]
pub struct All;

impl Strategy for All {
    fn select(&self, entries: &[PathEntry]) -> Vec<PathBuf> {
        entries.iter().map(|e| e.path.clone()).collect()
    }
}

/// 按 mtime 保留最新 `keep` 条，其余删除。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeepNewestByMtime {
    pub keep: usize,
}

impl Strategy for KeepNewestByMtime {
    fn select(&self, entries: &[PathEntry]) -> Vec<PathBuf> {
        if entries.len() <= self.keep {
            return Vec::new();
        }
        let mut sorted: Vec<&PathEntry> = entries.iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.mtime));
        sorted
            .into_iter()
            .skip(self.keep)
            .map(|e| e.path.clone())
            .collect()
    }
}

/// 按版本字符串保留最新 `keep` 条（初版：路径末段字符串降序，保留前 `keep`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeepNewestByVersion {
    pub keep: usize,
}

impl Strategy for KeepNewestByVersion {
    fn select(&self, entries: &[PathEntry]) -> Vec<PathBuf> {
        // TODO(Task 9+): 解析语义化版本号；当前按路径末段字符串降序近似。
        if entries.len() <= self.keep {
            return Vec::new();
        }
        let mut sorted: Vec<&PathEntry> = entries.iter().collect();
        sorted.sort_by(|a, b| {
            let a_name = entry_name(a);
            let b_name = entry_name(b);
            b_name.cmp(a_name)
        });
        sorted
            .into_iter()
            .skip(self.keep)
            .map(|e| e.path.clone())
            .collect()
    }
}

/// 删除早于 `days` 天的条目。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlderThanDays {
    pub days: u32,
    pub now: SystemTime,
}

impl OlderThanDays {
    pub fn new(days: u32) -> Self {
        Self {
            days,
            now: SystemTime::now(),
        }
    }

    #[cfg(test)]
    fn with_now(days: u32, now: SystemTime) -> Self {
        Self { days, now }
    }

    fn cutoff(&self) -> Option<SystemTime> {
        self.now
            .checked_sub(Duration::from_secs(u64::from(self.days) * 86_400))
    }
}

impl Strategy for OlderThanDays {
    fn select(&self, entries: &[PathEntry]) -> Vec<PathBuf> {
        let Some(cutoff) = self.cutoff() else {
            return Vec::new();
        };
        entries
            .iter()
            .filter(|e| e.mtime < cutoff)
            .map(|e| e.path.clone())
            .collect()
    }
}

/// 保留 `names` 中的条目名，删除其余。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepNamed {
    pub names: Vec<String>,
}

impl KeepNamed {
    pub fn new(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            names: names.into_iter().map(Into::into).collect(),
        }
    }
}

impl Strategy for KeepNamed {
    fn select(&self, entries: &[PathEntry]) -> Vec<PathBuf> {
        entries
            .iter()
            .filter(|e| {
                let name = entry_name(e);
                !self.names.iter().any(|n| n == name)
            })
            .map(|e| e.path.clone())
            .collect()
    }
}

/// `custom` 策略占位：解析 handler，执行留待注册表（Task 9+）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Custom {
    pub handler: String,
}

impl Strategy for Custom {
    fn select(&self, _entries: &[PathEntry]) -> Vec<PathBuf> {
        // handler 注册表尚未接线；返回空表示本阶段不产出候选。
        let _ = &self.handler;
        Vec::new()
    }
}

/// 解析后的策略，可动态分发。
#[derive(Debug)]
pub enum ResolvedStrategy {
    All(All),
    KeepNewestByMtime(KeepNewestByMtime),
    KeepNewestByVersion(KeepNewestByVersion),
    OlderThanDays(OlderThanDays),
    KeepNamed(KeepNamed),
    Custom(Custom),
}

impl Strategy for ResolvedStrategy {
    fn select(&self, entries: &[PathEntry]) -> Vec<PathBuf> {
        match self {
            Self::All(s) => s.select(entries),
            Self::KeepNewestByMtime(s) => s.select(entries),
            Self::KeepNewestByVersion(s) => s.select(entries),
            Self::OlderThanDays(s) => s.select(entries),
            Self::KeepNamed(s) => s.select(entries),
            Self::Custom(s) => s.select(entries),
        }
    }
}

/// 从 `StrategyConfig` 构建可执行策略。
pub fn resolve_strategy(config: &StrategyConfig) -> Result<ResolvedStrategy, StrategyBuildError> {
    let keep = resolve_keep(config);

    let strategy = match config.kind {
        StrategyKind::All => ResolvedStrategy::All(All),
        StrategyKind::KeepNewestByMtime => {
            let keep = keep.ok_or(StrategyBuildError::MissingField {
                kind: config.kind,
                field: "keep",
            })?;
            ResolvedStrategy::KeepNewestByMtime(KeepNewestByMtime { keep })
        }
        StrategyKind::KeepNewestByVersion => {
            let keep = keep.ok_or(StrategyBuildError::MissingField {
                kind: config.kind,
                field: "keep",
            })?;
            ResolvedStrategy::KeepNewestByVersion(KeepNewestByVersion { keep })
        }
        StrategyKind::OlderThanDays => {
            let days = config.days.ok_or(StrategyBuildError::MissingField {
                kind: config.kind,
                field: "days",
            })?;
            ResolvedStrategy::OlderThanDays(OlderThanDays::new(days))
        }
        StrategyKind::KeepNamed => {
            let names = config
                .names
                .clone()
                .ok_or(StrategyBuildError::MissingField {
                    kind: config.kind,
                    field: "names",
                })?;
            if names.is_empty() {
                return Err(StrategyBuildError::EmptyNames);
            }
            ResolvedStrategy::KeepNamed(KeepNamed { names })
        }
        StrategyKind::Custom => {
            let handler = config
                .handler
                .clone()
                .ok_or(StrategyBuildError::MissingField {
                    kind: config.kind,
                    field: "handler",
                })?;
            ResolvedStrategy::Custom(Custom { handler })
        }
    };
    Ok(strategy)
}

fn resolve_keep(config: &StrategyConfig) -> Option<usize> {
    resolve_keep_with(config, |name| std::env::var(name).ok())
}

fn resolve_keep_with(
    config: &StrategyConfig,
    env_get: impl Fn(&str) -> Option<String>,
) -> Option<usize> {
    if let Some(var) = &config.env_override {
        if let Some(v) = env_get(var) {
            if let Ok(n) = v.parse::<usize>() {
                return Some(n);
            }
        }
    }
    config.keep
}

fn entry_name(entry: &PathEntry) -> &str {
    entry
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
}

/// 构建策略时的错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StrategyBuildError {
    #[error("strategy {kind:?} requires field `{field}`")]
    MissingField {
        kind: StrategyKind,
        field: &'static str,
    },
    #[error("keep_named strategy requires non-empty names")]
    EmptyNames,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn paths(selected: &[PathEntry]) -> Vec<PathBuf> {
        All.select(selected)
    }

    #[test]
    fn all_selects_every_entry() {
        let entries = vec![
            PathEntry::with_secs("/tmp/a", 1),
            PathEntry::with_secs("/tmp/b", 2),
        ];
        assert_eq!(
            paths(&entries),
            vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")]
        );
    }

    #[test]
    fn keep_newest_by_mtime_keeps_n_newest() {
        let entries = vec![
            PathEntry::with_secs("/v/old", 1),
            PathEntry::with_secs("/v/mid", 2),
            PathEntry::with_secs("/v/new", 3),
        ];
        let strategy = KeepNewestByMtime { keep: 1 };
        let selected = strategy.select(&entries);
        assert_eq!(selected.len(), 2);
        assert!(!selected.contains(&PathBuf::from("/v/new")));
        assert!(selected.contains(&PathBuf::from("/v/old")));
        assert!(selected.contains(&PathBuf::from("/v/mid")));
    }

    #[test]
    fn keep_newest_by_mtime_returns_empty_when_within_keep() {
        let entries = vec![PathEntry::with_secs("/v/only", 1)];
        let strategy = KeepNewestByMtime { keep: 1 };
        assert!(strategy.select(&entries).is_empty());
    }

    #[test]
    fn older_than_days_selects_stale_entries() {
        let now = UNIX_EPOCH + Duration::from_secs(10 * 86_400);
        let entries = vec![
            PathEntry::with_secs("/v/stale", 0),
            PathEntry::with_secs("/v/fresh", 9 * 86_400),
        ];
        let strategy = OlderThanDays::with_now(7, now);
        assert_eq!(strategy.select(&entries), vec![PathBuf::from("/v/stale")]);
    }

    #[test]
    fn keep_named_preserves_listed_names() {
        let entries = vec![
            PathEntry::with_secs("/d/current", 1),
            PathEntry::with_secs("/d/old-build", 2),
        ];
        let strategy = KeepNamed::new(["current"]);
        assert_eq!(
            strategy.select(&entries),
            vec![PathBuf::from("/d/old-build")]
        );
    }

    #[test]
    fn resolve_keep_env_override() {
        let config = StrategyConfig {
            kind: StrategyKind::KeepNewestByMtime,
            keep: Some(1),
            env_override: Some("VOLE_TEST_KEEP".to_string()),
            days: None,
            names: None,
            handler: None,
        };
        let keep = resolve_keep_with(&config, |name| {
            if name == "VOLE_TEST_KEEP" {
                Some("2".to_string())
            } else {
                None
            }
        });
        assert_eq!(keep, Some(2));
    }

    #[test]
    fn keep_newest_by_version_sorts_by_name() {
        let entries = vec![
            PathEntry::with_secs("/v/1.0.0", 1),
            PathEntry::with_secs("/v/2.0.0", 1),
            PathEntry::with_secs("/v/1.5.0", 1),
        ];
        let strategy = KeepNewestByVersion { keep: 1 };
        let selected = strategy.select(&entries);
        assert_eq!(selected.len(), 2);
        assert!(!selected.contains(&PathBuf::from("/v/2.0.0")));
    }
}
