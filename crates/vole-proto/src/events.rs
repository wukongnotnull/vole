use serde::{Deserialize, Serialize};

use crate::report::Report;
use crate::SkipReason;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Progress {
        scanned: u64,
        current: String,
    },
    Candidate {
        id: String,
        path: String,
        label: String,
        size: u64,
        rule_id: String,
    },
    Skipped {
        rule_id: String,
        reason: SkipReason,
    },
    Done {
        report: Report,
    },
    Aborted {
        reason: String,
    },
}

impl StreamEvent {
    pub fn with_schema(self, schema_version: u32) -> serde_json::Value {
        let mut v = serde_json::to_value(self).expect("StreamEvent 可序列化");
        if let Some(obj) = v.as_object_mut() {
            obj.insert("schema_version".into(), schema_version.into());
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn progress_event_serializes_with_snake_case_type() {
        let e = StreamEvent::Progress {
            scanned: 100,
            current: "~/Library/Caches".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "progress");
        assert_eq!(v["scanned"], 100);
    }

    #[test]
    fn done_event_wraps_report() {
        let e = StreamEvent::Done {
            report: Report {
                succeeded: 1,
                skipped: 2,
                failed: 0,
                skipped_by_reason: vec![],
                trashed_bytes: 0,
                deleted_bytes: 0,
                coverage_note: None,
            },
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "done");
        assert_eq!(v["report"]["succeeded"], 1);
    }

    #[test]
    fn with_schema_injects_version() {
        let v = StreamEvent::Progress {
            scanned: 0,
            current: ".".into(),
        }
        .with_schema(1);
        assert_eq!(v["schema_version"], json!(1));
    }
}
