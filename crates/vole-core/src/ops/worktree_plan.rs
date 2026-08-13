//! Git worktree 盘点：porcelain 解析、来源判定、启发式排序。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use vole_sys::timeouts::{MEDIUM_PROBE, PKG_LIST, SHORT_QUERY};

use crate::protection::AppProtection;
use crate::safety::{capture_plan_entry_identity, validate_path_for_deletion};
use crate::vole_proto::{Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, SCHEMA_VERSION};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeKind {
    Linked,
    Stale,
    OrphanDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeSource {
    Git,
    Cursor,
    Codex,
    Claude,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeHead {
    Detached,
    Branch(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorcelainWorktree {
    pub path: PathBuf,
    pub head: WorktreeHead,
    pub locked: bool,
    pub prunable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRecord {
    pub path: PathBuf,
    pub repo: PathBuf,
    pub kind: WorktreeKind,
    pub source: WorktreeSource,
    pub head: WorktreeHead,
    pub locked: bool,
    pub size: u64,
    pub age_unix: i64,
    pub blockers: Vec<String>,
}

pub fn parse_worktree_porcelain(text: &str) -> Vec<PorcelainWorktree> {
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let mut path = None;
        let mut head = WorktreeHead::Detached;
        let mut locked = false;
        let mut prunable = false;
        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(rest));
            } else if let Some(rest) = line.strip_prefix("branch ") {
                let name = rest.strip_prefix("refs/heads/").unwrap_or(rest).to_string();
                head = WorktreeHead::Branch(name);
            } else if line == "detached" {
                head = WorktreeHead::Detached;
            } else if line == "locked" || line.starts_with("locked ") {
                locked = true;
            } else if line == "prunable" || line.starts_with("prunable ") {
                prunable = true;
            }
        }
        if let Some(path) = path {
            out.push(PorcelainWorktree {
                path,
                head,
                locked,
                prunable,
            });
        }
    }
    out
}

pub fn source_for_path(home: &Path, path: &Path) -> WorktreeSource {
    let p = path.to_string_lossy();
    let codex = home.join(".codex/worktrees");
    if path.starts_with(&codex) {
        return WorktreeSource::Codex;
    }
    let claude_home = home.join(".claude/worktrees");
    if path.starts_with(&claude_home) || p.contains("/.claude/worktrees/") {
        return WorktreeSource::Claude;
    }
    if p.contains("/.worktrees/") {
        return WorktreeSource::Cursor;
    }
    WorktreeSource::Git
}

pub fn rule_id_for(kind: WorktreeKind) -> &'static str {
    match kind {
        WorktreeKind::Linked => "worktree:linked",
        WorktreeKind::Stale => "worktree:stale",
        WorktreeKind::OrphanDir => "worktree:orphan-dir",
    }
}

pub fn sort_worktree_records(rows: &mut [WorktreeRecord]) {
    rows.sort_by(|a, b| {
        kind_rank(a.kind)
            .cmp(&kind_rank(b.kind))
            .then_with(|| age_key(a.age_unix).cmp(&age_key(b.age_unix)))
            .then_with(|| head_rank(&a.head).cmp(&head_rank(&b.head)))
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.path.cmp(&b.path))
    });
}

fn kind_rank(kind: WorktreeKind) -> u8 {
    match kind {
        WorktreeKind::Stale => 0,
        WorktreeKind::OrphanDir => 1,
        WorktreeKind::Linked => 2,
    }
}

fn age_key(age_unix: i64) -> i64 {
    if age_unix == 0 {
        i64::MAX
    } else {
        age_unix
    }
}

fn head_rank(head: &WorktreeHead) -> u8 {
    match head {
        WorktreeHead::Detached => 0,
        WorktreeHead::Branch(_) => 1,
    }
}

