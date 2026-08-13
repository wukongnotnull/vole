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
        let operations_log = operations_log.as_ref().to_path_buf();
        let deletions_log = deletions_log.as_ref().to_path_buf();
        let sessions = crate::history::session::load_sessions(&operations_log);
        let deletions = crate::history::session::load_deletions(&deletions_log);
        Self {
            operations_log,
            deletions_log,
            sessions,
            deletions,
        }
    }

    pub(crate) fn from_parts(
        operations_log: PathBuf,
        deletions_log: PathBuf,
        sessions: Vec<HistorySession>,
        deletions: Vec<HistoryDeletion>,
    ) -> Self {
        Self {
            operations_log,
            deletions_log,
            sessions,
            deletions,
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

    pub fn operations_log(&self) -> &Path {
        &self.operations_log
    }

    pub fn deletions_log(&self) -> &Path {
        &self.deletions_log
    }
}

pub fn render_text(report: &HistoryReport, limit: u32) -> String {
    let json = report.to_json(limit);
    let mut out = String::new();
    out.push_str("\nVole History\n\n");

    out.push_str("Recent sessions\n");
    if json.sessions.is_empty() {
        out.push_str("  No operation history yet.\n");
    } else {
        for s in &json.sessions {
            out.push_str(&format!(
                "  {:<10} {}, {} items, {}\n",
                s.command, s.started_at, s.items, s.size
            ));
            let ended = if s.ended_at.is_empty() {
                "not ended"
            } else {
                s.ended_at.as_str()
            };
            out.push_str(&format!(
                "             {}, ended {}\n",
                join_counts(&s.actions),
                ended
            ));
        }
    }

    out.push_str("\nDeletion audit\n");
    if json.deletions.is_empty() {
        out.push_str("  No deletion audit entries yet.\n");
    } else {
        for d in &json.deletions {
            let size_label = match d.size_kb {
                Some(kb) => format_size_kb(kb),
                None => "unknown".to_string(),
            };
            out.push_str(&format!(
                "  {:<24} {:<9} {:<16} {:>8}  {}\n",
                d.timestamp, d.mode, d.status, size_label, d.path
            ));
        }
    }

    out.push_str("\nLogs\n");
    out.push_str(&format!("  operations: {}\n", json.logs.operations));
    out.push_str(&format!("  deletions:  {}\n\n", json.logs.deletions));
    out
}

fn join_counts(a: &HistoryActions) -> String {
    let mut parts = Vec::new();
    if a.removed > 0 {
        parts.push(format!("removed {}", a.removed));
    }
    if a.trashed > 0 {
        parts.push(format!("trashed {}", a.trashed));
    }
    if a.skipped > 0 {
        parts.push(format!("skipped {}", a.skipped));
    }
    if a.failed > 0 {
        parts.push(format!("failed {}", a.failed));
    }
    if a.rebuilt > 0 {
        parts.push(format!("rebuilt {}", a.rebuilt));
    }
    if a.other > 0 {
        parts.push(format!("other {}", a.other));
    }
    if parts.is_empty() {
        "no file actions".to_string()
    } else {
        parts.join(", ")
    }
}

fn format_size_kb(size_kb: u64) -> String {
    // Align with mole bytes_to_human_kb / session size style (base-10 from KB).
    let bytes = size_kb.saturating_mul(1024);
    if bytes >= 1_000_000_000 {
        let scaled = (bytes * 100 + 500_000_000) / 1_000_000_000;
        format!("{}.{:02}GB", scaled / 100, scaled % 100)
    } else if bytes >= 1_000_000 {
        let scaled = (bytes * 10 + 500_000) / 1_000_000;
        format!("{}.{:01}MB", scaled / 10, scaled % 10)
    } else if bytes >= 1000 {
        format!("{}KB", (bytes + 500) / 1000)
    } else if bytes > 0 {
        format!("{bytes}B")
    } else {
        "0B".to_string()
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
