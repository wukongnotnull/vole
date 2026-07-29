//! plan 生成用的规则候选类型（Task 9 管线入口）。

use std::path::PathBuf;

use crate::rules::schema::Rule;

/// 单条待清理路径候选。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    /// 展开后的策略路径模式来源（规则 `paths` 中的一条）。
    pub source_pattern: String,
}

impl Candidate {
    pub fn new(path: impl Into<PathBuf>, source_pattern: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            source_pattern: source_pattern.into(),
        }
    }
}

/// 某条规则经 glob + 策略筛选后的候选集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCandidate {
    pub rule_id: String,
    pub label: String,
    pub category: Option<String>,
    pub candidates: Vec<Candidate>,
}

impl RuleCandidate {
    pub fn from_rule(rule: &Rule, candidates: Vec<Candidate>) -> Self {
        Self {
            rule_id: rule.id.clone(),
            label: rule.label.clone(),
            category: rule.category.clone(),
            candidates,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}
