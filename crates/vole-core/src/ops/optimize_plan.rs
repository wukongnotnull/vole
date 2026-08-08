//! `optimize` plan：聚合 M3 任务发现器 → ProtoPlan。

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::optimize::{
    discover_cache_refresh, discover_fix_broken_configs, discover_launch_agents_cleanup,
    discover_saved_state_cleanup, optimize_action_rule_id, optimize_catalog,
    optimize_delete_rule_id, plan_coreduet_cleanup, plan_disk_permissions_repair,
    plan_dock_refresh, plan_launch_services_rebuild, plan_legacy_overrides_audit,
    plan_memory_pressure_relief, plan_network_optimization, plan_network_stack_optimize,
    plan_notification_cleanup, plan_periodic_maintenance, plan_prevent_network_dsstore,
    plan_quarantine_cleanup, plan_sqlite_vacuum, plan_system_maintenance, OptimizeCandidate,
    OptimizeTaskKind,
};
use crate::protection::{AppProtection, ProtectionCatalog};
use crate::safety::capture_plan_entry_identity;
use crate::vole_proto::{Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, SCHEMA_VERSION};

use super::plan::DEFAULT_PLAN_TTL;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OptimizePlanError {
    #[error("unknown optimize task id: {0}")]
    UnknownTask(String),
}

pub struct OptimizePlanOptions<'a> {
    pub home: &'a Path,
    pub ttl_secs: u64,
    pub only_task: Option<&'a str>,
}

pub fn build_optimize_plan(
    catalog: &ProtectionCatalog,
    _protection: &AppProtection,
    opts: &OptimizePlanOptions<'_>,
) -> Result<ProtoPlan, OptimizePlanError> {
    if let Some(id) = opts.only_task {
        if !optimize_catalog().iter().any(|t| t.id == id) {
            return Err(OptimizePlanError::UnknownTask(id.to_string()));
        }
    }

    let mut candidates: Vec<OptimizeCandidate> = Vec::new();
    let allow = |task_id: &str| -> bool {
        match opts.only_task {
            Some(only) => only == task_id,
            None => optimize_catalog()
                .iter()
                .find(|t| t.id == task_id)
                .map(|t| t.in_m3)
                .unwrap_or(false),
        }
    };

    if allow("saved_state_cleanup") {
        candidates.extend(discover_saved_state_cleanup(opts.home, catalog));
    }
    if allow("cache_refresh") {
        candidates.extend(discover_cache_refresh(opts.home, catalog));
    }
    if allow("fix_broken_configs") {
        candidates.extend(discover_fix_broken_configs(opts.home, catalog));
    }
    if allow("launch_agents_cleanup") {
        candidates.extend(discover_launch_agents_cleanup(opts.home, catalog));
    }
    if allow("quarantine_cleanup") {
        if let Some(c) = plan_quarantine_cleanup(opts.home, catalog) {
            candidates.push(c);
        }
    }
    if allow("sqlite_vacuum") {
        candidates.extend(plan_sqlite_vacuum(opts.home, catalog));
    }
    if allow("prevent_network_dsstore") {
        if let Some(c) = plan_prevent_network_dsstore(opts.home) {
            candidates.push(c);
        }
    }
    if allow("legacy_overrides_audit") {
        candidates.extend(plan_legacy_overrides_audit(opts.home));
    }
    if allow("notification_cleanup") {
        if let Some(c) = plan_notification_cleanup(opts.home, catalog) {
            candidates.push(c);
        }
    }
    if allow("coreduet_cleanup") {
        if let Some(c) = plan_coreduet_cleanup(opts.home, catalog) {
            candidates.push(c);
        }
    }
    if allow("dock_refresh") {
        candidates.push(plan_dock_refresh(opts.home));
    }
    if allow("launch_services_rebuild") {
        candidates.push(plan_launch_services_rebuild(opts.home));
    }
    if allow("system_maintenance") {
        candidates.push(plan_system_maintenance(opts.home));
    }
    if allow("network_optimization") {
        candidates.push(plan_network_optimization(opts.home));
    }
    if allow("memory_pressure_relief") {
        candidates.push(plan_memory_pressure_relief(opts.home));
    }
    if allow("network_stack_optimize") {
        candidates.push(plan_network_stack_optimize(opts.home));
    }
    if allow("disk_permissions_repair") {
        candidates.push(plan_disk_permissions_repair(opts.home));
    }
    if allow("periodic_maintenance") {
        candidates.push(plan_periodic_maintenance(opts.home));
    }

    let mut entries = Vec::new();
    for (idx, c) in candidates.into_iter().enumerate() {
        let rule_id = match c.kind {
            OptimizeTaskKind::Delete => optimize_delete_rule_id(c.task_id),
            OptimizeTaskKind::Action => optimize_action_rule_id(c.task_id),
        };
        let (dev, ino, mtime) = match capture_plan_entry_identity(&c.path) {
            Ok(id) => (
                id.dev,
                id.ino,
                UNIX_EPOCH + Duration::from_secs(id.mtime.max(0) as u64),
            ),
            Err(_) => (0, 0, UNIX_EPOCH),
        };
        entries.push(ProtoPlanEntry {
            id: format!("{}-{idx}", c.task_id),
            path: c.path,
            label: c.label,
            size: c.size,
            rule_id,
            skip_reason: None,
            dev,
            ino,
            mtime,
        });
    }

    let coverage_note = coverage_note_for_long_tail(opts.only_task);

    Ok(ProtoPlan {
        schema_version: SCHEMA_VERSION,
        created_at: SystemTime::now(),
        ttl_secs: if opts.ttl_secs == 0 {
            DEFAULT_PLAN_TTL.as_secs()
        } else {
            opts.ttl_secs
        },
        entries,
        coverage_note,
    })
}

