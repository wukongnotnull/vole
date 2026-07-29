//! 集中超时常量，对齐 Mole `lib/core/timeouts.sh`。

use std::time::Duration;

pub type DurationSec = Duration;

pub const QUICK_DETECT: Duration = Duration::from_secs(2);
pub const SHORT_QUERY: Duration = Duration::from_secs(3);
pub const MEDIUM_PROBE: Duration = Duration::from_secs(5);
pub const PKG_LIST: Duration = Duration::from_secs(10);
pub const HINT_SCAN: Duration = Duration::from_secs(15);
pub const PKG_CLEANUP: Duration = Duration::from_secs(20);
pub const DISK_VERIFY: Duration = Duration::from_secs(30);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_detect_is_two_seconds() {
        assert_eq!(QUICK_DETECT.as_secs(), 2);
    }
}
