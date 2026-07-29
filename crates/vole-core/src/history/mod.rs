//! Mole-compatible history load/render (`operations.log` + `deletions.log`).

mod json;
mod parse;
mod session;

pub use json::{
    HistoryActions, HistoryDeletion, HistoryJson, HistoryLogs, HistoryReport, HistorySession,
    DEFAULT_LIMIT, MAX_LIMIT, normalize_limit,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn history_json_empty_logs_contract() {
        let ops = PathBuf::from("/tmp/vole-history-missing/operations.log");
        let dels = PathBuf::from("/tmp/vole-history-missing/deletions.log");
        let report = HistoryReport::load(&ops, &dels);
        let json = report.to_json(20);
        let value = serde_json::to_value(&json).expect("serialize");

        assert_eq!(value["limit"], 20);
        assert_eq!(value["sessions"], serde_json::json!([]));
        assert_eq!(value["deletions"], serde_json::json!([]));
        assert_eq!(
            value["logs"]["operations"],
            serde_json::Value::String(ops.display().to_string())
        );
        assert_eq!(
            value["logs"]["deletions"],
            serde_json::Value::String(dels.display().to_string())
        );
    }

    #[test]
    fn parse_operations_session_with_trashed_and_end() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ops = dir.path().join("operations.log");
        let dels = dir.path().join("deletions.log");
        fs::write(
            &ops,
            "\
# ========== clean session started at 2026-05-24 10:00:00 ==========
[2026-05-24 10:00:01] [clean] REMOVED /tmp/cache one (2KB)
[2026-05-24 10:00:02] [clean] TRASHED /tmp/Old App.app (4KB)
[2026-05-24 10:00:03] [clean] SKIPPED /tmp/protected (whitelist)
[2026-05-24 10:00:04] [clean] FAILED /tmp/fail (permission denied)
# ========== clean session ended at 2026-05-24 10:00:05, 2 items, 6KB ==========
",
        )
        .expect("write ops");

        let report = HistoryReport::load(&ops, &dels);
        let json = report.to_json(20);
        assert_eq!(json.sessions.len(), 1);
        let s = &json.sessions[0];
        assert_eq!(s.command, "clean");
        assert_eq!(s.started_at, "2026-05-24 10:00:00");
        assert_eq!(s.ended_at, "2026-05-24 10:00:05");
        assert_eq!(s.items, 2);
        assert_eq!(s.size, "6KB");
        assert_eq!(s.operation_count, 4);
        assert_eq!(s.actions.removed, 1);
        assert_eq!(s.actions.trashed, 1);
        assert_eq!(s.actions.skipped, 1);
        assert_eq!(s.actions.failed, 1);
        assert!(!s.ended_at.is_empty());
    }

    #[test]
    fn orphan_operation_starts_implicit_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ops = dir.path().join("operations.log");
        let dels = dir.path().join("deletions.log");
        fs::write(
            &ops,
            "[2026-05-24 10:00:01] [clean] TRASHED /tmp/x (1KB)\n",
        )
        .expect("write ops");

        let report = HistoryReport::load(&ops, &dels);
        let json = report.to_json(20);
        assert_eq!(json.sessions.len(), 1);
        assert_eq!(json.sessions[0].command, "clean");
        assert_eq!(json.sessions[0].started_at, "2026-05-24 10:00:01");
        assert_eq!(json.sessions[0].ended_at, "");
        assert_eq!(json.sessions[0].actions.trashed, 1);
    }

    #[test]
    fn unfinished_session_has_empty_ended_at() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ops = dir.path().join("operations.log");
        let dels = dir.path().join("deletions.log");
        fs::write(
            &ops,
            "\
# ========== clean session started at 2026-05-24 10:00:00 ==========
[2026-05-24 10:00:01] [clean] REMOVED /tmp/cache (2KB)
",
        )
        .expect("write ops");

        let report = HistoryReport::load(&ops, &dels);
        let json = report.to_json(20);
        assert_eq!(json.sessions.len(), 1);
        assert_eq!(json.sessions[0].ended_at, "");
        assert_eq!(json.sessions[0].actions.removed, 1);
    }
}
