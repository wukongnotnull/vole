mod fs;
mod metrics;
mod plist;
mod sqlite;
mod syscommand;
mod trash;

use std::sync::Arc;

pub use fs::MacFs;
pub use metrics::MacMetrics;
pub use plist::MacPlist;
pub use sqlite::MacSqlite;
pub use syscommand::MacSysCommand;
pub use trash::MacTrash;

pub struct MacOsBackend {
    pub fs: Arc<MacFs>,
    pub syscommand: Arc<MacSysCommand>,
    pub plist: Arc<MacPlist>,
    pub sqlite: Arc<MacSqlite>,
    pub trash: Arc<MacTrash>,
    pub metrics: Arc<MacMetrics>,
}

impl Default for MacOsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOsBackend {
    pub fn new() -> Self {
        Self {
            fs: Arc::new(MacFs),
            syscommand: Arc::new(MacSysCommand),
            plist: Arc::new(MacPlist),
            sqlite: Arc::new(MacSqlite),
            trash: Arc::new(MacTrash),
            metrics: Arc::new(MacMetrics),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MacOsBackend;
    use crate::traits::Metrics;

    #[test]
    fn backend_constructs() {
        let b = MacOsBackend::new();
        assert!(b.metrics.total_disk_bytes() > 0);
    }
}
