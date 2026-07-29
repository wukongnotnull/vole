//! 状态快照采集调度。

use std::time::Duration;

use thiserror::Error;
use vole_sys::MacStatusCollector;

use crate::status::health::calculate_health_score;
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

pub struct StatusCollector {
    backend: MacStatusCollector,
}

impl Default for StatusCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusCollector {
    pub fn new() -> Self {
        Self {
            backend: MacStatusCollector::new(),
        }
    }

    pub fn collect(&mut self, mode: CollectionMode) -> Result<StatusSnapshot, CollectError> {
        let full_hw = mode == CollectionMode::Full;
        let mut snap = self.backend.collect_snapshot(full_hw);
        let (score, msg) = calculate_health_score(
            &snap.cpu,
            &snap.memory,
            &snap.disks,
            &snap.disk_io,
            &snap.thermal,
            &snap.batteries,
            snap.uptime_seconds,
        );
        snap.health_score = score;
        snap.health_score_msg = msg;
        Ok(snap)
    }

    pub fn collect_full(&mut self) -> Result<StatusSnapshot, CollectError> {
        self.collect(CollectionMode::Full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_full_returns_valid_ranges() {
        let mut c = StatusCollector::new();
        let snap = c.collect_full().expect("collect");
        assert!(snap.cpu.usage >= 0.0 && snap.cpu.usage <= 100.0);
        assert!(snap.memory.used_percent >= 0.0 && snap.memory.used_percent <= 100.0);
        assert!(snap.health_score >= 0 && snap.health_score <= 100);
        assert!(!snap.host.is_empty());
    }
}
