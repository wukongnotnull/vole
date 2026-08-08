//! `installer` apply：TTL + TOCTOU + Cleanup 保护 + `mole_delete_verified`。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use vole_sys::Trash;

use crate::delete::{
    mole_delete_verified, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions,
};
use crate::oplog::OperationLogger;
use crate::protection::AppProtection;
use crate::safety::{
    verify_plan_entry_for_apply, PlanApplyError, PlanEntryIdentity, ValidationError,
};
use crate::vole_proto::{
    Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, Report, SkipReason, SkipSummary, StreamEvent,
    SCHEMA_VERSION,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstallerApplyError {
    #[error("plan expired; rescan with `vole installer --plan`")]
    Expired,
    #[error("unsupported plan schema version {got} (expected {expected})")]
    UnsupportedSchema { expected: u32, got: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct InstallerApplyOptions {
    pub permanent: bool,
}

pub struct InstallerApplyContext<'a> {
    pub protection: &'a AppProtection,
    pub whitelist_patterns: &'a [String],
    pub options: InstallerApplyOptions,
    pub trash: &'a dyn Trash,
    pub deletion_log: &'a DeletionLogger,
    pub oplog: &'a mut OperationLogger,
    pub on_event: Option<&'a dyn Fn(StreamEvent)>,
    pub now: SystemTime,
}

pub fn apply_installer_plan(
    plan: &ProtoPlan,
    protection: &AppProtection,
    options: InstallerApplyOptions,
    on_event: Option<&dyn Fn(StreamEvent)>,
) -> Result<Report, InstallerApplyError> {
    let deletion_log = DeletionLogger::from_env();
    let mut oplog = OperationLogger::new("installer");
    let _ = oplog.session_start();
    let mut ctx = InstallerApplyContext {
        protection,
        whitelist_patterns: &[],
        options,
        trash: &vole_sys::macos::MacTrash,
        deletion_log: &deletion_log,
        oplog: &mut oplog,
        on_event,
        now: SystemTime::now(),
    };
    let report = apply_installer_proto_plan(plan, &mut ctx)?;
    let _ = oplog.session_end(
        report.succeeded,
        report.trashed_bytes / 1024 + report.deleted_bytes / 1024,
    );
    Ok(report)
}

pub fn apply_installer_proto_plan(
    plan: &ProtoPlan,
    ctx: &mut InstallerApplyContext<'_>,
) -> Result<Report, InstallerApplyError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(InstallerApplyError::UnsupportedSchema {
            expected: SCHEMA_VERSION,
            got: plan.schema_version,
        });
    }
    if plan_is_expired(plan, ctx.now) {
        return Err(InstallerApplyError::Expired);
    }

    let delete_mode = if ctx.options.permanent {
        DeleteMode::Permanent
    } else {
        DeleteMode::Trash
    };

    let mut succeeded = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;
    let mut trashed_bytes = 0u64;
    let mut deleted_bytes = 0u64;
    let mut skip_tracker = SkipTracker::default();

    for (idx, entry) in plan.entries.iter().enumerate() {
        if let Some(event) = &ctx.on_event {
            event(StreamEvent::Progress {
                scanned: idx as u64 + 1,
                current: entry.path.display().to_string(),
            });
        }

        if entry.skip_reason.is_some() {
            skipped += 1;
            let reason = entry
                .skip_reason
                .clone()
                .unwrap_or(SkipReason::PathVanished);
            if let Some(event) = &ctx.on_event {
                event(StreamEvent::Skipped {
                    rule_id: entry.rule_id.clone(),
                    reason: reason.clone(),
                });
            }
            skip_tracker.record(reason, &entry.rule_id);
            continue;
        }

        if !entry.rule_id.starts_with("installer:") {
            skipped += 1;
            skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            continue;
        }

        let path = entry.path.display().to_string();
        let identity = proto_identity(entry);

        if let Err(err) = verify_plan_entry_for_apply(&path, &identity, ctx.protection) {
            let reason = skip_reason_for_apply(&err);
            skipped += 1;
            if let Some(event) = &ctx.on_event {
                event(StreamEvent::Skipped {
                    rule_id: entry.rule_id.clone(),
                    reason: reason.clone(),
                });
            }
            skip_tracker.record(reason, &entry.rule_id);
            continue;
        }

        let delete_opts = MoleDeleteOptions {
            mode: delete_mode,
            dry_run: false,
            needs_sudo: false,
            privilege: None,
        };

        match mole_delete_verified(
            &path,
            &identity,
            ctx.protection,
            ctx.whitelist_patterns,
            delete_opts,
            ctx.trash,
            ctx.deletion_log,
            ctx.oplog,
        ) {
            Ok(outcome) => {
                succeeded += 1;
                match delete_mode {
                    DeleteMode::Trash => trashed_bytes += outcome.bytes,
                    DeleteMode::Permanent => deleted_bytes += outcome.bytes,
                }
            }
            Err(MoleDeleteError::Whitelisted) => {
                skipped += 1;
                skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            }
            Err(MoleDeleteError::Rejected)
            | Err(MoleDeleteError::IdentityMismatch)
            | Err(MoleDeleteError::Vanished) => {
                skipped += 1;
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
            }
            Err(_) => {
                failed += 1;
            }
        }
    }

    let report = Report {
        succeeded,
        skipped,
        failed,
        skipped_by_reason: skip_tracker.into_summaries(),
        trashed_bytes,
        deleted_bytes,
        coverage_note: plan.coverage_note.clone(),
    };

    if let Some(event) = &ctx.on_event {
        event(StreamEvent::Done {
            report: report.clone(),
        });
    }

    Ok(report)
}

