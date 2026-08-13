//! `worktree` apply：TTL + TOCTOU + 废纸篓 + `git worktree prune`。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use vole_sys::Trash;

use crate::delete::{
    mole_delete_verified, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions,
};
use crate::oplog::OperationLogger;
use crate::ops::worktree_plan::{parse_repo_from_label, GitProbe};
use crate::protection::AppProtection;
use crate::safety::{
    verify_plan_entry_for_apply, PlanApplyError, PlanEntryIdentity, ValidationError,
};
use crate::vole_proto::{
    Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, Report, SkipReason, SkipSummary, StreamEvent,
    SCHEMA_VERSION,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorktreeApplyError {
    #[error("plan expired; rescan with `vole worktree --plan`")]
    Expired,
    #[error("unsupported plan schema version {got} (expected {expected})")]
    UnsupportedSchema { expected: u32, got: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct WorktreeApplyOptions {
    pub permanent: bool,
}

pub struct WorktreeApplyContext<'a> {
    pub protection: &'a AppProtection,
    pub whitelist_patterns: &'a [String],
    pub options: WorktreeApplyOptions,
    pub trash: &'a dyn Trash,
    pub deletion_log: &'a DeletionLogger,
    pub oplog: &'a mut OperationLogger,
    pub on_event: Option<&'a dyn Fn(StreamEvent)>,
    pub now: SystemTime,
    pub cwd: PathBuf,
    pub git: &'a dyn GitProbe,
}

pub fn apply_worktree_plan(
    plan: &ProtoPlan,
    protection: &AppProtection,
    options: WorktreeApplyOptions,
    git: &dyn GitProbe,
    on_event: Option<&dyn Fn(StreamEvent)>,
) -> Result<Report, WorktreeApplyError> {
    let deletion_log = DeletionLogger::from_env();
    let mut oplog = OperationLogger::new("worktree");
    let _ = oplog.session_start();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let mut ctx = WorktreeApplyContext {
        protection,
        whitelist_patterns: &[],
        options,
        trash: &vole_sys::macos::MacTrash,
        deletion_log: &deletion_log,
        oplog: &mut oplog,
        on_event,
        now: SystemTime::now(),
        cwd,
        git,
    };
    let report = apply_worktree_proto_plan(plan, &mut ctx)?;
    let _ = oplog.session_end(
        report.succeeded,
        report.trashed_bytes / 1024 + report.deleted_bytes / 1024,
    );
    Ok(report)
}

pub fn apply_worktree_proto_plan(
    plan: &ProtoPlan,
    ctx: &mut WorktreeApplyContext<'_>,
) -> Result<Report, WorktreeApplyError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(WorktreeApplyError::UnsupportedSchema {
            expected: SCHEMA_VERSION,
            got: plan.schema_version,
        });
    }
    if plan_is_expired(plan, ctx.now) {
        return Err(WorktreeApplyError::Expired);
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
    let cwd = ctx.cwd.canonicalize().unwrap_or_else(|_| ctx.cwd.clone());

    for (idx, entry) in plan.entries.iter().enumerate() {
        if let Some(event) = &ctx.on_event {
            event(StreamEvent::Progress {
                scanned: idx as u64 + 1,
                current: entry.path.display().to_string(),
            });
        }

        if entry.skip_reason.is_some() {
            skipped += 1;
            skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
            continue;
        }

        if !is_worktree_rule(&entry.rule_id) {
            skipped += 1;
            skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            continue;
        }

        let Some(repo) = parse_repo_from_label(&entry.label) else {
            skipped += 1;
            skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
            continue;
        };

        let canon = entry
            .path
            .canonicalize()
            .unwrap_or_else(|_| entry.path.clone());
        if is_hard_excluded(&canon, &cwd, &repo, &entry.rule_id) {
            skipped += 1;
            skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            continue;
        }

        if entry.rule_id == "worktree:stale" {
            if entry.path.exists() {
                skipped += 1;
                skip_tracker.record(SkipReason::PathVanished, &entry.rule_id);
                continue;
            }
            match ctx.git.prune(&repo) {
                Ok(()) => succeeded += 1,
                Err(_) => failed += 1,
            }
            continue;
        }

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
                let should_prune = entry.rule_id != "worktree:orphan-dir"
                    || (canon != repo.canonicalize().unwrap_or_else(|_| repo.clone())
                        && repo.exists());
                let mut pruned = if should_prune {
                    ctx.git.prune(&repo)
                } else {
                    Ok(())
                };
                if pruned.is_err() && has_locked_blocker(entry) {
                    let _ = ctx.git.unlock(&repo, &entry.path);
                    pruned = ctx.git.prune(&repo);
                }
                if pruned.is_err() {
                    failed += 1;
                    continue;
                }
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

fn is_worktree_rule(rule_id: &str) -> bool {
    matches!(
        rule_id,
        "worktree:linked" | "worktree:stale" | "worktree:orphan-dir"
    )
}

fn is_hard_excluded(canon: &Path, cwd: &Path, repo: &Path, rule_id: &str) -> bool {
    if canon == cwd || cwd.starts_with(canon) {
        return true;
    }
    if rule_id == "worktree:orphan-dir" {
        return false;
    }
    let repo_c = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    canon == repo_c
}

fn has_locked_blocker(entry: &ProtoPlanEntry) -> bool {
    if entry.blockers.iter().any(|b| b == "locked") {
        return true;
    }
    label_blockers(&entry.label).contains(&"locked")
}

fn label_blockers(label: &str) -> Vec<&str> {
    let Some(rest) = label.split("blockers=").nth(1) else {
        return Vec::new();
    };
    let csv = rest.split(' ').next().unwrap_or("");
    if csv.is_empty() || csv == "-" {
        return Vec::new();
    }
    csv.split(',').filter(|s| !s.is_empty()).collect()
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
    use crate::ops::worktree_plan::GitProbe;
    use crate::protection::AppProtection;
    use std::path::Path;
    use std::sync::Mutex;

    struct NoopGit;

    impl GitProbe for NoopGit {
        fn worktree_list(&self, _repo: &Path) -> Result<String, String> {
            Ok(String::new())
        }
        fn status_porcelain(&self, _w: &Path, _i: bool) -> Result<String, String> {
            Ok(String::new())
        }
        fn log_unpushed(&self, _w: &Path) -> Result<String, String> {
            Ok(String::new())
        }
        fn last_commit_unix(&self, _w: &Path) -> Result<Option<i64>, String> {
            Ok(None)
        }
        fn rev_parse_toplevel(&self, cwd: &Path) -> Result<PathBuf, String> {
            Ok(cwd.to_path_buf())
        }
        fn prune(&self, _repo: &Path) -> Result<(), String> {
            Ok(())
        }
        fn unlock(&self, _repo: &Path, _w: &Path) -> Result<(), String> {
            Ok(())
        }
    }

    struct RecordingGit {
        prune_calls: Mutex<u32>,
    }

    impl GitProbe for RecordingGit {
        fn worktree_list(&self, _repo: &Path) -> Result<String, String> {
            Ok(String::new())
        }
        fn status_porcelain(&self, _w: &Path, _i: bool) -> Result<String, String> {
            Ok(String::new())
        }
        fn log_unpushed(&self, _w: &Path) -> Result<String, String> {
            Ok(String::new())
        }
        fn last_commit_unix(&self, _w: &Path) -> Result<Option<i64>, String> {
            Ok(None)
        }
        fn rev_parse_toplevel(&self, cwd: &Path) -> Result<PathBuf, String> {
            Ok(cwd.to_path_buf())
        }
        fn prune(&self, _repo: &Path) -> Result<(), String> {
            *self.prune_calls.lock().unwrap() += 1;
            Ok(())
        }
        fn unlock(&self, _repo: &Path, _w: &Path) -> Result<(), String> {
            Ok(())
        }
    }

    fn apply_with(
        plan: &ProtoPlan,
        git: &dyn GitProbe,
        cwd: PathBuf,
    ) -> Result<Report, WorktreeApplyError> {
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::from_env();
        let mut oplog = OperationLogger::new("worktree");
        let mut ctx = WorktreeApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: WorktreeApplyOptions { permanent: false },
            trash: &vole_sys::macos::MacTrash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            cwd,
            git,
        };
        apply_worktree_proto_plan(plan, &mut ctx)
    }

    #[test]
    fn skips_non_worktree_rule_ids() {
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "x".into(),
                path: PathBuf::from("/tmp"),
                label: "repo:/tmp/repo linked git detached blockers=- /tmp".into(),
                size: 0,
                rule_id: "purge:node_modules".into(),
                skip_reason: None,
                dev: 0,
                ino: 0,
                mtime: UNIX_EPOCH,
                blockers: vec![],
            }],
            coverage_note: None,
        };
        let report = apply_worktree_plan(
            &plan,
            &AppProtection::new(),
            WorktreeApplyOptions { permanent: false },
            &NoopGit,
            None,
        )
        .unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
    }

    #[test]
    fn stale_only_prunes_when_path_missing() {
        let missing = std::env::temp_dir().join(format!(
            "vole-wt-stale-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let git = RecordingGit {
            prune_calls: Mutex::new(0),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "s".into(),
                path: missing.clone(),
                label: format!(
                    "repo:/tmp/repo stale git detached blockers=- {}",
                    missing.display()
                ),
                size: 0,
                rule_id: "worktree:stale".into(),
                skip_reason: None,
                dev: 0,
                ino: 0,
                mtime: UNIX_EPOCH,
                blockers: vec![],
            }],
            coverage_note: None,
        };
        let report = apply_with(&plan, &git, PathBuf::from("/tmp")).unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(report.failed, 0);
        assert_eq!(*git.prune_calls.lock().unwrap(), 1);
        assert!(!missing.exists());
    }

    #[test]
    fn refuses_primary_repo_path() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("app");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("keep"), b"x").unwrap();
        let identity = crate::safety::capture_plan_entry_identity(&repo).unwrap();
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "p".into(),
                path: repo.clone(),
                label: format!(
                    "repo:{} linked git detached blockers=- {}",
                    repo.display(),
                    repo.display()
                ),
                size: 1,
                rule_id: "worktree:linked".into(),
                skip_reason: None,
                dev: identity.dev,
                ino: identity.ino,
                mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
                blockers: vec![],
            }],
            coverage_note: None,
        };
        let report = apply_with(&plan, &NoopGit, repo.clone()).unwrap();
        assert_eq!(report.succeeded, 0);
        assert!(report.skipped >= 1);
        assert!(repo.join("keep").exists());
    }

    #[test]
    fn identity_change_skips_without_deleting() {
        let dir = tempfile::tempdir().unwrap();
        let wt = dir.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join("f"), b"x").unwrap();
        let identity = crate::safety::capture_plan_entry_identity(&wt).unwrap();
        let later = SystemTime::now() + Duration::from_secs(120);
        std::fs::File::open(&wt)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(later))
            .unwrap();
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "i".into(),
                path: wt.clone(),
                label: format!(
                    "repo:/tmp/repo linked git detached blockers=- {}",
                    wt.display()
                ),
                size: 1,
                rule_id: "worktree:linked".into(),
                skip_reason: None,
                dev: identity.dev,
                ino: identity.ino,
                mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
                blockers: vec![],
            }],
            coverage_note: None,
        };
        let report = apply_with(&plan, &NoopGit, dir.path().to_path_buf()).unwrap();
        assert_eq!(report.succeeded, 0);
        assert!(report.skipped >= 1);
        assert!(wt.join("f").exists());
    }

    struct UnlockGit {
        prune_calls: Mutex<u32>,
        unlock_calls: Mutex<u32>,
    }

    impl GitProbe for UnlockGit {
        fn worktree_list(&self, _repo: &Path) -> Result<String, String> {
            Ok(String::new())
        }
        fn status_porcelain(&self, _w: &Path, _i: bool) -> Result<String, String> {
            Ok(String::new())
        }
        fn log_unpushed(&self, _w: &Path) -> Result<String, String> {
            Ok(String::new())
        }
        fn last_commit_unix(&self, _w: &Path) -> Result<Option<i64>, String> {
            Ok(None)
        }
        fn rev_parse_toplevel(&self, cwd: &Path) -> Result<PathBuf, String> {
            Ok(cwd.to_path_buf())
        }
        fn prune(&self, _repo: &Path) -> Result<(), String> {
            *self.prune_calls.lock().unwrap() += 1;
            if *self.unlock_calls.lock().unwrap() == 0 {
                Err("locked".into())
            } else {
                Ok(())
            }
        }
        fn unlock(&self, _repo: &Path, _w: &Path) -> Result<(), String> {
            *self.unlock_calls.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[test]
    fn locked_unlocks_then_prunes() {
        let _guard = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let wt = dir.path().join("wt");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join("f"), b"x").unwrap();
        let identity = crate::safety::capture_plan_entry_identity(&wt).unwrap();
        let trash_dir = dir.path().join("trash");
        std::fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        let git = UnlockGit {
            prune_calls: Mutex::new(0),
            unlock_calls: Mutex::new(0),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "l".into(),
                path: wt.clone(),
                label: format!(
                    "repo:{} linked git detached blockers=locked {}",
                    repo.display(),
                    wt.display()
                ),
                size: 1,
                rule_id: "worktree:linked".into(),
                skip_reason: None,
                dev: identity.dev,
                ino: identity.ino,
                mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
                blockers: vec![],
            }],
            coverage_note: None,
        };
        let report = apply_with(&plan, &git, dir.path().to_path_buf()).unwrap();
        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        assert_eq!(report.succeeded, 1);
        assert_eq!(*git.unlock_calls.lock().unwrap(), 1);
        assert!(*git.prune_calls.lock().unwrap() >= 2);
        assert!(!wt.exists());
    }

    #[test]
    fn orphan_independent_clone_trashes_without_treating_as_primary() {
        let _guard = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let clone = dir.path().join("solo");
        std::fs::create_dir_all(clone.join(".git")).unwrap();
        std::fs::write(clone.join("README"), b"x").unwrap();
        let identity = crate::safety::capture_plan_entry_identity(&clone).unwrap();
        let trash_dir = dir.path().join("trash");
        std::fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        let git = RecordingGit {
            prune_calls: Mutex::new(0),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            entries: vec![ProtoPlanEntry {
                id: "o".into(),
                path: clone.clone(),
                label: format!(
                    "repo:{} orphan-dir git detached blockers=- {}",
                    clone.display(),
                    clone.display()
                ),
                size: 1,
                rule_id: "worktree:orphan-dir".into(),
                skip_reason: None,
                dev: identity.dev,
                ino: identity.ino,
                mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
                blockers: vec![],
            }],
            coverage_note: None,
        };
        let report = apply_with(&plan, &git, dir.path().to_path_buf()).unwrap();
        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        assert_eq!(report.succeeded, 1);
        assert_eq!(*git.prune_calls.lock().unwrap(), 0);
        assert!(!clone.exists());
    }
}
