//! `agent` apply：TTL + TOCTOU + 既有废纸篓漏斗。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use vole_sys::Trash;

use crate::delete::{
    mole_delete_verified, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions,
};
use crate::oplog::OperationLogger;
use crate::ops::agent_plan::is_cwd_excluded;
use crate::ops::worktree_plan::{
    collect_worktree_claimed_paths, looks_like_git_checkout, GitProbe, LiveGitProbe,
};
use crate::protection::AppProtection;
use crate::safety::{
    verify_plan_entry_for_apply, PlanApplyError, PlanEntryIdentity, ValidationError,
};
use crate::vole_proto::{
    Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, Report, SkipReason, SkipSummary, StreamEvent,
    SCHEMA_VERSION,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentApplyError {
    #[error("plan expired; rescan with `vole agent --plan`")]
    Expired,
    #[error("unsupported plan schema version {got} (expected {expected})")]
    UnsupportedSchema { expected: u32, got: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct AgentApplyOptions {
    pub permanent: bool,
}

pub struct AgentApplyContext<'a> {
    pub protection: &'a AppProtection,
    pub whitelist_patterns: &'a [String],
    pub options: AgentApplyOptions,
    pub trash: &'a dyn Trash,
    pub deletion_log: &'a DeletionLogger,
    pub oplog: &'a mut OperationLogger,
    pub on_event: Option<&'a dyn Fn(StreamEvent)>,
    pub now: SystemTime,
    pub cwd: PathBuf,
    pub home: PathBuf,
    pub git: &'a dyn GitProbe,
    pub search_roots: Option<&'a [PathBuf]>,
}

pub fn apply_agent_plan(
    plan: &ProtoPlan,
    protection: &AppProtection,
    options: AgentApplyOptions,
    on_event: Option<&dyn Fn(StreamEvent)>,
) -> Result<Report, AgentApplyError> {
    let deletion_log = DeletionLogger::from_env();
    let mut oplog = OperationLogger::new("agent");
    let _ = oplog.session_start();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let git = LiveGitProbe;
    let mut ctx = AgentApplyContext {
        protection,
        whitelist_patterns: &[],
        options,
        trash: &vole_sys::macos::MacTrash,
        deletion_log: &deletion_log,
        oplog: &mut oplog,
        on_event,
        now: SystemTime::now(),
        cwd,
        home,
        git: &git,
        search_roots: None,
    };
    let report = apply_agent_proto_plan(plan, &mut ctx)?;
    let _ = oplog.session_end(
        report.succeeded,
        report.trashed_bytes / 1024 + report.deleted_bytes / 1024,
    );
    Ok(report)
}

pub fn apply_agent_proto_plan(
    plan: &ProtoPlan,
    ctx: &mut AgentApplyContext<'_>,
) -> Result<Report, AgentApplyError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(AgentApplyError::UnsupportedSchema {
            expected: SCHEMA_VERSION,
            got: plan.schema_version,
        });
    }
    if plan_is_expired(plan, ctx.now) {
        return Err(AgentApplyError::Expired);
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
    let claimed = collect_worktree_claimed_paths(&ctx.home, &ctx.cwd, ctx.git, ctx.search_roots);

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

        if !is_agent_rule(&entry.rule_id) {
            skipped += 1;
            skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
            continue;
        }

        let canon = entry
            .path
            .canonicalize()
            .unwrap_or_else(|_| entry.path.clone());
        if is_hard_excluded(&canon, &cwd) || claimed.contains(&canon) {
            skipped += 1;
            skip_tracker.record(SkipReason::Whitelisted, &entry.rule_id);
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

fn is_agent_rule(rule_id: &str) -> bool {
    matches!(rule_id, "agent:container" | "agent:session" | "agent:cache")
}

fn is_hard_excluded(canon: &Path, cwd: &Path) -> bool {
    if is_cwd_excluded(canon, cwd) {
        return true;
    }
    if looks_like_git_checkout(canon) {
        return true;
    }
    canon.components().any(|c| {
        let n = c.as_os_str();
        n == "worktrees" || n == ".worktrees"
    })
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
    use crate::vole_proto::{Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, SCHEMA_VERSION};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

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
        fn rev_parse_toplevel(&self, _cwd: &Path) -> Result<PathBuf, String> {
            Err("no repo".into())
        }
        fn prune(&self, _repo: &Path) -> Result<(), String> {
            Ok(())
        }
        fn unlock(&self, _repo: &Path, _w: &Path) -> Result<(), String> {
            Ok(())
        }
    }

    struct ClaimGit {
        repo: PathBuf,
        extra: PathBuf,
    }

    impl GitProbe for ClaimGit {
        fn worktree_list(&self, repo: &Path) -> Result<String, String> {
            if repo != self.repo {
                return Ok(String::new());
            }
            Ok(format!(
                "worktree {}\nHEAD abc\n\nworktree {}\nHEAD def\n",
                self.repo.display(),
                self.extra.display()
            ))
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
        fn rev_parse_toplevel(&self, _cwd: &Path) -> Result<PathBuf, String> {
            Err("no repo".into())
        }
        fn prune(&self, _repo: &Path) -> Result<(), String> {
            Ok(())
        }
        fn unlock(&self, _repo: &Path, _w: &Path) -> Result<(), String> {
            Ok(())
        }
    }

    fn empty_plan(rule_id: &str, path: PathBuf) -> ProtoPlan {
        ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            coverage_note: None,
            entries: vec![ProtoPlanEntry {
                id: "x".into(),
                path,
                label: "cache cursor blockers=- /tmp".into(),
                size: 0,
                rule_id: rule_id.into(),
                skip_reason: None,
                dev: 0,
                ino: 0,
                mtime: UNIX_EPOCH,
                blockers: vec![],
            }],
        }
    }

    fn apply_with(
        plan: &ProtoPlan,
        git: &dyn GitProbe,
        home: PathBuf,
        cwd: PathBuf,
        search_roots: &[PathBuf],
    ) -> Result<Report, AgentApplyError> {
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::from_env();
        let mut oplog = OperationLogger::new("agent");
        let mut ctx = AgentApplyContext {
            protection: &protection,
            whitelist_patterns: &[],
            options: AgentApplyOptions { permanent: false },
            trash: &vole_sys::macos::MacTrash,
            deletion_log: &deletion_log,
            oplog: &mut oplog,
            on_event: None,
            now: SystemTime::now(),
            cwd,
            home,
            git,
            search_roots: Some(search_roots),
        };
        apply_agent_proto_plan(plan, &mut ctx)
    }

    #[test]
    fn skips_non_agent_rule_ids() {
        let report = apply_agent_plan(
            &empty_plan("worktree:linked", PathBuf::from("/tmp")),
            &AppProtection::new(),
            AgentApplyOptions { permanent: false },
            None,
        )
        .unwrap();
        assert_eq!(report.succeeded, 0);
        assert_eq!(report.skipped, 1);
    }

    #[test]
    fn skips_cwd_and_git_checkout_even_if_in_plan() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().join("cwd");
        let checkout = dir.path().join("wt");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(checkout.join(".git")).unwrap();
        let mut plan = empty_plan("agent:cache", cwd.clone());
        plan.entries.push(ProtoPlanEntry {
            id: "y".into(),
            path: checkout.clone(),
            label: "container cursor blockers=- x".into(),
            size: 0,
            rule_id: "agent:container".into(),
            skip_reason: None,
            dev: 0,
            ino: 0,
            mtime: UNIX_EPOCH,
            blockers: vec![],
        });
        let report =
            apply_with(&plan, &NoopGit, dir.path().to_path_buf(), cwd.clone(), &[]).unwrap();
        assert_eq!(report.succeeded, 0);
        assert!(report.skipped >= 2);
        assert!(cwd.exists());
        assert!(checkout.exists());
    }

    #[test]
    fn skips_worktree_claimed_orphan_dir_even_if_stuffed_in_plan() {
        let _guard = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let orphan = dir.path().join("orphan-wt");
        std::fs::create_dir_all(&orphan).unwrap();
        std::fs::write(orphan.join("keep-me"), b"x").unwrap();
        let identity = crate::safety::capture_plan_entry_identity(&orphan).unwrap();
        let trash_dir = dir.path().join("trash");
        std::fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        let git = ClaimGit {
            repo: repo.clone(),
            extra: orphan.clone(),
        };
        let plan = ProtoPlan {
            schema_version: SCHEMA_VERSION,
            created_at: SystemTime::now(),
            ttl_secs: 900,
            coverage_note: None,
            entries: vec![ProtoPlanEntry {
                id: "stuffed".into(),
                path: orphan.clone(),
                label: format!("container cursor blockers=- {}", orphan.display()),
                size: 1,
                rule_id: "agent:container".into(),
                skip_reason: None,
                dev: identity.dev,
                ino: identity.ino,
                mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
                blockers: vec![],
            }],
        };
        let report = apply_with(
            &plan,
            &git,
            dir.path().to_path_buf(),
            dir.path().join("cwd"),
            &[dir.path().to_path_buf()],
        )
        .unwrap();
        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        assert_eq!(report.succeeded, 0, "claimed checkout must not be deleted");
        assert!(report.skipped >= 1);
        assert!(orphan.join("keep-me").exists());
    }
}
