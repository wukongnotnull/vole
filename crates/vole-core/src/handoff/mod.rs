//! Handoff / Universal Clipboard pasteboard 暂存清理（Mole `clean_handoff_pasteboard_cache` 同形）。
//! 零保护层改动；见 design 2026-08-06-1716。

mod select;

pub use select::{
    recheck_handoff_pasteboard_entry, select_handoff_pasteboard, HandoffScanError,
    HandoffSelectResult,
};

use std::path::{Component, Path, PathBuf};

pub const HANDOFF_PASTEBOARD_RULE_ID: &str = "handoff-pasteboard-cache";
pub const HANDOFF_MTIME_MINUTES: u64 = 60;
pub const MAX_HANDOFF_LEAVES: usize = 2000;

pub fn handoff_pasteboard_root(home: &Path) -> PathBuf {
    home.join(
        "Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard",
    )
}

/// 必须恰为 `…/shared-pasteboard/<单层名>`（政策重验用，非 protect 豁免）。
pub fn is_handoff_pasteboard_leaf_path(path: &Path, home: &Path) -> bool {
    let root = handoff_pasteboard_root(home);
    let Ok(rel) = path.strip_prefix(&root) else {
        return false;
    };
    let mut comps = rel.components();
    matches!(
        (comps.next(), comps.next()),
        (Some(Component::Normal(_)), None)
    )
}

pub fn handoff_pasteboard_label(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    format!("Handoff pasteboard: {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn leaf_path_gate_accepts_single_component() {
        let home = Path::new("/Users/t");
        assert!(is_handoff_pasteboard_leaf_path(
            &handoff_pasteboard_root(home).join("item1"),
            home
        ));
        assert!(!is_handoff_pasteboard_leaf_path(
            &handoff_pasteboard_root(home).join("a").join("b"),
            home
        ));
        assert!(!is_handoff_pasteboard_leaf_path(
            &home.join("Library/Group Containers/group.com.apple.coreservices.useractivityd/other"),
            home
        ));
        assert!(!is_handoff_pasteboard_leaf_path(
            &handoff_pasteboard_root(home),
            home
        ));
    }

    #[test]
    fn label_uses_basename() {
        assert_eq!(
            handoff_pasteboard_label(Path::new(
                "/Users/t/Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard/abc"
            )),
            "Handoff pasteboard: abc"
        );
    }
}
