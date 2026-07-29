//! Mole-compatible history load/render (`operations.log` + `deletions.log`).

mod json;

pub use json::{
    HistoryActions, HistoryDeletion, HistoryJson, HistoryLogs, HistoryReport, HistorySession,
};

#[cfg(test)]
mod tests {
    use super::*;
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
}
