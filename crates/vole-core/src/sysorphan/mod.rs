//! System services orphan 扫描（可读子集，无 sudo）。

mod plist;
mod probe;
mod protect;
mod select;

pub use plist::read_launchd_program;
pub use probe::{is_package_managed_binary, probe_binary_presence, BinaryPresence};
pub use protect::{is_known_protected, system_service_app_exists, KNOWN_PROTECT_PATTERNS};
pub use select::{
    privileged_helper_bundle_id_from_binary, select_system_service_orphans, SysOrphanScanError,
    SystemServiceRoots,
};
