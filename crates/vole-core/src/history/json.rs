//! History JSON shapes aligned with mole `history_render_json`.

use serde::Serialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_LIMIT: u32 = 20;
pub const MAX_LIMIT: u32 = 200;

#[derive(Debug, Clone, Serialize)]
pub struct HistoryLogs {
    pub operations: String,
    pub deletions: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct HistoryActions {
    pub removed: u64,
    pub trashed: u64,
    pub skipped: u64,
    pub failed: u64,
    pub rebuilt: u64,
    pub other: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistorySession {
    pub command: String,
    pub started_at: String,
    pub ended_at: String,
    pub items: u64,
    pub size: String,
    pub operation_count: u64,
    pub actions: HistoryActions,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryDeletion {
    pub timestamp: String,
    pub mode: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_kb: Option<u64>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryJson {
    pub logs: HistoryLogs,
    pub limit: u32,
    pub sessions: Vec<HistorySession>,
    pub deletions: Vec<HistoryDeletion>,
}

/// Loaded history (sessions newest-last in storage; `to_json` emits newest-first).
#[derive(Debug, Clone)]
pub struct HistoryReport {
    operations_log: PathBuf,
    deletions_log: PathBuf,
    sessions: Vec<HistorySession>,
    deletions: Vec<HistoryDeletion>,
}

impl HistoryReport {
    /// Load history from log paths. Missing files are treated as empty.
    pub fn load(operations_log: impl AsRef<Path>, deletions_log: impl AsRef<Path>) -> Self {
        Self {
            operations_log: operations_log.as_ref().to_path_buf(),
            deletions_log: deletions_log.as_ref().to_path_buf(),
            sessions: Vec::new(),
            deletions: Vec::new(),
        }
    }

    pub fn to_json(&self, limit: u32) -> HistoryJson {
        let limit = normalize_limit(limit);
        HistoryJson {
            logs: HistoryLogs {
                operations: self.operations_log.display().to_string(),
                deletions: self.deletions_log.display().to_string(),
            },
            limit,
            sessions: take_newest(self.sessions.as_slice(), limit as usize),
            deletions: take_newest(self.deletions.as_slice(), limit as usize),
        }
    }
}

pub fn normalize_limit(value: u32) -> u32 {
    if value == 0 {
        DEFAULT_LIMIT
    } else {
        value.min(MAX_LIMIT)
    }
}

fn take_newest<T: Clone>(items: &[T], limit: usize) -> Vec<T> {
    if items.is_empty() || limit == 0 {
        return Vec::new();
    }
    let start = items.len().saturating_sub(limit);
    items[start..].iter().rev().cloned().collect()
}

#[cfg(test)]
mod limit_tests {
    use super::*;

    #[test]
    fn normalize_limit_clamps_zero_and_max() {
        assert_eq!(normalize_limit(0), DEFAULT_LIMIT);
        assert_eq!(normalize_limit(20), 20);
        assert_eq!(normalize_limit(500), MAX_LIMIT);
    }
}
