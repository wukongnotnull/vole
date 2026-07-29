use sysinfo::Disks;

use crate::traits::Metrics;

pub struct MacMetrics;

impl Metrics for MacMetrics {
    fn total_disk_bytes(&self) -> u64 {
        Disks::new_with_refreshed_list()
            .iter()
            .map(|d| d.total_space())
            .sum()
    }
}