pub fn format_worktree_label(row: &WorktreeRecord) -> String {
    let kind = match row.kind {
        WorktreeKind::Linked => "linked",
        WorktreeKind::Stale => "stale",
        WorktreeKind::OrphanDir => "orphan-dir",
    };
    let source = match row.source {
        WorktreeSource::Git => "git",
        WorktreeSource::Cursor => "cursor",
        WorktreeSource::Codex => "codex",
        WorktreeSource::Claude => "claude",
    };
    let head = match &row.head {
        WorktreeHead::Detached => "detached".to_string(),
        WorktreeHead::Branch(name) => format!("branch:{name}"),
    };
    let blockers = if row.blockers.is_empty() {
        "-".to_string()
    } else {
        row.blockers.join(",")
    };
    format!(
        "repo:{} {kind} {source} {head} blockers={blockers} {}",
        row.repo.display(),
        row.path.display()
    )
}

pub fn parse_repo_from_label(label: &str) -> Option<PathBuf> {
    let rest = label.strip_prefix("repo:")?;
    for marker in [" linked ", " stale ", " orphan-dir "] {
        if let Some(idx) = rest.find(marker) {
            return Some(PathBuf::from(&rest[..idx]));
        }
    }
    None
}

pub const DEFAULT_WORKTREE_TTL_SECS: u64 = 900;

const COVERAGE_NOTE: &str =
    "worktree scan skips Conductor-style deep containers; positive removal verdicts are out of scope.";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorktreePlanError {
    #[error("HOME not usable: {0}")]
    Home(String),
}

pub struct WorktreePlanOptions<'a> {
    pub home: &'a Path,
    pub cwd: &'a Path,
    pub ttl_secs: u64,
    pub search_roots: Option<&'a [PathBuf]>,
    pub now: SystemTime,
    pub git: &'a dyn GitProbe,
}

pub trait GitProbe {
    fn worktree_list(&self, repo: &Path) -> Result<String, String>;
    fn status_porcelain(&self, worktree: &Path, ignored: bool) -> Result<String, String>;
    fn log_unpushed(&self, worktree: &Path) -> Result<String, String>;
    fn last_commit_unix(&self, worktree: &Path) -> Result<Option<i64>, String>;
    fn rev_parse_toplevel(&self, cwd: &Path) -> Result<PathBuf, String>;
    fn prune(&self, repo: &Path) -> Result<(), String>;
    fn unlock(&self, repo: &Path, worktree: &Path) -> Result<(), String>;
}

pub struct LiveGitProbe;

impl GitProbe for LiveGitProbe {
    fn worktree_list(&self, repo: &Path) -> Result<String, String> {
        git_stdout(&["worktree", "list", "--porcelain"], Some(repo), PKG_LIST)
    }

    fn status_porcelain(&self, worktree: &Path, ignored: bool) -> Result<String, String> {
        if ignored {
            git_stdout(
                &["status", "--porcelain", "--ignored"],
                Some(worktree),
                MEDIUM_PROBE,
            )
        } else {
            git_stdout(&["status", "--porcelain"], Some(worktree), MEDIUM_PROBE)
        }
    }

    fn log_unpushed(&self, worktree: &Path) -> Result<String, String> {
        git_stdout(
            &["log", "HEAD", "--not", "--remotes", "--pretty=%H"],
            Some(worktree),
            MEDIUM_PROBE,
        )
    }

    fn last_commit_unix(&self, worktree: &Path) -> Result<Option<i64>, String> {
        let out = git_stdout(&["log", "-1", "--format=%ct"], Some(worktree), SHORT_QUERY)?;
        let s = out.trim();
        if s.is_empty() {
            return Ok(None);
        }
        s.parse::<i64>().map(Some).map_err(|e| e.to_string())
    }

    fn rev_parse_toplevel(&self, cwd: &Path) -> Result<PathBuf, String> {
        let out = git_stdout(&["rev-parse", "--show-toplevel"], Some(cwd), SHORT_QUERY)?;
        let t = out.trim();
        if t.is_empty() {
            return Err("empty toplevel".into());
        }
        Ok(PathBuf::from(t))
    }

