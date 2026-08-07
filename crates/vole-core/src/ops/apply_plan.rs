//! plan apply 阶段：TTL 校验、TOCTOU 身份重验、`mole_delete` 执行。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use vole_sys::Trash;

use crate::delete::{
    mole_delete_verified, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions,
};
use crate::handoff::{recheck_handoff_pasteboard_entry, HANDOFF_PASTEBOARD_RULE_ID};
use crate::oplog::OperationLogger;
use crate::orphan::{
    bundle_id_from_orphan_path, claude_vm_orphan_age_days_from_env, is_claude_vm_bundle_path,
    orphan_age_days_from_env, LiveOrphanDeps, OrphanDeps, OrphanJudge, ORPHANED_RULE_ID,
};
use crate::privilege::{
    is_arm64_host, path_allowed_for_privilege, NoPrivilege, PrivilegeBackend, SudoNoninteractive,
    ICON_SERVICES_SYSTEM_CACHE_RULE_ID, ROSETTA_CACHE_RULE_ID,
};
use crate::protection::AppProtection;
use crate::rules::{should_skip_for_guards, ProcessProbe, Rule};
use crate::safety::{
    is_icon_services_system_cache, is_rosetta_update_bundle, verify_plan_entry,
    verify_plan_entry_for_apply, PlanApplyError, PlanEntryIdentity, ValidationError,
};
use crate::stubs::{
    recheck_container_stub_entry, remove_verified_container_stub, CONTAINER_STUB_RULE_ID,
};
use crate::sysorphan::{recheck_system_service_entry, SYSTEM_SERVICES_RULE_ID};
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
    pub rules: &'a [Rule],
    pub process_probe: &'a dyn ProcessProbe,
    pub orphan_deps: &'a dyn OrphanDeps,
    pub on_event: Option<&'a dyn Fn(StreamEvent)>,
    pub now: SystemTime,
    /// 缺省 `None` → `NoPrivilege`；CLI apply 注入 `SudoNoninteractive`。
    pub privilege: Option<&'a dyn PrivilegeBackend>,
}

impl<'a> ApplyPlanContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        protection: &'a AppProtection,
        whitelist_patterns: &'a [String],
        options: ApplyPlanOptions,
        trash: &'a dyn Trash,
        deletion_log: &'a DeletionLogger,
        oplog: &'a mut OperationLogger,
        rules: &'a [Rule],
        process_probe: &'a dyn ProcessProbe,
        orphan_deps: &'a dyn OrphanDeps,
        on_event: Option<&'a dyn Fn(StreamEvent)>,
    ) -> Self {
        Self {
            protection,
            whitelist_patterns,
            options,
            trash,
            deletion_log,
            oplog,
            rules,
            process_probe,
            orphan_deps,
            on_event,
            now: SystemTime::now(),
            privilege: None,
        }
    }
}

