//! 将内部 `ops::Plan` 转为可序列化的 `vole_proto::Plan`。

use std::time::{Duration, UNIX_EPOCH};

use crate::vole_proto::{Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, SCHEMA_VERSION};

use super::plan::{Plan, PlanEntry};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtoPlanError {
    #[error("plan entry {id} missing identity snapshot")]
    MissingIdentity { id: String },
}

/// 将 plan 生成结果转为协议层 `Plan`（含 `dev` / `ino` / `mtime`）。
pub fn plan_to_proto(plan: &Plan) -> Result<ProtoPlan, ProtoPlanError> {
    let entries = plan
        .entries
        .iter()
        .map(entry_to_proto)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProtoPlan {
        schema_version: SCHEMA_VERSION,
        created_at: plan.generated_at,
        ttl_secs: plan.ttl.as_secs(),
        coverage_note: None,
        entries,
    })
}

fn entry_to_proto(entry: &PlanEntry) -> Result<ProtoPlanEntry, ProtoPlanError> {
    let identity = entry
        .identity
        .ok_or_else(|| ProtoPlanError::MissingIdentity {
            id: entry.id.clone(),
        })?;
    Ok(ProtoPlanEntry {
        id: entry.id.clone(),
        path: entry.path.clone(),
        label: entry.label.clone(),
        size: entry.size,
        rule_id: entry.rule_id.clone(),
        skip_reason: None,
        dev: identity.dev,
        ino: identity.ino,
        mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
    blockers: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::plan::PlanEntry;
    use crate::safety::PlanEntryIdentity;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn sample_plan() -> Plan {
        Plan {
            generated_at: SystemTime::UNIX_EPOCH,
            ttl: Duration::from_secs(900),
            entries: vec![PlanEntry {
                id: "rule-0".into(),
                path: PathBuf::from("/tmp/cache.db"),
                label: "Cache".into(),
                size: 42,
                rule_id: "rule".into(),
                skip_reason: None,
                identity: Some(PlanEntryIdentity {
                    dev: 1,
                    ino: 2,
                    mtime: 1_700_000_000,
                }),
            }],
            notices: vec![],
        }
    }

    #[test]
    fn converts_ops_plan_to_proto() {
        let proto = plan_to_proto(&sample_plan()).unwrap();
        assert_eq!(proto.schema_version, SCHEMA_VERSION);
        assert_eq!(proto.ttl_secs, 900);
        assert_eq!(proto.entries.len(), 1);
        let e = &proto.entries[0];
        assert_eq!(e.dev, 1);
        assert_eq!(e.ino, 2);
        assert_eq!(e.size, 42);
    }

    #[test]
    fn proto_plan_json_roundtrip() {
        let mut proto = plan_to_proto(&sample_plan()).unwrap();
        proto.coverage_note = Some("test note".into());
        let json = serde_json::to_string(&proto).unwrap();
        let back: ProtoPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, proto);
        assert_eq!(back.coverage_note.as_deref(), Some("test note"));
    }

    #[test]
    fn missing_identity_is_error() {
        let mut plan = sample_plan();
        plan.entries[0].identity = None;
        assert!(matches!(
            plan_to_proto(&plan),
            Err(ProtoPlanError::MissingIdentity { .. })
        ));
    }
}
