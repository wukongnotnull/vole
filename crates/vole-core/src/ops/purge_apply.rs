//! `purge` apply：TTL + TOCTOU + Cleanup 保护 + `mole_delete_verified`。

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
pub enum PurgeApplyError {
    #[error("plan expired; rescan with `vole purge --plan`")]
    Expired,
    #[error("unsupported plan schema version {got} (expected {expected})")]
    UnsupportedSchema { expected: u32, got: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct PurgeApplyOptions {
    pub permanent: bool,
}

pub struct PurgeApplyContext<'a> {
    pub protection: &'a AppProtection,
    pub whitelist_patterns: &'a [String],
    pub options: PurgeApplyOptions,
    pub trash: &'a dyn Trash,
    pub deletion_log: &'a DeletionLogger,
    pub oplog: &'a mut OperationLogger,
    pub on_event: Option<&'a dyn Fn(StreamEvent)>,
    pub now: SystemTime,
}

pub fn apply_purge_plan(
    plan: &ProtoPlan,
    protection: &AppProtection,
    options: PurgeApplyOptions,
    on_event: Option<&dyn Fn(StreamEvent)>,
) -> Result<Report, PurgeApplyError> {
    let deletion_log = DeletionLogger::from_env();
    let mut oplog = OperationLogger::new("purge");
    let _ = oplog.session_start();
    let mut ctx = PurgeApplyContext {
        protection,
        whitelist_patterns: &[],
        options,
        trash: &vole_sys::macos::MacTrash,
        deletion_log: &deletion_log,
        oplog: &mut oplog,
        on_event,
        now: SystemTime::now(),
    };
    let report = apply_purge_proto_plan(plan, &mut ctx)?;
    let _ = oplog.session_end(
        report.succeeded,
        report.trashed_bytes / 1024 + report.deleted_bytes / 1024,
    );
    Ok(report)
}

pub fn apply_purge_proto_plan(
    plan: &ProtoPlan,
    ctx: &mut PurgeApplyContext<'_>,
) -> Result<Report, PurgeApplyError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(PurgeApplyError::UnsupportedSchema {
            expected: SCHEMA_VERSION,
            got: plan.schema_version,
        });
    }
    if plan_is_expired(plan, ctx.now) {
        return Err(PurgeApplyError::Expired);
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

        if !entry.rule_id.starts_with("purge:") {
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
    use crate::ops::purge_plan::{build_purge_plan, PurgePlanOptions};
    use crate::protection::AppProtection;
    use std::fs;
    use std::fs::FileTimes;
    use std::path::PathBuf;

    #[test]
    fn rejects_non_purge_rule_ids() {
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "x".into(),
                path: PathBuf::from("/tmp"),
                label: "x".into(),
                size: 0,
                rule_id: "uninstall:com.example".into(),
                skip_reason: None,
                dev: 0,
                ino: 0,
                mtime: UNIX_EPOCH,
                blockers: Vec::new(),
            }],
            coverage_note: None,
        };
        let protection = AppProtection::new();
        let report = apply_purge_plan(
            &plan,
            &protection,
            PurgeApplyOptions { permanent: false },
            None,
        )
        .unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
    }

    #[test]
    fn apply_trashes_planned_node_modules() {
        let _guard = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let project = home.join("Code/app");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("package.json"), b"{}").unwrap();
        let nm = project.join("node_modules");
        fs::create_dir_all(nm.join("pkg")).unwrap();
        fs::write(nm.join("pkg/i.js"), b"1").unwrap();
        let old = SystemTime::now() - Duration::from_secs(14 * 86_400);
        fs::File::open(&nm)
            .unwrap()
            .set_times(FileTimes::new().set_modified(old))
            .unwrap();

        let trash_dir = dir.path().join("trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);

        let protection = AppProtection::new();
        let roots = [home.join("Code")];
        let plan = build_purge_plan(
            &protection,
            &PurgePlanOptions {
                home,
                ttl_secs: 900,
                search_roots: Some(&roots),
                include_empty: false,
                min_age_days: 7,
                now: SystemTime::now(),
            },
        )
        .unwrap();
        assert!(
            !plan.entries.is_empty(),
            "expected purge candidates, got none"
        );

        let report = apply_purge_plan(
            &plan,
            &protection,
            PurgeApplyOptions { permanent: false },
            None,
        )
        .unwrap();
        assert!(report.succeeded >= 1, "report={report:?}");
        assert!(!nm.exists(), "node_modules should be trashed");

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
    }
}