    fn prune(&self, repo: &Path) -> Result<(), String> {
        git_stdout(&["worktree", "prune"], Some(repo), PKG_LIST).map(|_| ())
    }

    fn unlock(&self, repo: &Path, worktree: &Path) -> Result<(), String> {
        let path = worktree.display().to_string();
        git_stdout(
            &["worktree", "unlock", "--", &path],
            Some(repo),
            MEDIUM_PROBE,
        )
        .map(|_| ())
    }
}

pub fn build_worktree_plan(
    protection: &AppProtection,
    opts: &WorktreePlanOptions<'_>,
) -> Result<ProtoPlan, WorktreePlanError> {
    if !opts.home.is_absolute() {
        return Err(WorktreePlanError::Home(opts.home.display().to_string()));
    }

    let mut roots: Vec<PathBuf> = match opts.search_roots {
        Some(r) => r.to_vec(),
        None => super::purge_plan::resolve_search_roots(opts.home),
    };
    let mut timed_out_repos = 0u64;
    if let Ok(top) = opts.git.rev_parse_toplevel(opts.cwd) {
        if !roots.iter().any(|r| r == &top) {
            roots.push(top);
        }
    }

    let repos = discover_git_repos(&roots);
    let cwd_canon = opts
        .cwd
        .canonicalize()
        .unwrap_or_else(|_| opts.cwd.to_path_buf());

    let mut records: Vec<WorktreeRecord> = Vec::new();
    let mut seen = BTreeSet::new();

    for repo in &repos {
        let text = match opts.git.worktree_list(repo) {
            Ok(t) => t,
            Err(_) => {
                timed_out_repos += 1;
                continue;
            }
        };
        let listed = parse_worktree_porcelain(&text);
        let Some(primary) = listed.first() else {
            continue;
        };
        for (idx, wt) in listed.iter().enumerate() {
            let canon = wt.path.canonicalize().unwrap_or_else(|_| wt.path.clone());
            if idx == 0 {
                continue;
            }
            if is_excluded(&canon, &cwd_canon, &primary.path) {
                continue;
            }
            if !seen.insert(canon.clone()) {
                continue;
            }
            let missing = !wt.path.exists() || wt.prunable;
            let kind = if missing {
                WorktreeKind::Stale
            } else {
                WorktreeKind::Linked
            };
            records.push(make_record(
                opts,
                protection,
                &canon,
                repo,
                kind,
                wt.head.clone(),
                wt.locked,
                missing,
            ));
        }
    }

    for child in agent_checkout_dirs(opts.home, &repos) {
        let canon = child.canonicalize().unwrap_or_else(|_| child.clone());
        if !seen.insert(canon.clone()) {
            continue;
        }
        if is_excluded(&canon, &cwd_canon, Path::new("")) {
            continue;
        }
        let repo = repo_from_gitfile(&child).unwrap_or_else(|| child.clone());
        if is_excluded(&canon, &cwd_canon, &repo) {
            continue;
        }
        records.push(make_record(
            opts,
            protection,
            &canon,
            &repo,
            WorktreeKind::OrphanDir,
            WorktreeHead::Detached,
            false,
            false,
        ));
    }

    sort_worktree_records(&mut records);

    let mut entries = Vec::new();
    for row in records {
        if let Some(entry) = record_to_entry(&row, protection) {
            entries.push(entry);
        }
    }

    let mut note = COVERAGE_NOTE.to_string();
    if timed_out_repos > 0 {
        note.push_str(&format!(
            " skipped {timed_out_repos} git repo(s) on timeout."
        ));
    }

    Ok(ProtoPlan {
        schema_version: SCHEMA_VERSION,
        created_at: opts.now,
        ttl_secs: opts.ttl_secs,
        entries,
        coverage_note: Some(note),
    })
}

