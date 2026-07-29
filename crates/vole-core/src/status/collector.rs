//! 状态快照采集调度。

use std::time::Duration;

use thiserror::Error;
use crate::vole_proto::status::StatusSnapshot;

pub const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
pub const SLOW_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionMode {
    Fast,
    Process,
    Full,
}

#[derive(Debug, Error)]
pub enum CollectError {
    #[error("采集失败: {0}")]
    Failed(String),
}

pub struct StatusCollector;

impl Default for StatusCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn collect(&self, mode: CollectionMode) -> Result<StatusSnapshot, CollectError> {
        let _ = mode;
        Ok(StatusSnapshot::default())
    }
}
