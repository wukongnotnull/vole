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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub schema_version: u32,
    #[serde(with = "serde_time")]
    pub created_at: SystemTime,
    pub ttl_secs: u64,
    pub entries: Vec<PlanEntry>,
}
