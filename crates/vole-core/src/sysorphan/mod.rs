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

pub const SYSTEM_SERVICES_RULE_ID: &str = "orphaned-system-services";

/// Plan 条目 label：`Orphaned LaunchDaemon|LaunchAgent|PrivilegedHelper: <id>`。
pub fn system_service_label(path: &std::path::Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .trim_end_matches(".plist");
    let kind = {
        let s = path.to_string_lossy();
        if s.contains("LaunchDaemons") {
            "LaunchDaemon"
        } else if s.contains("LaunchAgents") {
            "LaunchAgent"
        } else {
            "PrivilegedHelper"
        }
    };
    format!("Orphaned {kind}: {name}")
}