pub fn apply_proto_plan(
    plan: &ProtoPlan,
    protection: &AppProtection,
    whitelist_patterns: &[String],
    options: ApplyPlanOptions,
    rules: &[Rule],
    process_probe: &dyn ProcessProbe,
    on_event: Option<&dyn Fn(StreamEvent)>,
) -> Result<Report, ApplyPlanError> {
    let deletion_log = DeletionLogger::with_path(crate::delete::deletion_log_path());
    let mut oplog = OperationLogger::new("clean");
    let orphan_deps = LiveOrphanDeps::new();
    let sudo = SudoNoninteractive;
    let mut ctx = ApplyPlanContext::new(
        protection,
        whitelist_patterns,
        options,
        &vole_sys::macos::MacTrash,
        &deletion_log,
        &mut oplog,
        rules,
        process_probe,
        &orphan_deps,
        on_event,
    );
    ctx.privilege = Some(&sudo);
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

        if let Some(rule) = ctx.rules.iter().find(|r| r.id == entry.rule_id) {
            if should_skip_for_guards(ctx.process_probe, &rule.guards) {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::AppRunning,
                    });
                }
                skip_tracker.record(SkipReason::AppRunning, &entry.rule_id);
                continue;
            }
        }

        if entry.rule_id == ORPHANED_RULE_ID
            && !recheck_orphaned_entry(entry, ctx.orphan_deps, ctx.protection, ctx.now)
        {
            skipped += 1;
            if let Some(event) = &ctx.on_event {
                event(StreamEvent::Skipped {
                    rule_id: entry.rule_id.clone(),
                    reason: SkipReason::PathVanished,
                });
            }
            skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
            continue;
        }

        // 政策重验（非 protect 豁免）：根形状 + mtime>60min，防过期/篡改 plan。
        if entry.rule_id == HANDOFF_PASTEBOARD_RULE_ID {
            let home = dirs_home();
            if !recheck_handoff_pasteboard_entry(&entry.path, &home, ctx.now) {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
        }

        // system-services：allowlist → probe → unload 尽力而为 → sudo permanent。
        if entry.rule_id == SYSTEM_SERVICES_RULE_ID {
            let fallback = NoPrivilege;
            let backend = ctx.privilege.unwrap_or(&fallback);
            if !path_allowed_for_privilege(&entry.path) {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            let home = dirs_home();
            if !recheck_system_service_entry(&entry.path, &home, ctx.orphan_deps) {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            if !backend.probe_noninteractive() {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::NeedsPrivilege,
                    });
                }
                skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                continue;
            }
            let path = entry.path.display().to_string();
            let identity = proto_identity(entry);
            if verify_plan_entry(&path, &identity).is_err() {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            if entry.path.extension().and_then(|e| e.to_str()) == Some("plist") {
                let _ = backend.launchctl_unload(&entry.path);
            }
            let delete_opts = MoleDeleteOptions {
                mode: DeleteMode::Permanent,
                dry_run: false,
                needs_sudo: true,
                privilege: Some(backend),
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
                    deleted_bytes += outcome.bytes;
                }
                Err(MoleDeleteError::SudoUnavailable)
                | Err(MoleDeleteError::SudoBlockedTestMode) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::NeedsPrivilege,
                        });
                    }
                    skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
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
            continue;
        }

        // Rosetta update bundle：arm64 → allowlist → probe → sudo permanent（无 unload）。
        if entry.rule_id == ROSETTA_CACHE_RULE_ID {
            let fallback = NoPrivilege;
            let backend = ctx.privilege.unwrap_or(&fallback);
            if !is_arm64_host()
                || !entry.path.to_str().is_some_and(is_rosetta_update_bundle)
                || !path_allowed_for_privilege(&entry.path)
            {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            if !backend.probe_noninteractive() {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::NeedsPrivilege,
                    });
                }
                skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                continue;
            }
            let path = entry.path.display().to_string();
            let identity = proto_identity(entry);
            if verify_plan_entry(&path, &identity).is_err() {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            let delete_opts = MoleDeleteOptions {
                mode: DeleteMode::Permanent,
                dry_run: false,
                needs_sudo: true,
                privilege: Some(backend),
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
                    deleted_bytes += outcome.bytes;
                }
                Err(MoleDeleteError::SudoUnavailable)
                | Err(MoleDeleteError::SudoBlockedTestMode) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::NeedsPrivilege,
                        });
                    }
                    skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
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
            continue;
        }

        // Icon Services 系统缓存：exact → allowlist → probe → sudo permanent（无 unload）。
        if entry.rule_id == ICON_SERVICES_SYSTEM_CACHE_RULE_ID {
            let fallback = NoPrivilege;
            let backend = ctx.privilege.unwrap_or(&fallback);
            if !entry
                .path
                .to_str()
                .is_some_and(is_icon_services_system_cache)
                || !path_allowed_for_privilege(&entry.path)
            {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            if !backend.probe_noninteractive() {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::NeedsPrivilege,
                    });
                }
                skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
                continue;
            }
            let path = entry.path.display().to_string();
            let identity = proto_identity(entry);
            if verify_plan_entry(&path, &identity).is_err() {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            let delete_opts = MoleDeleteOptions {
                mode: DeleteMode::Permanent,
                dry_run: false,
                needs_sudo: true,
                privilege: Some(backend),
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
                    deleted_bytes += outcome.bytes;
                }
                Err(MoleDeleteError::SudoUnavailable)
                | Err(MoleDeleteError::SudoBlockedTestMode) => {
                    skipped += 1;
                    if let Some(event) = &ctx.on_event {
                        event(StreamEvent::Skipped {
                            rule_id: entry.rule_id.clone(),
                            reason: SkipReason::NeedsPrivilege,
                        });
                    }
                    skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
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
            continue;
        }

        let path = entry.path.display().to_string();
        let identity = proto_identity(entry);

        // Carve-out（设计 §6）：container stub 不走 mole_delete_verified /
        // verify_plan_entry_for_apply（后者内含 validate_path_for_deletion，
        // com.macpaw.* data_protected 会再次拒绝）。对不可信 plan 先做策略
        // 重验（形状 + allowlist + app 存在），再做身份 TOCTOU + stub 删除。
        if entry.rule_id == CONTAINER_STUB_RULE_ID {
            let home = dirs_home();
            let ok = recheck_container_stub_entry(&entry.path, &home, ctx.orphan_deps)
                && verify_plan_entry(&path, &identity).is_ok()
                && remove_verified_container_stub(&entry.path).is_ok();
            if ok {
                succeeded += 1;
                ctx.deletion_log.log("stub-rmdir", "0", "ok", &path);
                ctx.oplog
                    .log("REMOVED", &entry.path, Some("stub-container"))
                    .ok();
            } else {
                skipped += 1;
                ctx.deletion_log.log("stub-rmdir", "0", "skipped", &path);
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::PathVanished,
                    });
                }
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
            }
            continue;
        }

        // 策略闸口先过一遍，再进入带身份重验的删除路径。
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
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::Whitelisted,
                    });
                }
                skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            }
            Err(MoleDeleteError::Rejected) => {
                skipped += 1;
                if let Some(event) = &ctx.on_event {
                    event(StreamEvent::Skipped {
                        rule_id: entry.rule_id.clone(),
                        reason: SkipReason::NeedsPrivilege,
                    });
                }
                skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
            }
            Err(MoleDeleteError::Vanished) | Err(MoleDeleteError::IdentityMismatch) => {
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
        coverage_note: None,
    };

    if let Some(event) = &ctx.on_event {
        event(StreamEvent::Done {
            report: report.clone(),
        });
    }

    Ok(report)
}