fn plan_is_expired(plan: &ProtoPlan, now: SystemTime) -> bool {
    let ttl = Duration::from_secs(plan.ttl_secs);
    plan.created_at
        .checked_add(ttl)
        .is_none_or(|expires| now > expires)
}

fn proto_identity(entry: &ProtoPlanEntry) -> PlanEntryIdentity {
    let mtime = entry
        .mtime
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64;
    PlanEntryIdentity {
        dev: entry.dev,
        ino: entry.ino,
        mtime,
    }
}

fn skip_reason_for_apply(err: &PlanApplyError) -> SkipReason {
    match err {
        PlanApplyError::Policy(ValidationError::EndpointSecurityCache) => SkipReason::TccDenied,
        PlanApplyError::Policy(ValidationError::ProtectedPath)
        | PlanApplyError::Policy(ValidationError::CriticalSystemPath)
        | PlanApplyError::Policy(ValidationError::SymlinkToCritical)
        | PlanApplyError::Policy(ValidationError::AncestorResolvesToCritical) => {
            SkipReason::NeedsPrivilege
        }
        _ => SkipReason::PathVanished,
    }
}

#[derive(Default)]
struct SkipTracker {
    entries: Vec<SkipSummary>,
}

impl SkipTracker {
    fn record(&mut self, reason: SkipReason, rule_id: &str) {
        if let Some(summary) = self.entries.iter_mut().find(|s| s.reason == reason) {
            summary.count += 1;
            if !summary.rule_ids.iter().any(|id| id == rule_id) {
                summary.rule_ids.push(rule_id.to_string());
            }
            return;
        }
        self.entries.push(SkipSummary {
            reason,
            count: 1,
            rule_ids: vec![rule_id.to_string()],
        });
    }

    fn into_summaries(self) -> Vec<SkipSummary> {
        self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::installer_plan::{build_installer_plan, InstallerPlanOptions};
    use crate::protection::AppProtection;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn rejects_non_installer_rule_ids() {
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "x".into(),
                path: PathBuf::from("/tmp"),
                label: "x".into(),
                size: 0,
                rule_id: "purge:node_modules".into(),
                skip_reason: None,
                dev: 0,
                ino: 0,
                mtime: UNIX_EPOCH,
            }],
            coverage_note: None,
        };
        let protection = AppProtection::new();
        let report = apply_installer_plan(
            &plan,
            &protection,
            InstallerApplyOptions { permanent: false },
            None,
        )
        .unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
    }

    #[test]
    fn apply_trashes_planned_dmg() {
        let _guard = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let downloads = home.join("Downloads");
        fs::create_dir_all(&downloads).unwrap();
        let dmg = downloads.join("App.dmg");
        fs::write(&dmg, b"installer-bytes").unwrap();

        let trash_dir = dir.path().join("trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);

        let protection = AppProtection::new();
        let roots = [downloads.clone()];
        let plan = build_installer_plan(
            &protection,
            &InstallerPlanOptions {
                home,
                ttl_secs: 900,
                scan_roots: Some(&roots),
                max_depth: 2,
                now: SystemTime::now(),
            },
        )
        .unwrap();
        assert!(
            !plan.entries.is_empty(),
            "expected installer candidates, got none"
        );

        let report = apply_installer_plan(
            &plan,
            &protection,
            InstallerApplyOptions { permanent: false },
            None,
        )
        .unwrap();
        assert!(report.succeeded >= 1, "report={report:?}");
        assert!(!dmg.exists(), "dmg should be trashed");

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
    }
}
