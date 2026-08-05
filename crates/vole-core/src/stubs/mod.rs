//! Container stubs orphan（CleanMyMac allowlist，Mole `clean_orphaned_container_stubs` 同形）。

mod select;

pub use select::{is_verified_stub_dir, select_container_stubs, StubScanError};

pub const CONTAINER_STUB_RULE_ID: &str = "orphaned-container-stubs";

/// stub 目录里唯一允许存在的条目。
pub const CONTAINER_STUB_METADATA: &str = ".com.apple.containermanagerd.metadata.plist";

/// 硬编码 allowlist（对齐 Mole 1.48.1）；扩表须另开 design。
pub const STUB_ALLOWLIST: &[(&str, &str)] = &[
    ("com.macpaw.CleanMyMac*", "/Applications/CleanMyMac X.app"),
    ("*.com.macpaw.CleanMyMac*", "/Applications/CleanMyMac X.app"),
];

/// Plan 条目 label：`Orphaned container stub: <bundle_id>`。
pub fn container_stub_label(path: &std::path::Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    format!("Orphaned container stub: {name}")
}
