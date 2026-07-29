use serde::{Deserialize, Serialize};

use crate::SkipReason;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Report {
    pub succeeded: u64,
    pub skipped: u64,
    pub failed: u64,
    pub skipped_by_reason: Vec<SkipSummary>,
    pub trashed_bytes: u64,
    pub deleted_bytes: u64,
    /// plan 阶段 `done` 可选：规则覆盖说明。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipSummary {
    pub reason: SkipReason,
    pub count: u64,
    pub rule_ids: Vec<String>,
}
