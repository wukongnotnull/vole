//! `optimize` apply：TTL + delete / action 分发。

use std::io::{self, IsTerminal, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use vole_sys::Trash;

use crate::delete::{
    mole_delete_verified, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions,
};
use crate::oplog::OperationLogger;
use crate::optimize::{
    apply_optimize_action, parse_optimize_rule_id, OptimizeActionError, OptimizeTaskKind,
};
use crate::privilege::{NoPrivilege, PrivilegeBackend, SudoNoninteractive};
use crate::protection::AppProtection;
use crate::safety::{
    verify_plan_entry_for_apply, PlanApplyError, PlanEntryIdentity, ValidationError,
};
use crate::vole_proto::{
    Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, Report, SkipReason, SkipSummary, StreamEvent,
    SCHEMA_VERSION,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OptimizeApplyError {
    #[error("plan expired; rescan with `vole optimize --plan`")]
    Expired,
    #[error("unsupported plan schema version {got} (expected {expected})")]
    UnsupportedSchema { expected: u32, got: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct OptimizeApplyOptions {
    pub permanent: bool,
}

pub struct OptimizeApplyContext<'a> {
    pub protection: &'a AppProtection,
    pub whitelist_patterns: &'a [String],
    pub options: OptimizeApplyOptions,
    pub trash: &'a dyn Trash,
    pub deletion_log: &'a DeletionLogger,
    pub oplog: &'a mut OperationLogger,
    pub on_event: Option<&'a dyn Fn(StreamEvent)>,
    pub now: SystemTime,
    /// 缺省 `None` → `NoPrivilege`；CLI / `apply_optimize_plan` 注入 `SudoNoninteractive`。
    pub privilege: Option<&'a dyn PrivilegeBackend>,
    /// 本轮 apply 是否已尝试过 `acquire_interactive`（至多一次）。
    pub privilege_acquire_attempted: bool,
    /// 同 session DNS flush 去重（对齐 Mole `MOLE_DNS_FLUSHED`）。
    pub dns_flushed: bool,
}

/// probe；失败则至多一次 `acquire_interactive` 后再 probe。
fn ensure_privilege_ready(
    ctx: &mut OptimizeApplyContext<'_>,
    backend: &dyn PrivilegeBackend,
) -> bool {
    if backend.probe_noninteractive() {
        return true;
    }
    if ctx.privilege_acquire_attempted {
        return false;
    }
    ctx.privilege_acquire_attempted = true;
    if io::stdin().is_terminal() {
        let _ = writeln!(io::stderr(), "正在请求管理员权限以执行系统优化…");
    }
    backend.acquire_interactive() && backend.probe_noninteractive()
}

fn needs_dns_privilege(task_id: &str) -> bool {
    matches!(task_id, "system_maintenance" | "network_optimization")
}

pub fn apply_optimize_plan(
    plan: &ProtoPlan,
    protection: &AppProtection,
    options: OptimizeApplyOptions,
    on_event: Option<&dyn Fn(StreamEvent)>,
) -> Result<Report, OptimizeApplyError> {
    let deletion_log = DeletionLogger::from_env();
    let mut oplog = OperationLogger::new("optimize");
    let _ = oplog.session_start();
    let sudo = SudoNoninteractive;
    let mut ctx = OptimizeApplyContext {
        protection,
        whitelist_patterns: &[],
        options,
        trash: &vole_sys::macos::MacTrash,
        deletion_log: &deletion_log,
        oplog: &mut oplog,
        on_event,
        now: SystemTime::now(),
        privilege: Some(&sudo),
        privilege_acquire_attempted: false,
        dns_flushed: false,
    };
    let report = apply_optimize_proto_plan(plan, &mut ctx)?;
    let _ = oplog.session_end(
        report.succeeded,
        report.trashed_bytes / 1024 + report.deleted_bytes / 1024,
    );
    Ok(report)
}

pub fn apply_optimize_proto_plan(
    plan: &ProtoPlan,
    ctx: &mut OptimizeApplyContext<'_>,
) -> Result<Report, OptimizeApplyError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(OptimizeApplyError::UnsupportedSchema {
            expected: SCHEMA_VERSION,
            got: plan.schema_version,
        });
    }
    if plan_is_expired(plan, ctx.now) {
        return Err(OptimizeApplyError::Expired);
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
            skip_tracker.record(reason, &entry.rule_id);
            continue;
        }

        let Some((kind, task_id)) = parse_optimize_rule_id(&entry.rule_id) else {
            skipped += 1;
            skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            continue;
        };

        match kind {
            OptimizeTaskKind::Delete => {
                let path = entry.path.display().to_string();
                let identity = proto_identity(entry);
                if let Err(err) = verify_plan_entry_for_apply(&path, &identity, ctx.protection) {
                    skipped += 1;
                    skip_tracker.record(skip_reason_for_apply(&err), &entry.rule_id);
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
                    Err(_) => failed += 1,
                }
            }
            OptimizeTaskKind::Action => {
                let fallback = NoPrivilege;
                let backend: &dyn PrivilegeBackend = ctx.privilege.unwrap_or(&fallback);
                if needs_dns_privilege(task_id) && !ensure_privilege_ready(ctx, backend) {
                    skipped += 1;
                    skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                    continue;
                }
                let privilege: Option<&dyn PrivilegeBackend> = Some(backend);
                match apply_optimize_action(task_id, &entry.path, privilege, &mut ctx.dns_flushed) {
                    Ok(()) => succeeded += 1,
                    Err(OptimizeActionError::NeedsPrivilege) => {
                        skipped += 1;
                        skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                    }
                    Err(OptimizeActionError::Skipped) => {
                        skipped += 1;
                        skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                    }
                    Err(OptimizeActionError::Failed) => failed += 1,
                }
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
    use crate::delete::DeletionLogger;
    use crate::oplog::OperationLogger;
    use crate::privilege::{NoPrivilege, RecordingPrivilege};
    use crate::protection::AppProtection;
    use crate::safety::capture_plan_entry_identity;
    use crate::test_env;
    use std::fs;
    use std::path::PathBuf;
    use vole_sys::macos::MacTrash;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vole-optimize-apply-{tag}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn dns_plan(home: &std::path::Path) -> ProtoPlan {
        let sys = home.join(".vole-optimize-action/system_maintenance");
        let net = home.join(".vole-optimize-action/network_optimization");
        ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![
                ProtoPlanEntry {
                    id: "system_maintenance-0".into(),
                    path: sys,
                    label: "DNS & Spotlight Check".into(),
                    size: 0,
                    rule_id: "optimize:action:system_maintenance".into(),
                    skip_reason: None,
                    dev: 0,
                    ino: 0,
                    mtime: UNIX_EPOCH,
                },
                ProtoPlanEntry {
                    id: "network_optimization-0".into(),
                    path: net,
                    label: "Network Cache Refresh".into(),
                    size: 0,
                    rule_id: "optimize:action:network_optimization".into(),
                    skip_reason: None,
                    dev: 0,
                    ino: 0,
                    mtime: UNIX_EPOCH,
                },
            ],
            coverage_note: None,
        }
    }

    #[test]
    fn apply_dns_tasks_skip_without_privilege() {
        let _guard = test_env::lock();
        let root = scratch("dns-deny");
        let plan = dns_plan(&root);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("optimize");
        let trash = MacTrash;
        let backend = NoPrivilege;
        let mut ctx = OptimizeApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: OptimizeApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            privilege: Some(&backend),
            privilege_acquire_attempted: false,
            dns_flushed: false,
        };
        let report = apply_optimize_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 2);
        assert!(report
            .skipped_by_reason
            .iter()
            .any(|s| s.reason == SkipReason::NeedsPrivilege && s.count == 2));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_dns_tasks_flush_once_with_recording() {
        let _guard = test_env::lock();
        let root = scratch("dns-ok");
        let plan = dns_plan(&root);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("optimize");
        let trash = MacTrash;
        let backend = RecordingPrivilege::allowing();
        let mut ctx = OptimizeApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: OptimizeApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            privilege: Some(&backend),
            privilege_acquire_attempted: false,
            dns_flushed: false,
        };
        let report = apply_optimize_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 2);
        assert_eq!(report.skipped, 0);
        assert_eq!(*backend.flush_dns_calls.lock().unwrap(), 1);
        assert!(ctx.dns_flushed);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_deletes_and_rejects_expired() {
        let _guard = test_env::lock();
        let root = scratch("ok");
        let file = root.join("victim.txt");
        fs::write(&file, b"hi").unwrap();
        let identity = capture_plan_entry_identity(&file).unwrap();
        let entry = ProtoPlanEntry {
            id: "1".into(),
            path: file.clone(),
            label: "victim".into(),
            size: 2,
            rule_id: "optimize:delete:cache_refresh".into(),
            skip_reason: None,
            dev: identity.dev,
            ino: identity.ino,
            mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![entry],
            coverage_note: None,
        };

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("optimize");
        let trash = MacTrash;
        let mut ctx = OptimizeApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: OptimizeApplyOptions { permanent: true },
            trash: &trash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            privilege: None,
            privilege_acquire_attempted: false,
            dns_flushed: false,
        };
        let report = apply_optimize_proto_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert!(!file.exists());

        let expired = ProtoPlan {
            created_at: UNIX_EPOCH,
            ttl_secs: 1,
            entries: vec![],
            ..plan
        };
        let err = apply_optimize_proto_plan(&expired, &mut ctx).unwrap_err();
        assert_eq!(err, OptimizeApplyError::Expired);
        fs::remove_dir_all(&root).ok();
    }
}
