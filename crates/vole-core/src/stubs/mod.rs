//! Container stubs orphan（CleanMyMac allowlist，Mole `clean_orphaned_container_stubs` 同形）。

mod remove;
mod select;

pub use remove::{remove_verified_container_stub, StubRemoveError};
pub use select::{is_verified_stub_dir, select_container_stubs, StubScanError};

pub const CONTAINER_STUB_RULE_ID: &str = "orphaned-container-stubs";

/// stub 目录里唯一允许存在的条目。
pub const CONTAINER_STUB_METADATA: &str = ".com.apple.containermanagerd.metadata.plist";

/// 硬编码 allowlist（对齐 Mole 1.48.1）；扩表须另开 design。
pub const STUB_ALLOWLIST: &[(&str, &str)] = &[
    ("com.macpaw.CleanMyMac*", "/Applications/CleanMyMac X.app"),
    ("*.com.macpaw.CleanMyMac*", "/Applications/CleanMyMac X.app"),
];

/// Plan 入选闸口豁免的形状校验：路径必须恰为 `home/Library/Containers/<单层名>`
/// （拒绝更深层级、`..`、非 Normal 组件）。豁免 `validate_path_for_deletion`
/// 的候选必须先通过本检查。
pub fn is_container_stub_candidate_path(path: &std::path::Path, home: &std::path::Path) -> bool {
    let containers = home.join("Library/Containers");
    let Ok(rel) = path.strip_prefix(&containers) else {
        return false;
    };
    let mut comps = rel.components();
    matches!(
        (comps.next(), comps.next()),
        (Some(std::path::Component::Normal(_)), None)
    )
}

/// Plan 条目 label：`Orphaned container stub: <bundle_id>`。
pub fn container_stub_label(path: &std::path::Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    format!("Orphaned container stub: {name}")
}
