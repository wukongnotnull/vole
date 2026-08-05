//! System services orphan 扫描（可读子集，无 sudo）。

mod plist;
mod probe;

pub use plist::read_launchd_program;
pub use probe::{is_package_managed_binary, probe_binary_presence, BinaryPresence};