fn is_excluded(canon: &Path, cwd: &Path, primary: &Path) -> bool {
    let prim = primary
        .canonicalize()
        .unwrap_or_else(|_| primary.to_path_buf());
    if !prim.as_os_str().is_empty() && canon == prim {
        return true;
    }
    canon == cwd || cwd.starts_with(canon)
}

fn discover_git_repos(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut repos = BTreeSet::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for ent in jwalk::WalkDir::new(root).max_depth(6).skip_hidden(false) {
            let Ok(ent) = ent else {
                continue;
            };
            let path = ent.path();
            if has_purge_component(&path) {
                continue;
            }
            if path.join(".git").is_dir() {
                repos.insert(path);
            }
        }
    }
    repos.into_iter().collect()
}

fn has_purge_component(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|n| super::purge_plan::PURGE_TARGETS.contains(&n))
    })
}

fn agent_checkout_dirs(home: &Path, repos: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    push_checkout_children(&home.join(".codex/worktrees"), &mut dirs);
    push_checkout_children(&home.join(".claude/worktrees"), &mut dirs);
    for repo in repos {
        push_checkout_children(&repo.join(".worktrees"), &mut dirs);
        push_checkout_children(&repo.join(".claude/worktrees"), &mut dirs);
    }
    dirs
}

fn push_checkout_children(container: &Path, dirs: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(container) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() && p.join(".git").exists() {
            dirs.push(p);
        }
    }
}

fn repo_from_gitfile(dir: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(dir.join(".git")).ok()?;
    let line = text.lines().next()?;
    let gitdir = line.strip_prefix("gitdir:")?.trim();
    let idx = gitdir.find("/.git/worktrees/")?;
    Some(PathBuf::from(&gitdir[..idx]))
}

