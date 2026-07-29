//! Parse mole-compatible `operations.log` and `deletions.log` lines.

use crate::history::json::{HistoryActions, HistoryDeletion, HistorySession};

/// Load sessions from an operations.log (missing/unreadable → empty).
pub fn load_sessions(path: &std::path::Path) -> Vec<HistorySession> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_operations(&content)
}

/// Load deletion audit entries (missing/unreadable → empty).
pub fn load_deletions(path: &std::path::Path) -> Vec<HistoryDeletion> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_deletions(&content)
}

pub fn parse_operations(content: &str) -> Vec<HistorySession> {
    let mut sessions = Vec::new();
    let mut active: Option<ActiveSession> = None;

    for line in content.lines() {
        if let Some((command, started_at)) = parse_session_start(line) {
            if let Some(prev) = active.take() {
                sessions.push(prev.into_session());
            }
            active = Some(ActiveSession::start(command, started_at));
            continue;
        }
        if let Some(end) = parse_session_end(line) {
            if active.is_none() {
                active = Some(ActiveSession::start(
                    end.command.clone(),
                    end.ended_at.clone(),
                ));
            }
            if let Some(mut sess) = active.take() {
                sess.apply_end(end);
                sessions.push(sess.into_session());
            }
            continue;
        }
        if let Some((command, action, timestamp)) = parse_operation_line(line) {
            if active.is_none() {
                active = Some(ActiveSession::start(command, timestamp));
            }
            if let Some(sess) = active.as_mut() {
                sess.record_operation(&action);
            }
        }
    }

    if let Some(sess) = active.take() {
        sessions.push(sess.into_session());
    }

    sessions
}

pub fn parse_deletions(content: &str) -> Vec<HistoryDeletion> {
    let mut out = Vec::new();
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(5, '\t');
        let Some(timestamp) = parts.next() else {
            continue;
        };
        let Some(mode) = parts.next() else {
            continue;
        };
        let Some(size_kb_raw) = parts.next() else {
            continue;
        };
        let Some(status) = parts.next() else {
            continue;
        };
        let path = parts.next().unwrap_or("");
        if timestamp.is_empty() || mode.is_empty() || status.is_empty() {
            continue;
        }
        let size_kb = if !size_kb_raw.is_empty() && size_kb_raw.chars().all(|c| c.is_ascii_digit())
        {
            size_kb_raw.parse::<u64>().ok()
        } else {
            None
        };
        out.push(HistoryDeletion {
            timestamp: timestamp.to_string(),
            mode: mode.to_string(),
            status: status.to_string(),
            size_kb,
            path: path.to_string(),
        });
    }
    out
}

struct SessionEnd {
    command: String,
    ended_at: String,
    items: Option<u64>,
    size: Option<String>,
}

struct ActiveSession {
    command: String,
    started_at: String,
    ended_at: String,
    items: u64,
    size: String,
    actions: HistoryActions,
    operation_count: u64,
}

impl ActiveSession {
    fn start(command: String, started_at: String) -> Self {
        Self {
            command,
            started_at,
            ended_at: String::new(),
            items: 0,
            size: "0B".to_string(),
            actions: HistoryActions::default(),
            operation_count: 0,
        }
    }

    fn apply_end(&mut self, end: SessionEnd) {
        self.ended_at = end.ended_at;
        if let Some(items) = end.items {
            self.items = items;
        }
        if let Some(size) = end.size {
            self.size = size;
        }
    }

    fn record_operation(&mut self, action: &str) {
        self.operation_count += 1;
        match action {
            "REMOVED" => self.actions.removed += 1,
            "TRASHED" => self.actions.trashed += 1,
            "SKIPPED" => self.actions.skipped += 1,
            "FAILED" => self.actions.failed += 1,
            "REBUILT" => self.actions.rebuilt += 1,
            _ => self.actions.other += 1,
        }
    }

    fn into_session(self) -> HistorySession {
        HistorySession {
            command: self.command,
            started_at: self.started_at,
            ended_at: self.ended_at,
            items: self.items,
            size: self.size,
            operation_count: self.operation_count,
            actions: self.actions,
        }
    }
}

fn parse_session_start(line: &str) -> Option<(String, String)> {
    let prefix = "# ========== ";
    let suffix = " ==========";
    if !line.starts_with(prefix) || !line.ends_with(suffix) {
        return None;
    }
    let inner = &line[prefix.len()..line.len() - suffix.len()];
    let marker = " session started at ";
    let idx = inner.find(marker)?;
    let command = inner[..idx].to_string();
    let started_at = inner[idx + marker.len()..].to_string();
    if command.is_empty() || started_at.is_empty() {
        return None;
    }
    // Reject "session ended" lines mistaken as start
    if inner.contains(" session ended at ") {
        return None;
    }
    Some((command, started_at))
}

fn parse_session_end(line: &str) -> Option<SessionEnd> {
    let prefix = "# ========== ";
    let suffix = " ==========";
    if !line.starts_with(prefix) || !line.ends_with(suffix) {
        return None;
    }
    let inner = &line[prefix.len()..line.len() - suffix.len()];
    let marker = " session ended at ";
    let idx = inner.find(marker)?;
    let command = inner[..idx].to_string();
    let rest = &inner[idx + marker.len()..];

    let (ended_at, items, size) = if let Some(comma) = rest.find(", ") {
        let ended_at = rest[..comma].to_string();
        let tail = &rest[comma + 2..];
        if let Some(items_end) = tail.find(" items, ") {
            let items_str = &tail[..items_end];
            let size = tail[items_end + " items, ".len()..].to_string();
            let items = if items_str.chars().all(|c| c.is_ascii_digit()) && !items_str.is_empty() {
                items_str.parse::<u64>().ok()
            } else {
                None
            };
            (ended_at, items, Some(size))
        } else {
            (ended_at, None, None)
        }
    } else {
        (rest.to_string(), None, None)
    };

    Some(SessionEnd {
        command,
        ended_at,
        items,
        size,
    })
}

fn parse_operation_line(line: &str) -> Option<(String, String, String)> {
    // [timestamp] [command] ACTION ...
    if !line.starts_with('[') {
        return None;
    }
    let after_ts = line.strip_prefix('[')?;
    let ts_end = after_ts.find(']')?;
    let timestamp = after_ts[..ts_end].to_string();
    let rest = after_ts[ts_end + 1..].trim_start();
    let rest = rest.strip_prefix('[')?;
    let cmd_end = rest.find(']')?;
    let command = rest[..cmd_end].to_string();
    let after_cmd = rest[cmd_end + 1..].trim_start();
    let action = after_cmd.split_whitespace().next()?.to_string();
    if timestamp.is_empty() || command.is_empty() || action.is_empty() {
        return None;
    }
    Some((command, action, timestamp))
}

#[cfg(test)]
mod parse_unit_tests {
    use super::*;

    #[test]
    fn parse_start_and_end_markers() {
        let (cmd, started) = parse_session_start(
            "# ========== clean session started at 2026-05-24 10:00:00 ==========",
        )
        .unwrap();
        assert_eq!(cmd, "clean");
        assert_eq!(started, "2026-05-24 10:00:00");

        let end = parse_session_end(
            "# ========== clean session ended at 2026-05-24 10:00:05, 2 items, 6KB ==========",
        )
        .unwrap();
        assert_eq!(end.command, "clean");
        assert_eq!(end.ended_at, "2026-05-24 10:00:05");
        assert_eq!(end.items, Some(2));
        assert_eq!(end.size.as_deref(), Some("6KB"));
    }
}
