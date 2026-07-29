//! 系统状态采集与健康分。

mod collector;
mod health;
mod ring;

pub use collector::{CollectionMode, CollectError, StatusCollector};
pub use health::calculate_health_score;
pub use ring::RingBuffer;
