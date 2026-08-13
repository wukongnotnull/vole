//! Mole-compatible history load/render (`operations.log` + `deletions.log`).

mod json;
mod parse;
mod session;

pub use json::{
    normalize_limit, HistoryActions, HistoryDeletion, HistoryJson, HistoryLogs, HistoryReport,
    HistorySession, DEFAULT_LIMIT, MAX_LIMIT,
};

use std::path::PathBuf;

/// Canonical operations.log path (vole write location, or env override).
pub fn operations_log_path() -> PathBuf {
    crate::user_paths::operations_log_write_path()
}

/// Canonical deletions.log path (vole write location, or env override).
pub fn deletions_log_path() -> PathBuf {
    crate::user_paths::deletions_log_write_path()
}

/// Load vole logs, plus leftover Mole logs when no env override is set.
pub fn load_default() -> HistoryReport {
    let ops_display = operations_log_path();
    let dels_display = deletions_log_path();
    let ops_overridden = crate::user_paths::operations_log_env_overridden();
    let dels_overridden = crate::user_paths::deletions_log_env_overridden();

    let mut sessions = Vec::new();
    if !ops_overridden {
        sessions.extend(session::load_sessions(
            &crate::user_paths::mole_logs_dir().join("operations.log"),
        ));
    }
    sessions.extend(session::load_sessions(&ops_display));

    let mut deletions = Vec::new();
    if !dels_overridden {
        deletions.extend(session::load_deletions(
            &crate::user_paths::mole_logs_dir().join("deletions.log"),
        ));
    }
    deletions.extend(session::load_deletions(&dels_display));

    HistoryReport::from_parts(ops_display, dels_display, sessions, deletions)
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
    fn operations_log_path_defaults_to_vole_when_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("h");
        std::fs::create_dir_all(&home).unwrap();
        let _guard = crate::test_env::lock();
        std::env::remove_var("VOLE_OPERATIONS_LOG");
        std::env::remove_var("MOLE_OPERATIONS_LOG");
        std::env::remove_var("OPERATIONS_LOG_FILE");
        std::env::set_var("HOME", &home);
        let path = operations_log_path();
        assert!(
            path.ends_with("Library/Logs/vole/operations.log"),
            "{}",
            path.display()
        );
        std::env::remove_var("HOME");
    }

    #[test]
    fn load_default_includes_mole_and_vole_sessions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("h");
        let mole = home.join("Library/Logs/mole");
        let vole = home.join("Library/Logs/vole");
        std::fs::create_dir_all(&mole).unwrap();
        std::fs::create_dir_all(&vole).unwrap();
        fs::write(
            mole.join("operations.log"),
            "\
# ========== clean session started at 2026-05-24 10:00:00 ==========
[2026-05-24 10:00:01] [clean] REMOVED /tmp/mole (1KB)
# ========== clean session ended at 2026-05-24 10:00:02, 1 items, 1KB ==========
",
        )
        .unwrap();
        fs::write(
            vole.join("operations.log"),
            "\
# ========== purge session started at 2026-05-24 11:00:00 ==========
[2026-05-24 11:00:01] [purge] REMOVED /tmp/vole (2KB)
# ========== purge session ended at 2026-05-24 11:00:02, 1 items, 2KB ==========
",
        )
        .unwrap();
        let _guard = crate::test_env::lock();
        std::env::remove_var("VOLE_OPERATIONS_LOG");
        std::env::remove_var("MOLE_OPERATIONS_LOG");
        std::env::remove_var("OPERATIONS_LOG_FILE");
        std::env::remove_var("VOLE_DELETE_LOG");
        std::env::remove_var("MOLE_DELETE_LOG");
        std::env::set_var("HOME", &home);
        let json = load_default().to_json(20);
        assert_eq!(json.sessions[0].command, "purge");
        assert_eq!(json.sessions[1].command, "clean");
        assert!(json
            .logs
            .operations
            .contains("Library/Logs/vole/operations.log"));
        std::env::remove_var("HOME");
    }

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
        fs::write(&ops, "[2026-05-24 10:00:01] [clean] TRASHED /tmp/x (1KB)\n").expect("write ops");

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
