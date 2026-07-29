use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::SkipReason;

mod serde_time {
    use super::*;

    pub fn serialize<S>(time: &SystemTime, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let secs = time
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        s.serialize_u64(secs)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(d)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntry {
    pub id: String,
    pub path: PathBuf,
    pub label: String,
    pub size: u64,
    pub rule_id: String,
    pub skip_reason: Option<SkipReason>,
    pub dev: u64,
    pub ino: u64,
    #[serde(with = "serde_time")]
    pub mtime: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub schema_version: u32,
    #[serde(with = "serde_time")]
    pub created_at: SystemTime,
    pub ttl_secs: u64,
    pub entries: Vec<PlanEntry>,
    /// plan 阶段：规则覆盖说明（可选；Phase 4c v1 未移植类别提示）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SCHEMA_VERSION;

    #[test]
    fn plan_json_roundtrip() {
        let plan = Plan {
            schema_version: SCHEMA_VERSION,
            created_at: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            ttl_secs: 900,
            coverage_note: None,
            entries: vec![PlanEntry {
                id: "chrome-cache-0".into(),
                path: PathBuf::from("/Users/test/Library/Caches/Google"),
                label: "Chrome cache".into(),
                size: 1024,
                rule_id: "chrome-cache".into(),
                skip_reason: None,
                dev: 17,
                ino: 42,
                mtime: UNIX_EPOCH + Duration::from_secs(1_700_000_001),
            }],
        };
        let json = serde_json::to_string(&plan).unwrap();
        let back: Plan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }
}
