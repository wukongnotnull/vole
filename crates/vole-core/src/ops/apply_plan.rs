//! plan apply 阶段：TTL 校验、TOCTOU 身份重验、`mole_delete` 执行。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use vole_sys::Trash;

use crate::delete::{mole_delete, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions};
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
pub enum ApplyPlanError {
    #[error("plan expired; rescan with `vole clean --plan`")]
    Expired,
    #[error("unsupported plan schema version {got} (expected {expected})")]
    UnsupportedSchema { expected: u32, got: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct ApplyPlanOptions {
    pub permanent: bool,
}

pub struct ApplyPlanContext<'a> {
    pub protection: &'a AppProtection,
    pub whitelist_patterns: &'a [String],
    pub options: ApplyPlanOptions,
    pub trash: &'a dyn Trash,
    pub deletion_log: &'a DeletionLogger,
    pub oplog: &'a mut OperationLogger,
    pub on_event: Option<&'a dyn Fn(StreamEvent)>,
    pub now: SystemTime,
}

impl<'a> ApplyPlanContext<'a> {
    pub fn new(
        protection: &'a AppProtection,
        whitelist_patterns: &'a [String],
        options: ApplyPlanOptions,
        trash: &'a dyn Trash,
        deletion_log: &'a DeletionLogger,
        oplog: &'a mut OperationLogger,
        on_event: Option<&'a dyn Fn(StreamEvent)>,
    ) -> Self {
        Self {
            protection,
            whitelist_patterns,
            options,
            trash,
            deletion_log,
            oplog,
            on_event,
            now: SystemTime::now(),
        }
    }
}

pub fn apply_proto_plan(
    plan: &ProtoPlan,
    protection: &AppProtection,
    whitelist_patterns: &[String],
    options: ApplyPlanOptions,
    on_event: Option<&dyn Fn(StreamEvent)>,
) -> Result<Report, ApplyPlanError> {
    let deletion_log = DeletionLogger::with_path(crate::delete::deletion_log_path());
    let mut oplog = OperationLogger::new("clean");
    let mut ctx = ApplyPlanContext::new(
        protection,
        whitelist_patterns,
        options,
        &vole_sys::macos::MacTrash,
        &deletion_log,
        &mut oplog,
        on_event,
    );
    apply_plan(plan, &mut ctx)
}

pub fn apply_plan(
    plan: &ProtoPlan,
    ctx: &mut ApplyPlanContext<'_>,
) -> Result<Report, ApplyPlanError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(ApplyPlanError::UnsupportedSchema {
            expected: SCHEMA_VERSION,
            got: plan.schema_version,
        });
    }
    if plan_is_expired(plan, ctx.now) {
        return Err(ApplyPlanError::Expired);
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

        let path = entry.path.display().to_string();
        let identity = proto_identity(entry);

        if let Err(err) = verify_plan_entry_for_apply(&path, &identity, ctx.protection) {
            skipped += 1;
            let reason = skip_reason_for_apply(&err);
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
        };

        match mole_delete(
            &path,
            ctx.protection,
            ctx.whitelist_patterns,
            delete_opts,
            ctx.trash,
            ctx.deletion_log,
            ctx.oplog,
        ) {
            Ok(()) => {
                succeeded += 1;
                match delete_mode {
                    DeleteMode::Trash => trashed_bytes += entry.size,
                    DeleteMode::Permanent => deleted_bytes += entry.size,
                }
            }
            Err(MoleDeleteError::Rejected) => {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
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
        PlanApplyError::Policy(_) | PlanApplyError::Identity(_) => SkipReason::PathVanished,
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
    use crate::safety::capture_plan_entry_identity;
    use crate::test_env;
    use std::fs;
    use std::path::PathBuf;
    use vole_sys::macos::MacTrash;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vole-apply-plan-{tag}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn plan_entry(path: &PathBuf, rule_id: &str) -> ProtoPlanEntry {
        let identity = capture_plan_entry_identity(path).unwrap();
        ProtoPlanEntry {
            id: format!("{rule_id}-0"),
            path: path.clone(),
            label: "test".into(),
            size: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            rule_id: rule_id.into(),
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
        }
    }

    fn fresh_plan(entries: Vec<ProtoPlanEntry>) -> ProtoPlan {
        ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries,
        }
    }

    fn apply_opts(permanent: bool) -> ApplyPlanOptions {
        ApplyPlanOptions { permanent }
    }

    fn run_apply(
        plan: &ProtoPlan,
        protection: &AppProtection,
        options: ApplyPlanOptions,
        deletion_log: &DeletionLogger,
        oplog: &mut OperationLogger,
        now: Option<SystemTime>,
    ) -> Result<Report, ApplyPlanError> {
        let mut ctx = ApplyPlanContext::new(
            protection,
            &[],
            options,
            &MacTrash,
            deletion_log,
            oplog,
            None,
        );
        if let Some(now) = now {
            ctx.now = now;
        }
        apply_plan(plan, &mut ctx)
    }

    #[test]
    fn ttl_expired_rejects() {
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: UNIX_EPOCH,
            ttl_secs: 60,
            entries: vec![],
        };
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(scratch("ttl-log").join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let now = UNIX_EPOCH + Duration::from_secs(120);

        let err = run_apply(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            Some(now),
        )
        .unwrap_err();

        assert_eq!(err, ApplyPlanError::Expired);
    }

    #[test]
    fn identity_mismatch_skips_entry() {
        let _guard = test_env::lock();
        let root = scratch("identity");
        let file = root.join("cache.db");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"v1").unwrap();
        let entry = plan_entry(&file, "rule-a");
        let plan = fresh_plan(vec![entry]);

        std::thread::sleep(Duration::from_secs(1));
        fs::write(&file, b"v2").unwrap();

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");

        let report = run_apply(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            None,
        )
        .unwrap();

        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(file.exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn trash_mode_moves_to_test_trash() {
        let _guard = test_env::lock();
        let root = scratch("trash-apply");
        let file = root.join("victim.txt");
        fs::write(&file, b"delete-me").unwrap();
        let trash_dir = root.join("Trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        std::env::set_var("MOLE_DELETE_LOG", root.join("deletions.log"));

        let plan = fresh_plan(vec![plan_entry(&file, "rule-trash")]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");

        let report = run_apply(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            None,
        )
        .unwrap();

        assert_eq!(report.succeeded, 1);
        assert_eq!(report.trashed_bytes, 9);
        assert_eq!(report.deleted_bytes, 0);
        assert!(!file.exists());
        assert!(fs::read_dir(&trash_dir).unwrap().next().is_some());

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn permanent_mode_deletes_file() {
        let _guard = test_env::lock();
        let root = scratch("permanent-apply");
        let file = root.join("victim.txt");
        fs::write(&file, b"gone").unwrap();
        let trash_dir = root.join("Trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        std::env::set_var("MOLE_DELETE_LOG", root.join("deletions.log"));

        let plan = fresh_plan(vec![plan_entry(&file, "rule-perm")]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");

        let report = run_apply(
            &plan,
            &protection,
            apply_opts(true),
            &deletion_log,
            &mut oplog,
            None,
        )
        .unwrap();

        assert_eq!(report.succeeded, 1);
        assert_eq!(report.trashed_bytes, 0);
        assert_eq!(report.deleted_bytes, 4);
        assert!(!file.exists());
        assert!(fs::read_dir(&trash_dir).unwrap().next().is_none());

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&root).ok();
    }
}
