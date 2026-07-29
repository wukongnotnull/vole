//! Mole-compatible history load/render (`operations.log` + `deletions.log`).

mod json;
mod parse;
mod session;

pub use json::{
    HistoryActions, HistoryDeletion, HistoryJson, HistoryLogs, HistoryReport, HistorySession,
    DEFAULT_LIMIT, MAX_LIMIT, normalize_limit,
};

use std::path::PathBuf;

/// Default operations.log path (mole-compatible env overrides).
pub fn operations_log_path() -> PathBuf {
    if let Some(p) = std::env::var_os("MOLE_OPERATIONS_LOG") {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("OPERATIONS_LOG_FILE") {
        return PathBuf::from(p);
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Library/Logs/mole/operations.log"))
        .unwrap_or_else(|| PathBuf::from("Library/Logs/mole/operations.log"))
}

/// Default deletions.log path (shares delete config / MOLE_DELETE_LOG).
pub fn deletions_log_path() -> PathBuf {
    crate::delete::deletion_log_path()
}

/// Load from default mole log paths.
pub fn load_default() -> HistoryReport {
    HistoryReport::load(operations_log_path(), deletions_log_path())
}

/// Human-readable text aligned with mole `history_render_text` sections.
pub fn render_text(report: &HistoryReport, limit: u32) -> String {
    json::render_text(report, limit)
}

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

    #[test]
    fn malformed_session_end_keeps_ops_and_ended_at() {
        // Aligns with mole tests/history.bats "tolerates malformed session summaries".
        let dir = tempfile::tempdir().expect("tempdir");
        let ops = dir.path().join("operations.log");
        let dels = dir.path().join("deletions.log");
        fs::write(
            &ops,
            "\
# ========== clean session started at 2026-05-24 10:00:00 ==========
[2026-05-24 10:00:01] [clean] REMOVED /tmp/cache (2KB)
# ========== clean session ended at malformed summary ==========
",
        )
        .expect("write ops");

        let report = HistoryReport::load(&ops, &dels);
        let json = report.to_json(20);
        assert_eq!(json.sessions.len(), 1);
        let s = &json.sessions[0];
        assert_eq!(s.command, "clean");
        assert_eq!(s.started_at, "2026-05-24 10:00:00");
        assert_eq!(s.ended_at, "malformed summary");
        assert_eq!(s.items, 0);
        assert_eq!(s.size, "0B");
        assert_eq!(s.actions.removed, 1);
        assert_eq!(s.operation_count, 1);
    }

    #[test]
    fn parse_deletions_newest_first_with_limit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ops = dir.path().join("operations.log");
        let dels = dir.path().join("deletions.log");
        fs::write(&ops, "").expect("write ops");
        fs::write(
            &dels,
            "\
2026-05-24T10:00:02+0000\ttrash\t4\tok\t/tmp/Old App.app
2026-05-24T11:00:01+0000\tpermanent\t10\tdry-run\t/tmp/build
not-a-valid-line
2026-05-24T12:00:00+0000\ttrash\tunknown\tok\t/tmp/weird
",
        )
        .expect("write dels");

        let report = HistoryReport::load(&ops, &dels);
        let json = report.to_json(1);
        assert_eq!(json.limit, 1);
        assert_eq!(json.deletions.len(), 1);
        assert_eq!(json.deletions[0].mode, "trash");
        assert_eq!(json.deletions[0].path, "/tmp/weird");
        assert_eq!(json.deletions[0].size_kb, None);

        let json_all = report.to_json(20);
        assert_eq!(json_all.deletions.len(), 3);
        assert_eq!(json_all.deletions[0].path, "/tmp/weird");
        assert_eq!(json_all.deletions[1].mode, "permanent");
        assert_eq!(json_all.deletions[1].size_kb, Some(10));
        assert_eq!(json_all.deletions[2].path, "/tmp/Old App.app");
    }
}
