//! Optimize `spotlight_index_optimize`（对齐 Mole `opt_spotlight_index_optimize`）。
//!
//! 与 `system_maintenance` 去重：后者仅 `mdutil -s` 只读检查；本模块条件满足时才 `mdutil -E`。

use std::process::{Command, Stdio};
use std::time::Instant;

use super::delete_paths::OptimizeCandidate;
use crate::optimize::OptimizeTaskKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotlightIndexStatus {
    Disabled,
    Enabled,
    Other,
}

/// Whether apply should call `rebuild_spotlight_index`.
pub fn spotlight_index_needs_rebuild() -> bool {
    match spotlight_status() {
        SpotlightIndexStatus::Disabled | SpotlightIndexStatus::Other => return false,
        SpotlightIndexStatus::Enabled => {}
    }
    if !is_ac_power() {
        return false;
    }
    probes_are_slow()
}

pub fn plan_spotlight_index_optimize(home: &std::path::Path) -> OptimizeCandidate {
    OptimizeCandidate {
        path: home.join(".vole-optimize-action/spotlight_index_optimize"),
        label: "Spotlight Optimization".into(),
        size: 0,
        task_id: "spotlight_index_optimize",
        kind: OptimizeTaskKind::Action,
    }
}

fn env_flag_tri(name: &str) -> Option<bool> {
    let Ok(v) = std::env::var(name) else {
        return None;
    };
    match v.trim() {
        "1" | "true" | "TRUE" | "yes" | "YES" => Some(true),
        "0" | "false" | "FALSE" | "no" | "NO" => Some(false),
        _ => None,
    }
}

pub fn spotlight_status() -> SpotlightIndexStatus {
    if let Ok(v) = std::env::var("VOLE_TEST_SPOTLIGHT_STATUS") {
        return match v.trim().to_ascii_lowercase().as_str() {
            "disabled" => SpotlightIndexStatus::Disabled,
            "enabled" => SpotlightIndexStatus::Enabled,
            _ => SpotlightIndexStatus::Other,
        };
    }
    let Ok(out) = Command::new("mdutil")
        .args(["-s", "/"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        return SpotlightIndexStatus::Other;
    };
    let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
    if text.contains("indexing disabled") {
        return SpotlightIndexStatus::Disabled;
    }
    if text.contains("indexing enabled") && !text.contains("indexing and searching disabled") {
        return SpotlightIndexStatus::Enabled;
    }
    SpotlightIndexStatus::Other
}

pub fn is_ac_power() -> bool {
    if let Some(v) = env_flag_tri("VOLE_TEST_AC_POWER") {
        return v;
    }
    let Ok(out) = Command::new("pmset")
        .args(["-g", "batt"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        // Fail-closed: unknown power → do not rebuild.
        return false;
    };
    String::from_utf8_lossy(&out.stdout).contains("AC Power")
}

fn slow_threshold_secs() -> i64 {
    std::env::var("VOLE_OPTIMIZE_SPOTLIGHT_SLOW_SEC")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
}

pub fn probes_are_slow() -> bool {
    if let Some(v) = env_flag_tri("VOLE_TEST_SPOTLIGHT_SLOW") {
        return v;
    }
    let threshold = slow_threshold_secs();
    let mut slow_count = 0;
    for i in 0..2 {
        let start = Instant::now();
        let _ = Command::new("mdfind")
            .arg("kMDItemFSName == 'Applications'")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let secs = start.elapsed().as_secs() as i64;
        if secs > threshold {
            slow_count += 1;
        }
        if i == 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    slow_count >= 2
}

#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn plan_emits_sentinel() {
        let home = tempdir().unwrap();
        let c = plan_spotlight_index_optimize(home.path());
        assert_eq!(c.task_id, "spotlight_index_optimize");
        assert!(c
            .path
            .ends_with(".vole-optimize-action/spotlight_index_optimize"));
    }

    #[test]
    fn needs_rebuild_respects_env_gates() {
        let _guard = test_env_lock();
        std::env::set_var("VOLE_TEST_SPOTLIGHT_STATUS", "disabled");
        std::env::set_var("VOLE_TEST_AC_POWER", "1");
        std::env::set_var("VOLE_TEST_SPOTLIGHT_SLOW", "1");
        assert!(!spotlight_index_needs_rebuild());

        std::env::set_var("VOLE_TEST_SPOTLIGHT_STATUS", "enabled");
        std::env::set_var("VOLE_TEST_AC_POWER", "0");
        assert!(!spotlight_index_needs_rebuild());

        std::env::set_var("VOLE_TEST_AC_POWER", "1");
        std::env::set_var("VOLE_TEST_SPOTLIGHT_SLOW", "0");
        assert!(!spotlight_index_needs_rebuild());

        std::env::set_var("VOLE_TEST_SPOTLIGHT_SLOW", "1");
        assert!(spotlight_index_needs_rebuild());

        std::env::remove_var("VOLE_TEST_SPOTLIGHT_STATUS");
        std::env::remove_var("VOLE_TEST_AC_POWER");
        std::env::remove_var("VOLE_TEST_SPOTLIGHT_SLOW");
    }
}