/// apply 时重新跑完整 orphan judge；失败则不得删除。
fn recheck_orphaned_entry(
    entry: &ProtoPlanEntry,
    deps: &dyn OrphanDeps,
    protection: &AppProtection,
    now: SystemTime,
) -> bool {
    let home = dirs_home();
    let Ok(installed) = deps.scan_installed_bundle_ids(home.as_path()) else {
        return false;
    };
    let judge = OrphanJudge {
        catalog: protection.catalog(),
        deps,
        installed: &installed,
        age_days: orphan_age_days_from_env(),
        now,
    };
    if is_claude_vm_bundle_path(&entry.path, home.as_path()) {
        return judge.is_claude_vm_bundle_orphaned(
            &entry.path,
            entry.mtime,
            claude_vm_orphan_age_days_from_env(),
        );
    }
    let Some(bundle_id) = bundle_id_from_orphan_path(&entry.path) else {
        return false;
    };
    judge.is_bundle_orphaned(&bundle_id, &entry.path, entry.mtime)
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/"))
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
            coverage_note: None,
            entries,
        }
    }

    fn apply_opts(permanent: bool) -> ApplyPlanOptions {
        ApplyPlanOptions { permanent }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_apply(
        plan: &ProtoPlan,
        protection: &AppProtection,
        options: ApplyPlanOptions,
        deletion_log: &DeletionLogger,
        oplog: &mut OperationLogger,
        now: Option<SystemTime>,
        rules: &[crate::rules::Rule],
        process_probe: &dyn crate::rules::ProcessProbe,
        trash: &dyn Trash,
    ) -> Result<Report, ApplyPlanError> {
        let orphan_deps = LiveOrphanDeps::new();
        let mut ctx = ApplyPlanContext::new(
            protection,
            &[],
            options,
            trash,
            deletion_log,
            oplog,
            rules,
            process_probe,
            &orphan_deps,
            None,
        );
        if let Some(now) = now {
            ctx.now = now;
        }
        apply_plan(plan, &mut ctx)
    }

    fn run_apply_with_orphan_deps(
        plan: &ProtoPlan,
        protection: &AppProtection,
        options: ApplyPlanOptions,
        deletion_log: &DeletionLogger,
        oplog: &mut OperationLogger,
        orphan_deps: &dyn OrphanDeps,
        trash: &dyn Trash,
    ) -> Result<Report, ApplyPlanError> {
        let probe = crate::rules::FakeProcessProbe::default();
        let mut ctx = ApplyPlanContext::new(
            protection,
            &[],
            options,
            trash,
            deletion_log,
            oplog,
            &[],
            &probe,
            orphan_deps,
            None,
        );
        apply_plan(plan, &mut ctx)
    }

    fn run_apply_defaults(
        plan: &ProtoPlan,
        protection: &AppProtection,
        options: ApplyPlanOptions,
        deletion_log: &DeletionLogger,
        oplog: &mut OperationLogger,
        now: Option<SystemTime>,
    ) -> Result<Report, ApplyPlanError> {
        let probe = crate::rules::FakeProcessProbe::default();
        run_apply(
            plan,
            protection,
            options,
            deletion_log,
            oplog,
            now,
            &[],
            &probe,
            &MacTrash,
        )
    }

    #[test]
    fn ttl_expired_rejects() {
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: UNIX_EPOCH,
            ttl_secs: 60,
            coverage_note: None,
            entries: vec![],
        };
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(scratch("ttl-log").join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let now = UNIX_EPOCH + Duration::from_secs(120);

        let err = run_apply_defaults(
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

        let report = run_apply_defaults(
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
    fn apply_system_services_skips_when_probe_fails() {
        use crate::orphan::FakeOrphanDeps;
        use crate::sysorphan::SYSTEM_SERVICES_RULE_ID;
        use std::collections::HashSet;

        let _guard = test_env::lock();
        let root = scratch("syssvc-noprobe");
        let lib = root.join("Library");
        for d in ["LaunchDaemons", "LaunchAgents", "PrivilegedHelperTools"] {
            fs::create_dir_all(lib.join(d)).unwrap();
        }
        let missing = root.join("nowhere/bin/gone");
        let plist = lib.join("LaunchDaemons/com.example.orphan.plist");
        fs::write(
            &plist,
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Program</key><string>{}</string></dict></plist>
"#,
                missing.display()
            ),
        )
        .unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let plan = fresh_plan(vec![plan_entry(&plist, SYSTEM_SERVICES_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let probe = crate::rules::FakeProcessProbe::default();
        let orphan_deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        let backend = crate::privilege::NoPrivilege;
        let mut ctx = ApplyPlanContext::new(
            &protection,
            &[],
            apply_opts(false),
            &MacTrash,
            &deletion_log,
            &mut oplog,
            &[],
            &probe,
            &orphan_deps,
            None,
        );
        ctx.privilege = Some(&backend);

        let report = apply_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(report
            .skipped_by_reason
            .iter()
            .any(|s| s.reason == SkipReason::NeedsPrivilege));
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_system_services_records_remove_when_probe_ok() {
        use crate::orphan::FakeOrphanDeps;
        use crate::sysorphan::SYSTEM_SERVICES_RULE_ID;
        use std::collections::HashSet;

        let _guard = test_env::lock();
        let root = scratch("syssvc-probe-ok");
        let lib = root.join("Library");
        for d in ["LaunchDaemons", "LaunchAgents", "PrivilegedHelperTools"] {
            fs::create_dir_all(lib.join(d)).unwrap();
        }
        let missing = root.join("nowhere/bin/gone");
        let plist = lib.join("LaunchDaemons/com.example.orphan.plist");
        fs::write(
            &plist,
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Program</key><string>{}</string></dict></plist>
"#,
                missing.display()
            ),
        )
        .unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let plan = fresh_plan(vec![plan_entry(&plist, SYSTEM_SERVICES_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let probe = crate::rules::FakeProcessProbe::default();
        let orphan_deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        let backend = crate::privilege::RecordingPrivilege::allowing();
        let mut ctx = ApplyPlanContext::new(
            &protection,
            &[],
            apply_opts(false),
            &MacTrash,
            &deletion_log,
            &mut oplog,
            &[],
            &probe,
            &orphan_deps,
            None,
        );
        ctx.privilege = Some(&backend);

        let report = apply_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(backend.removed.lock().unwrap().len(), 1);
        assert!(!plist.exists());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_rosetta_skips_when_probe_fails() {
        use crate::orphan::FakeOrphanDeps;
        use crate::privilege::ROSETTA_CACHE_RULE_ID;
        use std::collections::HashSet;

        let _guard = test_env::lock();
        let root = scratch("rosetta-noprobe");
        let lib = root.join("Library");
        let bundle = lib.join("Apple/usr/share/rosetta/rosetta_update_bundle");
        fs::create_dir_all(bundle.parent().unwrap()).unwrap();
        fs::write(&bundle, b"bundle").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);
        std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "arm64");

        let plan = fresh_plan(vec![plan_entry(&bundle, ROSETTA_CACHE_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let probe = crate::rules::FakeProcessProbe::default();
        let orphan_deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        let backend = crate::privilege::NoPrivilege;
        let mut ctx = ApplyPlanContext::new(
            &protection,
            &[],
            apply_opts(false),
            &MacTrash,
            &deletion_log,
            &mut oplog,
            &[],
            &probe,
            &orphan_deps,
            None,
        );
        ctx.privilege = Some(&backend);

        let report = apply_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(report
            .skipped_by_reason
            .iter()
            .any(|s| s.reason == SkipReason::NeedsPrivilege));
        assert!(bundle.exists());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        std::env::remove_var("VOLE_TEST_FORCE_UNAME_M");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_rosetta_records_remove_when_probe_ok() {
        use crate::orphan::FakeOrphanDeps;
        use crate::privilege::ROSETTA_CACHE_RULE_ID;
        use std::collections::HashSet;

        let _guard = test_env::lock();
        let root = scratch("rosetta-probe-ok");
        let lib = root.join("Library");
        let bundle = lib.join("Apple/usr/share/rosetta/rosetta_update_bundle");
        fs::create_dir_all(bundle.parent().unwrap()).unwrap();
        fs::write(&bundle, b"bundle").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);
        std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "arm64");

        let plan = fresh_plan(vec![plan_entry(&bundle, ROSETTA_CACHE_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let probe = crate::rules::FakeProcessProbe::default();
        let orphan_deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        let backend = crate::privilege::RecordingPrivilege::allowing();
        let mut ctx = ApplyPlanContext::new(
            &protection,
            &[],
            apply_opts(false),
            &MacTrash,
            &deletion_log,
            &mut oplog,
            &[],
            &probe,
            &orphan_deps,
            None,
        );
        ctx.privilege = Some(&backend);

        let report = apply_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(backend.removed.lock().unwrap().len(), 1);
        assert!(!bundle.exists());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        std::env::remove_var("VOLE_TEST_FORCE_UNAME_M");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_rosetta_skips_on_x86_host() {
        use crate::orphan::FakeOrphanDeps;
        use crate::privilege::ROSETTA_CACHE_RULE_ID;
        use std::collections::HashSet;

        let _guard = test_env::lock();
        let root = scratch("rosetta-x86");
        let lib = root.join("Library");
        let bundle = lib.join("Apple/usr/share/rosetta/rosetta_update_bundle");
        fs::create_dir_all(bundle.parent().unwrap()).unwrap();
        fs::write(&bundle, b"bundle").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);
        std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "x86_64");

        let plan = fresh_plan(vec![plan_entry(&bundle, ROSETTA_CACHE_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let probe = crate::rules::FakeProcessProbe::default();
        let orphan_deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        let backend = crate::privilege::RecordingPrivilege::allowing();
        let mut ctx = ApplyPlanContext::new(
            &protection,
            &[],
            apply_opts(false),
            &MacTrash,
            &deletion_log,
            &mut oplog,
            &[],
            &probe,
            &orphan_deps,
            None,
        );
        ctx.privilege = Some(&backend);

        let report = apply_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(bundle.exists());
        assert!(backend.removed.lock().unwrap().is_empty());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        std::env::remove_var("VOLE_TEST_FORCE_UNAME_M");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_rosetta_rejects_three_tree_path_with_rosetta_rule_id() {
        use crate::orphan::FakeOrphanDeps;
        use crate::privilege::ROSETTA_CACHE_RULE_ID;
        use std::collections::HashSet;

        let _guard = test_env::lock();
        let root = scratch("rosetta-wrong-tree");
        let lib = root.join("Library");
        for d in ["LaunchDaemons", "LaunchAgents", "PrivilegedHelperTools"] {
            fs::create_dir_all(lib.join(d)).unwrap();
        }
        let plist = lib.join("LaunchDaemons/com.example.notorphan.plist");
        fs::write(&plist, b"plist").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);
        std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "arm64");

        // 篡改式 plan：rule_id=rosetta，path=三树叶 → 必须 skip，不得 sudo 删。
        let plan = fresh_plan(vec![plan_entry(&plist, ROSETTA_CACHE_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let probe = crate::rules::FakeProcessProbe::default();
        let orphan_deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        let backend = crate::privilege::RecordingPrivilege::allowing();
        let mut ctx = ApplyPlanContext::new(
            &protection,
            &[],
            apply_opts(false),
            &MacTrash,
            &deletion_log,
            &mut oplog,
            &[],
            &probe,
            &orphan_deps,
            None,
        );
        ctx.privilege = Some(&backend);

        let report = apply_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(plist.exists());
        assert!(backend.removed.lock().unwrap().is_empty());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        std::env::remove_var("VOLE_TEST_FORCE_UNAME_M");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_icon_services_system_records_remove_when_probe_ok() {
        use crate::orphan::FakeOrphanDeps;
        use crate::privilege::ICON_SERVICES_SYSTEM_CACHE_RULE_ID;
        use std::collections::HashSet;

        let _guard = test_env::lock();
        let root = scratch("icon-sys-ok");
        let lib = root.join("Library");
        let store = lib.join("Caches/com.apple.iconservices.store");
        fs::create_dir_all(store.parent().unwrap()).unwrap();
        fs::write(&store, b"icons").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let plan = fresh_plan(vec![plan_entry(&store, ICON_SERVICES_SYSTEM_CACHE_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let probe = crate::rules::FakeProcessProbe::default();
        let orphan_deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        let backend = crate::privilege::RecordingPrivilege::allowing();
        let mut ctx = ApplyPlanContext::new(
            &protection,
            &[],
            apply_opts(false),
            &MacTrash,
            &deletion_log,
            &mut oplog,
            &[],
            &probe,
            &orphan_deps,
            None,
        );
        ctx.privilege = Some(&backend);

        let report = apply_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 1);
        assert!(!store.exists());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_icon_services_system_rejects_three_tree_path() {
        use crate::orphan::FakeOrphanDeps;
        use crate::privilege::ICON_SERVICES_SYSTEM_CACHE_RULE_ID;
        use std::collections::HashSet;

        let _guard = test_env::lock();
        let root = scratch("icon-sys-wrong");
        let lib = root.join("Library");
        for d in ["LaunchDaemons", "LaunchAgents", "PrivilegedHelperTools"] {
            fs::create_dir_all(lib.join(d)).unwrap();
        }
        let plist = lib.join("LaunchDaemons/com.example.x.plist");
        fs::write(&plist, b"p").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let plan = fresh_plan(vec![plan_entry(&plist, ICON_SERVICES_SYSTEM_CACHE_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let probe = crate::rules::FakeProcessProbe::default();
        let orphan_deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            ..Default::default()
        };
        let backend = crate::privilege::RecordingPrivilege::allowing();
        let mut ctx = ApplyPlanContext::new(
            &protection,
            &[],
            apply_opts(false),
            &MacTrash,
            &deletion_log,
            &mut oplog,
            &[],
            &probe,
            &orphan_deps,
            None,
        );
        ctx.privilege = Some(&backend);

        let report = apply_plan(&plan, &mut ctx).unwrap();
        assert_eq!(report.succeeded, 0);
        assert!(plist.exists());
        assert!(backend.removed.lock().unwrap().is_empty());
        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_container_stub_carve_out_removes_without_trash() {
        use crate::orphan::FakeOrphanDeps;
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }
        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }
        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                Ok(())
            }
        }

        let _guard = test_env::lock();
        let home = scratch("stub-carveout");
        // com.macpaw.* 是 data_protected：走共享删除管线必被拒，
        // 本测试证明 carve-out 真删且 trash 未被调用。
        let stub = home.join("Library/Containers/com.macpaw.CleanMyMac4");
        fs::create_dir_all(&stub).unwrap();
        fs::write(
            stub.join(".com.apple.containermanagerd.metadata.plist"),
            b"p",
        )
        .unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&stub, CONTAINER_STUB_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let fake_trash = FakeTrash::default();
        let deps = FakeOrphanDeps {
            spotlight: true,
            ..Default::default()
        };

        // --permanent 与默认行为一致：都是 carve-out（unlink + rmdir）。
        let report = run_apply_with_orphan_deps(
            &plan,
            &protection,
            apply_opts(true),
            &deletion_log,
            &mut oplog,
            &deps,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 1);
        assert_eq!(report.skipped, 0);
        assert!(!stub.exists());
        assert!(
            fake_trash.calls.lock().unwrap().is_empty(),
            "carve-out must not touch trash"
        );
        assert_eq!(report.trashed_bytes, 0);
        assert_eq!(report.deleted_bytes, 0);
        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_container_stub_skips_when_content_appears_after_plan() {
        use crate::orphan::FakeOrphanDeps;
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }
        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }
        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                Ok(())
            }
        }

        let _guard = test_env::lock();
        let home = scratch("stub-toctou");
        let stub = home.join("Library/Containers/com.macpaw.CleanMyMac4");
        fs::create_dir_all(&stub).unwrap();
        fs::write(
            stub.join(".com.apple.containermanagerd.metadata.plist"),
            b"p",
        )
        .unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&stub, CONTAINER_STUB_RULE_ID)]);
        // plan 之后长出用户数据 → apply 重验必须拒绝并保留。
        fs::create_dir_all(stub.join("Data")).unwrap();

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let fake_trash = FakeTrash::default();
        let deps = FakeOrphanDeps {
            spotlight: true,
            ..Default::default()
        };

        let report = run_apply_with_orphan_deps(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            &deps,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(stub
            .join(".com.apple.containermanagerd.metadata.plist")
            .exists());
        assert!(stub.join("Data").exists());
        assert!(fake_trash.calls.lock().unwrap().is_empty());
        assert!(report
            .skipped_by_reason
            .iter()
            .any(|s| s.reason == SkipReason::PathVanished));
        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_container_stub_recheck_rejects_path_outside_containers() {
        use crate::orphan::FakeOrphanDeps;
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }
        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }
        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                Ok(())
            }
        }

        let _guard = test_env::lock();
        let home = scratch("stub-outside");
        // 篡改 plan：path 在 Containers 外但凑成 stub 形。
        let outside = home.join("Library/Preferences/com.macpaw.CleanMyMac4");
        fs::create_dir_all(&outside).unwrap();
        fs::write(
            outside.join(".com.apple.containermanagerd.metadata.plist"),
            b"p",
        )
        .unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&outside, CONTAINER_STUB_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let fake_trash = FakeTrash::default();
        let deps = FakeOrphanDeps {
            spotlight: true,
            ..Default::default()
        };

        let report = run_apply_with_orphan_deps(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            &deps,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(outside.exists());
        assert!(fake_trash.calls.lock().unwrap().is_empty());
        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_container_stub_recheck_rejects_non_allowlist_bundle() {
        use crate::orphan::FakeOrphanDeps;
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }
        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }
        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                Ok(())
            }
        }

        let _guard = test_env::lock();
        let home = scratch("stub-allowlist");
        let stub = home.join("Library/Containers/com.example.app");
        fs::create_dir_all(&stub).unwrap();
        fs::write(
            stub.join(".com.apple.containermanagerd.metadata.plist"),
            b"p",
        )
        .unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&stub, CONTAINER_STUB_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let fake_trash = FakeTrash::default();
        let deps = FakeOrphanDeps {
            spotlight: true,
            ..Default::default()
        };

        let report = run_apply_with_orphan_deps(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            &deps,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(stub.exists());
        assert!(fake_trash.calls.lock().unwrap().is_empty());
        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_container_stub_recheck_rejects_when_app_reinstalled() {
        use crate::orphan::FakeOrphanDeps;
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }
        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }
        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                Ok(())
            }
        }

        let _guard = test_env::lock();
        let home = scratch("stub-reinstall");
        let stub = home.join("Library/Containers/com.macpaw.CleanMyMac4");
        fs::create_dir_all(&stub).unwrap();
        fs::write(
            stub.join(".com.apple.containermanagerd.metadata.plist"),
            b"p",
        )
        .unwrap();
        // plan 后 app 被重装到 ~/Applications。
        fs::create_dir_all(home.join("Applications/CleanMyMac X.app")).unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&stub, CONTAINER_STUB_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let fake_trash = FakeTrash::default();
        let deps = FakeOrphanDeps {
            spotlight: true,
            ..Default::default()
        };

        let report = run_apply_with_orphan_deps(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            &deps,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(stub.exists());
        assert!(fake_trash.calls.lock().unwrap().is_empty());
        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
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

        let report = run_apply_defaults(
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
    fn apply_group_container_cache_leaf_via_normal_delete() {
        use crate::groupcaches::GROUP_CONTAINER_CACHE_RULE_ID;

        let _guard = test_env::lock();
        let home = scratch("gcc-apply");
        let leaf = home.join("Library/Group Containers/group.com.example.app/Library/Caches/c1");
        fs::create_dir_all(leaf.parent().unwrap()).unwrap();
        fs::write(&leaf, b"cache").unwrap();
        std::env::set_var("HOME", &home);

        let trash_dir = home.join("Trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        std::env::set_var("MOLE_DELETE_LOG", home.join("deletions.log"));

        let plan = fresh_plan(vec![plan_entry(&leaf, GROUP_CONTAINER_CACHE_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");

        let report = run_apply_defaults(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            None,
        )
        .unwrap();

        assert_eq!(report.succeeded, 1, "expected normal mole_delete path");
        assert!(!leaf.exists());
        assert!(fs::read_dir(&trash_dir).unwrap().next().is_some());
        // 确认无 carve-out 早分支：规则仍走废纸篓
        assert_eq!(report.trashed_bytes, 5);

        std::env::remove_var("HOME");
        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_handoff_old_leaf_trashes() {
        use crate::handoff::HANDOFF_PASTEBOARD_RULE_ID;

        let _guard = test_env::lock();
        let home = scratch("handoff-apply");
        let root = home.join(
            "Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard",
        );
        fs::create_dir_all(&root).unwrap();
        let leaf = root.join("old");
        fs::write(&leaf, b"clip").unwrap();
        let ancient = SystemTime::now() - Duration::from_secs(2 * 3600);
        filetime::set_file_mtime(&leaf, filetime::FileTime::from_system_time(ancient)).unwrap();
        std::env::set_var("HOME", &home);

        let trash_dir = home.join("Trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        std::env::set_var("MOLE_DELETE_LOG", home.join("deletions.log"));

        let plan = fresh_plan(vec![plan_entry(&leaf, HANDOFF_PASTEBOARD_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");

        let report = run_apply_defaults(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            None,
        )
        .unwrap();

        assert_eq!(report.succeeded, 1);
        assert!(!leaf.exists());
        assert_eq!(report.trashed_bytes, 4);

        std::env::remove_var("HOME");
        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_handoff_fresh_mtime_skips() {
        use crate::handoff::HANDOFF_PASTEBOARD_RULE_ID;

        let _guard = test_env::lock();
        let home = scratch("handoff-fresh-apply");
        let root = home.join(
            "Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard",
        );
        fs::create_dir_all(&root).unwrap();
        let leaf = root.join("was-old");
        fs::write(&leaf, b"clip").unwrap();
        let ancient = SystemTime::now() - Duration::from_secs(2 * 3600);
        filetime::set_file_mtime(&leaf, filetime::FileTime::from_system_time(ancient)).unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&leaf, HANDOFF_PASTEBOARD_RULE_ID)]);
        // apply 前变「新鲜」
        filetime::set_file_mtime(
            &leaf,
            filetime::FileTime::from_system_time(SystemTime::now()),
        )
        .unwrap();

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let report = run_apply_defaults(
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
        assert!(leaf.exists());

        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_handoff_outside_root_skips() {
        use crate::handoff::HANDOFF_PASTEBOARD_RULE_ID;

        let _guard = test_env::lock();
        let home = scratch("handoff-outside");
        let outside =
            home.join("Library/Group Containers/group.com.apple.coreservices.useractivityd/other");
        fs::create_dir_all(outside.parent().unwrap()).unwrap();
        fs::write(&outside, b"nope").unwrap();
        let ancient = SystemTime::now() - Duration::from_secs(2 * 3600);
        filetime::set_file_mtime(&outside, filetime::FileTime::from_system_time(ancient)).unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&outside, HANDOFF_PASTEBOARD_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let report = run_apply_defaults(
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
        assert!(outside.exists());

        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
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

        let report = run_apply_defaults(
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

    #[test]
    fn apply_skips_when_not_running_guard_hits_at_apply() {
        use crate::rules::{FakeProcessProbe, Rule, StrategyConfig};
        use std::collections::HashSet;
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }

        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }

        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                Ok(())
            }
        }

        fn all_rule(id: &str) -> Rule {
            Rule {
                id: id.into(),
                category: None,
                label: format!("label-{id}"),
                platform: vec![],
                paths: vec![],
                impact: None,
                disabled: false,
                last_verified: None,
                strategy: StrategyConfig::default(),
                guards: Default::default(),
            }
        }

        let _guard = test_env::lock();
        let root = scratch("apply-not-running");
        let file = root.join("victim.txt");
        fs::write(&file, b"keep-me").unwrap();

        let rule_id = "firefox-cache";
        let mut rule = all_rule(rule_id);
        rule.guards.not_running = vec!["Firefox".into()];

        let plan = fresh_plan(vec![plan_entry(&file, rule_id)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");

        let probe = FakeProcessProbe {
            running: HashSet::from(["Firefox".into()]),
            ..Default::default()
        };
        let fake_trash = FakeTrash::default();

        let report = run_apply(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            None,
            std::slice::from_ref(&rule),
            &probe,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(file.exists());
        assert!(fake_trash.calls.lock().unwrap().is_empty());
        assert!(report
            .skipped_by_reason
            .iter()
            .any(|s| s.reason == SkipReason::AppRunning));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn replaced_inode_before_delete_is_skipped_not_succeeded() {
        let _guard = test_env::lock();
        let root = scratch("toctou-replace");
        let file = root.join("victim.txt");
        fs::write(&file, b"planned").unwrap();
        let mut entry = plan_entry(&file, "rule-toctou");
        // 伪造计划身份，模拟 plan 生成后目标被替换。
        entry.ino = entry.ino.wrapping_add(1);
        let plan = fresh_plan(vec![entry]);

        let trash_dir = root.join("Trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        std::env::set_var("MOLE_DELETE_LOG", root.join("deletions.log"));

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(root.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");

        let report = run_apply_defaults(
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
        assert_eq!(report.trashed_bytes, 0);
        assert!(file.exists());

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_skips_orphaned_when_rejudge_fails_after_plan() {
        use crate::orphan::FakeOrphanDeps;
        use std::collections::{HashMap, HashSet};
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }

        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }

        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                Ok(())
            }
        }

        let _guard = test_env::lock();
        let home = scratch("orphan-recheck");
        let cache = home.join("Library/Caches/com.gone.app");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("x"), b"data").unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&cache, ORPHANED_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let fake_trash = FakeTrash::default();

        // Plan 时可能判为 orphan；apply 时该 bundle「又装回来了」。
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::from(["com.gone.app".into()]),
            mdfind: HashMap::new(),
            scan_error: false,
            ..Default::default()
        };

        let report = run_apply_with_orphan_deps(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            &deps,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(cache.exists());
        assert!(fake_trash.calls.lock().unwrap().is_empty());
        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_skips_claude_vm_when_rejudge_sees_installed() {
        use crate::orphan::{FakeOrphanDeps, CLAUDE_DESKTOP_BUNDLE_ID};
        use std::collections::{HashMap, HashSet};
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }
        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }
        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                Ok(())
            }
        }

        let _guard = test_env::lock();
        let home = scratch("claude-vm-recheck-skip");
        let bundle = home.join("Library/Application Support/Claude/vm_bundles/claudevm.bundle");
        fs::create_dir_all(&bundle).unwrap();
        fs::write(bundle.join("rootfs.img"), b"vm").unwrap();
        std::env::set_var("HOME", &home);

        let mut entry = plan_entry(&bundle, ORPHANED_RULE_ID);
        entry.mtime = SystemTime::now() - Duration::from_secs(10 * 86400);
        let plan = fresh_plan(vec![entry]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let fake_trash = FakeTrash::default();
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::from([CLAUDE_DESKTOP_BUNDLE_ID.into()]),
            mdfind: HashMap::from([(CLAUDE_DESKTOP_BUNDLE_ID.into(), Ok(false))]),
            ..Default::default()
        };

        let report = run_apply_with_orphan_deps(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            &deps,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
        assert!(bundle.exists());
        assert!(fake_trash.calls.lock().unwrap().is_empty());
        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn apply_trashes_claude_vm_when_rejudge_passes() {
        use crate::orphan::{FakeOrphanDeps, CLAUDE_DESKTOP_BUNDLE_ID};
        use std::collections::{HashMap, HashSet};
        use std::sync::Mutex;

        struct FakeTrash {
            calls: Mutex<Vec<PathBuf>>,
        }
        impl Default for FakeTrash {
            fn default() -> Self {
                Self {
                    calls: Mutex::new(Vec::new()),
                }
            }
        }
        impl Trash for FakeTrash {
            fn trash_path(
                &self,
                path: &std::path::Path,
                _timeout: Duration,
            ) -> std::io::Result<()> {
                self.calls.lock().unwrap().push(path.to_path_buf());
                fs::remove_dir_all(path).ok();
                Ok(())
            }
        }

        let _guard = test_env::lock();
        let home = scratch("claude-vm-recheck-ok");
        let bundle = home.join("Library/Application Support/Claude/vm_bundles/claudevm.bundle");
        fs::create_dir_all(&bundle).unwrap();
        fs::write(bundle.join("rootfs.img"), b"vm").unwrap();
        let old = SystemTime::now() - Duration::from_secs(10 * 86400);
        fs::File::open(&bundle).unwrap().set_modified(old).unwrap();
        std::env::set_var("HOME", &home);

        let plan = fresh_plan(vec![plan_entry(&bundle, ORPHANED_RULE_ID)]);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(home.join("deletions.log"));
        let mut oplog = OperationLogger::new("clean");
        let fake_trash = FakeTrash::default();
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            mdfind: HashMap::from([(CLAUDE_DESKTOP_BUNDLE_ID.into(), Ok(false))]),
            ..Default::default()
        };

        let report = run_apply_with_orphan_deps(
            &plan,
            &protection,
            apply_opts(false),
            &deletion_log,
            &mut oplog,
            &deps,
            &fake_trash,
        )
        .unwrap();

        assert_eq!(report.succeeded, 1);
        assert_eq!(report.skipped, 0);
        assert!(!bundle.exists());
        assert_eq!(fake_trash.calls.lock().unwrap().len(), 1);
        std::env::remove_var("HOME");
        fs::remove_dir_all(&home).ok();
    }
}