fn coverage_note_for_long_tail(only_task: Option<&str>) -> Option<String> {
    if only_task.is_some() {
        return None;
    }
    let skipped: Vec<&str> = optimize_catalog()
        .iter()
        .filter(|t| !t.in_m3)
        .map(|t| t.title)
        .collect();
    if skipped.is_empty() {
        return None;
    }
    Some(format!(
        "Skipped sudo/system long-tail optimize tasks (use Mole if needed): {}",
        skipped.join("; ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::fs::FileTimes;
    use std::time::{Duration, SystemTime};

    use crate::protection::{AppProtection, ProtectionCatalog};

    #[test]
    fn build_plan_includes_old_saved_state_and_coverage() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let state = home.join("Library/Saved Application State/com.example.old.savedState");
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join("w.plist"), b"x").unwrap();
        let modified = SystemTime::now()
            .checked_sub(Duration::from_secs(40 * 86_400))
            .unwrap();
        fs::File::open(&state)
            .unwrap()
            .set_times(FileTimes::new().set_modified(modified))
            .unwrap();

        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let plan = build_optimize_plan(
            &catalog,
            &protection,
            &OptimizePlanOptions {
                home,
                ttl_secs: 900,
                only_task: None,
            },
        )
        .unwrap();
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "optimize:delete:saved_state_cleanup"));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "optimize:action:dock_refresh"));
        let note = plan.coverage_note.unwrap();
        assert!(!note.contains("Memory Optimization"));
        assert!(note.contains("Network Stack") || note.contains("Spotlight"));
    }

    #[test]
    fn build_plan_includes_system_and_network_sentinels() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let catalog = ProtectionCatalog::embedded();
        let protection = AppProtection::new();
        let plan = build_optimize_plan(
            &catalog,
            &protection,
            &OptimizePlanOptions {
                home,
                ttl_secs: 900,
                only_task: None,
            },
        )
        .unwrap();
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "optimize:action:system_maintenance"));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "optimize:action:network_optimization"));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "optimize:action:memory_pressure_relief"));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "optimize:action:network_stack_optimize"));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "optimize:action:disk_permissions_repair"));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "optimize:action:periodic_maintenance"));
        let note = plan.coverage_note.unwrap();
        assert!(!note.contains("DNS & Spotlight Check"));
        assert!(!note.contains("Network Cache Refresh"));
        assert!(!note.contains("Memory Optimization"));
        assert!(!note.contains("Network Stack Refresh"));
        assert!(!note.contains("Permission Repair"));
        assert!(!note.contains("Periodic Maintenance"));
        assert!(note.contains("Spotlight") || note.contains("Shared File Lists"));
    }
}