#[allow(clippy::too_many_arguments)]
fn make_record(
    opts: &WorktreePlanOptions<'_>,
    _protection: &AppProtection,
    path: &Path,
    repo: &Path,
    kind: WorktreeKind,
    head: WorktreeHead,
    locked: bool,
    missing: bool,
) -> WorktreeRecord {
    let source = source_for_path(opts.home, path);
    let (size, age_unix, blockers) = if missing {
        (0, 0, Vec::new())
    } else {
        let size = dir_size(path);
        let age_unix = opts
            .git
            .last_commit_unix(path)
            .ok()
            .flatten()
            .or_else(|| {
                fs::symlink_metadata(path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
            })
            .unwrap_or(0);
        let blockers = collect_blockers(opts.git, path, locked);
        (size, age_unix, blockers)
    };
    WorktreeRecord {
        path: path.to_path_buf(),
        repo: repo.to_path_buf(),
        kind,
        source,
        head,
        locked,
        size,
        age_unix,
        blockers,
    }
}

fn collect_blockers(git: &dyn GitProbe, wt: &Path, locked: bool) -> Vec<String> {
    let mut blockers = Vec::new();
    match git.status_porcelain(wt, false) {
        Ok(s) if !s.trim().is_empty() => blockers.push("dirty".into()),
        Err(_) => blockers.push("status-unknown".into()),
        Ok(_) => {}
    }
    if let Ok(s) = git.status_porcelain(wt, true) {
        for line in s.lines() {
            let Some(rest) = line.strip_prefix("!! ") else {
                continue;
            };
            let name = Path::new(rest.trim_end_matches('/'))
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if !name.is_empty() && !super::purge_plan::PURGE_TARGETS.contains(&name) {
                if !blockers.iter().any(|b| b == "ignored-keep") {
                    blockers.push("ignored-keep".into());
                }
            }
        }
    }
    if let Ok(s) = git.log_unpushed(wt) {
        if !s.trim().is_empty() {
            blockers.push("unpushed".into());
        }
    }
    if locked {
        blockers.push("locked".into());
    }
    blockers
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    for ent in jwalk::WalkDir::new(path).max_depth(4).skip_hidden(false) {
        let Ok(ent) = ent else {
            continue;
        };
        if let Ok(m) = ent.metadata() {
            if m.is_file() {
                total = total.saturating_add(m.len());
            }
        }
    }
    total
}

fn record_to_entry(row: &WorktreeRecord, protection: &AppProtection) -> Option<ProtoPlanEntry> {
    let path_str = row.path.display().to_string();
    let (dev, ino, mtime) = if row.kind == WorktreeKind::Stale {
        (0, 0, UNIX_EPOCH)
    } else {
        validate_path_for_deletion(&path_str, protection).ok()?;
        let identity = capture_plan_entry_identity(&row.path).ok()?;
        (
            identity.dev,
            identity.ino,
            UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
        )
    };
    let kind_s = match row.kind {
        WorktreeKind::Linked => "linked",
        WorktreeKind::Stale => "stale",
        WorktreeKind::OrphanDir => "orphan-dir",
    };
    Some(ProtoPlanEntry {
        id: format!("worktree:{kind_s}:{}", row.path.display()),
        path: row.path.clone(),
        label: format_worktree_label(row),
        size: row.size,
        rule_id: rule_id_for(row.kind).to_string(),
        skip_reason: None,
        dev,
        ino,
        mtime,
        blockers: row.blockers.clone(),
    })
}

fn git_stdout(args: &[&str], dir: Option<&Path>, timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new("git");
    if let Some(dir) = dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.args(args);
    let out = run_command_timeout(cmd, timeout)?;
    if !out.status.success() {
        return Err(format!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn run_command_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().map_err(|e| e.to_string()),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("command timed out after {}s", timeout.as_secs()));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_porcelain_main_and_linked_and_locked() {
        let text = "\
worktree /Users/me/src/app
HEAD abc
branch refs/heads/main

worktree /Users/me/src/app/.worktrees/feat
HEAD def
detached
locked

worktree /Users/me/gone
HEAD ghi
branch refs/heads/old
prunable gitdir gone
";
        let rows = parse_worktree_porcelain(text);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].path, PathBuf::from("/Users/me/src/app"));
        assert!(matches!(rows[0].head, WorktreeHead::Branch(ref b) if b == "main"));
        assert!(!rows[0].locked);
        assert!(matches!(rows[1].head, WorktreeHead::Detached));
        assert!(rows[1].locked);
        assert!(rows[2].prunable);
        assert_eq!(rule_id_for(WorktreeKind::Linked), "worktree:linked");
        assert_eq!(rule_id_for(WorktreeKind::Stale), "worktree:stale");
        assert_eq!(rule_id_for(WorktreeKind::OrphanDir), "worktree:orphan-dir");
    }

    #[test]
    fn source_for_agent_containers() {
        let home = Path::new("/Users/me");
        assert_eq!(
            source_for_path(home, Path::new("/Users/me/.codex/worktrees/a")),
            WorktreeSource::Codex
        );
        assert_eq!(
            source_for_path(home, Path::new("/Users/me/.claude/worktrees/a")),
            WorktreeSource::Claude
        );
        assert_eq!(
            source_for_path(home, Path::new("/Users/me/src/app/.claude/worktrees/a")),
            WorktreeSource::Claude
        );
        assert_eq!(
            source_for_path(home, Path::new("/Users/me/src/app/.worktrees/a")),
            WorktreeSource::Cursor
        );
        assert_eq!(
            source_for_path(home, Path::new("/tmp/other")),
            WorktreeSource::Git
        );
    }

    #[test]
    fn sort_stale_and_orphan_before_linked_then_age_then_detached() {
        let mut rows = vec![
            rec(
                "linked-new",
                WorktreeKind::Linked,
                WorktreeHead::Branch("x".into()),
                200,
                10,
            ),
            rec(
                "linked-old-det",
                WorktreeKind::Linked,
                WorktreeHead::Detached,
                50,
                10,
            ),
            rec(
                "orphan",
                WorktreeKind::OrphanDir,
                WorktreeHead::Detached,
                90,
                1,
            ),
            rec("stale", WorktreeKind::Stale, WorktreeHead::Detached, 80, 0),
        ];
        sort_worktree_records(&mut rows);
        let names: Vec<_> = rows
            .iter()
            .map(|r| r.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["stale", "orphan", "linked-old-det", "linked-new"]);
    }

    fn rec(
        name: &str,
        kind: WorktreeKind,
        head: WorktreeHead,
        age: i64,
        size: u64,
    ) -> WorktreeRecord {
        WorktreeRecord {
            path: PathBuf::from(format!("/tmp/{name}")),
            repo: PathBuf::from("/tmp/repo"),
            kind,
            source: WorktreeSource::Git,
            head,
            locked: false,
            size,
            age_unix: age,
            blockers: vec![],
        }
    }

    #[test]
    fn label_roundtrip_repo_with_spaces() {
        let row = WorktreeRecord {
            path: PathBuf::from("/tmp/my wt"),
            repo: PathBuf::from("/tmp/my repo"),
            kind: WorktreeKind::Linked,
            source: WorktreeSource::Cursor,
            head: WorktreeHead::Detached,
            locked: false,
            size: 1,
            age_unix: 1,
            blockers: vec!["dirty".into(), "unpushed".into()],
        };
        let label = format_worktree_label(&row);
        assert!(!label.to_lowercase().contains("safe"));
        assert_eq!(parse_repo_from_label(&label).unwrap(), row.repo);
    }

    struct FakeGit {
        lists: std::collections::HashMap<PathBuf, String>,
        toplevel: PathBuf,
        statuses: std::collections::HashMap<PathBuf, String>,
        ignored: std::collections::HashMap<PathBuf, String>,
        unpushed: std::collections::HashMap<PathBuf, String>,
        ages: std::collections::HashMap<PathBuf, i64>,
    }

    fn lookup_map<'a>(
        map: &'a std::collections::HashMap<PathBuf, String>,
        p: &Path,
    ) -> Option<&'a String> {
        if let Some(v) = map.get(p) {
            return Some(v);
        }
        let want = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        map.iter().find_map(|(k, v)| {
            let k = k.canonicalize().unwrap_or_else(|_| k.clone());
            (k == want).then_some(v)
        })
    }

    fn lookup_age(map: &std::collections::HashMap<PathBuf, i64>, p: &Path) -> Option<i64> {
        if let Some(v) = map.get(p) {
            return Some(*v);
        }
        let want = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        map.iter().find_map(|(k, v)| {
            let k = k.canonicalize().unwrap_or_else(|_| k.clone());
            (k == want).then_some(*v)
        })
    }

    impl GitProbe for FakeGit {
        fn worktree_list(&self, repo: &Path) -> Result<String, String> {
            Ok(lookup_map(&self.lists, repo).cloned().unwrap_or_default())
        }
        fn status_porcelain(&self, worktree: &Path, ignored: bool) -> Result<String, String> {
            let map = if ignored {
                &self.ignored
            } else {
                &self.statuses
            };
            Ok(lookup_map(map, worktree).cloned().unwrap_or_default())
        }
        fn log_unpushed(&self, worktree: &Path) -> Result<String, String> {
            Ok(lookup_map(&self.unpushed, worktree)
                .cloned()
                .unwrap_or_default())
        }
        fn last_commit_unix(&self, worktree: &Path) -> Result<Option<i64>, String> {
            Ok(lookup_age(&self.ages, worktree))
        }
        fn rev_parse_toplevel(&self, _cwd: &Path) -> Result<PathBuf, String> {
            Ok(self.toplevel.clone())
        }
        fn prune(&self, _repo: &Path) -> Result<(), String> {
            Ok(())
        }
        fn unlock(&self, _repo: &Path, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn build_plan_excludes_primary_and_cwd_merges_orphan() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let repo = home.join("Projects/app");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let extra = repo.join(".worktrees/feat");
        fs::create_dir_all(&extra).unwrap();
        fs::write(extra.join(".git"), b"gitdir: /x\n").unwrap();
        let cwd_wt = extra.clone();
        let other = repo.join(".worktrees/old");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join(".git"), b"gitdir: /x\n").unwrap();
        fs::write(other.join("README"), b"x").unwrap();
        let porcelain = format!(
            "worktree {}\nHEAD a\nbranch refs/heads/main\n\nworktree {}\nHEAD b\ndetached\n\nworktree {}\nHEAD c\ndetached\n",
            repo.display(),
            extra.display(),
            other.display()
        );
        let git = FakeGit {
            lists: std::collections::HashMap::from([(repo.clone(), porcelain)]),
            toplevel: repo.clone(),
            statuses: std::collections::HashMap::new(),
            ignored: std::collections::HashMap::new(),
            unpushed: std::collections::HashMap::new(),
            ages: std::collections::HashMap::from([(other.clone(), 10)]),
        };
        let roots = [home.join("Projects")];
        let plan = build_worktree_plan(
            &AppProtection::new(),
            &WorktreePlanOptions {
                home,
                cwd: &cwd_wt,
                ttl_secs: 900,
                search_roots: Some(&roots),
                now: SystemTime::now(),
                git: &git,
            },
        )
        .unwrap();
        let other_canon = other.canonicalize().unwrap();
        let repo_canon = repo.canonicalize().unwrap();
        let cwd_canon = cwd_wt.canonicalize().unwrap();
        assert!(plan.entries.iter().all(|e| e.path != repo_canon));
        assert!(plan.entries.iter().all(|e| e.path != cwd_canon));
        assert!(
            plan.entries
                .iter()
                .any(|e| e.path == other_canon && e.rule_id == "worktree:linked"),
            "entries={:?}",
            plan.entries
                .iter()
                .map(|e| (&e.path, &e.rule_id))
                .collect::<Vec<_>>()
        );
        let note = plan.coverage_note.as_deref().unwrap().to_lowercase();
        assert!(note.contains("conductor"));
        assert!(note.contains("positive removal verdicts"));
        let json = serde_json::to_string(&plan).unwrap();
        assert!(!json.contains("\"safe\""));
        assert!(!json.contains("\"deletable\""));
    }

    #[test]
    fn ignored_env_is_ignored_keep_node_modules_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let repo = home.join("Projects/app");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let other = repo.join(".worktrees/old");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join(".git"), b"gitdir: /x\n").unwrap();
        fs::write(other.join("README"), b"x").unwrap();
        let porcelain = format!(
            "worktree {}\nHEAD a\nbranch refs/heads/main\n\nworktree {}\nHEAD c\ndetached\n",
            repo.display(),
            other.display()
        );
        let git = FakeGit {
            lists: std::collections::HashMap::from([(repo.clone(), porcelain)]),
            toplevel: repo.clone(),
            statuses: std::collections::HashMap::new(),
            ignored: std::collections::HashMap::from([(
                other.clone(),
                "!! .env\n!! node_modules/\n".into(),
            )]),
            unpushed: std::collections::HashMap::new(),
            ages: std::collections::HashMap::from([(other.clone(), 10)]),
        };
        let roots = [home.join("Projects")];
        let plan = build_worktree_plan(
            &AppProtection::new(),
            &WorktreePlanOptions {
                home,
                cwd: &repo,
                ttl_secs: 900,
                search_roots: Some(&roots),
                now: SystemTime::now(),
                git: &git,
            },
        )
        .unwrap();
        let other_canon = other.canonicalize().unwrap();
        let entry = plan
            .entries
            .iter()
            .find(|e| e.path == other_canon)
            .expect("old worktree");
        assert!(entry.blockers.iter().any(|b| b == "ignored-keep"));
        assert_eq!(
            entry
                .blockers
                .iter()
                .filter(|b| *b == "ignored-keep")
                .count(),
            1
        );
    }
}
