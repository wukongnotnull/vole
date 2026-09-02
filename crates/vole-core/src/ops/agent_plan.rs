use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use super::clean_hints::PathSizeKb;
use super::worktree_plan::{
    collect_worktree_claimed_paths, discover_git_repos, looks_like_git_checkout, GitProbe,
};
use crate::protection::AppProtection;
use crate::safety::{capture_plan_entry_identity, validate_path_for_deletion};
use crate::vole_proto::{Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, SCHEMA_VERSION};

pub const DEFAULT_AGENT_TTL_SECS: u64 = 900;
pub const DEFAULT_AGENT_SCAN_BUDGET_SECS: u64 = 15;
pub const DEFAULT_AGENT_PER_ROOT_SECS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Container,
    Session,
    Cache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSource {
    Cursor,
    Codex,
    Claude,
    Repo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    pub path: PathBuf,
    pub kind: AgentKind,
    pub source: AgentSource,
    pub size: u64,
    pub age_unix: i64,
    pub blockers: Vec<String>,
}

pub fn rule_id_for(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Container => "agent:container",
        AgentKind::Session => "agent:session",
        AgentKind::Cache => "agent:cache",
    }
}

pub fn source_for_path(home: &Path, path: &Path) -> AgentSource {
    let p = path.to_string_lossy();
    if path.starts_with(&home.join(".codex")) {
        return AgentSource::Codex;
    }
    if path.starts_with(&home.join(".claude")) || p.contains("/.claude/") {
        return AgentSource::Claude;
    }
    if path.starts_with(&home.join(".cursor")) || p.contains("/.cursor/") {
        return AgentSource::Cursor;
    }
    AgentSource::Repo
}

pub fn named_relatives(kind: AgentKind) -> &'static [&'static str] {
    match kind {
        AgentKind::Container => &["projects/*"],
        AgentKind::Session => &[
            "projects/*/agent-transcripts",
            "chats",
            "sessions",
            "archived_sessions",
            "history.jsonl",
            "todos",
            "file-history",
        ],
        AgentKind::Cache => &[
            "Cache",
            "CachedData",
            "CachedExtensionVSIXs",
            "CachedProfilesData",
            "GPUCache",
            "Code Cache",
            "DawnGraphiteCache",
            "DawnWebGPUCache",
            "blob_storage",
            "logs",
            "Crashpad",
            "sentry",
            ".tmp",
            "tmp",
            "log",
            "statsig",
            "debug",
            "shell-snapshots",
            "ide",
            "telemetry",
        ],
    }
}

pub fn expand_allowlist(root: &Path) -> Vec<(AgentKind, PathBuf)> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    for child in list_dir_names(root.join("projects")) {
        push_if_ok(&mut out, AgentKind::Container, child.clone());
        push_if_ok(
            &mut out,
            AgentKind::Session,
            child.join("agent-transcripts"),
        );
    }
    for rel in [
        "chats",
        "sessions",
        "archived_sessions",
        "todos",
        "file-history",
    ] {
        push_if_ok(&mut out, AgentKind::Session, root.join(rel));
    }
    push_if_ok(&mut out, AgentKind::Session, root.join("history.jsonl"));
    for rel in [
        "Cache",
        "CachedData",
        "CachedExtensionVSIXs",
        "CachedProfilesData",
        "GPUCache",
        "Code Cache",
        "DawnGraphiteCache",
        "DawnWebGPUCache",
        "blob_storage",
        "logs",
        "Crashpad",
        "sentry",
        ".tmp",
        "tmp",
        "log",
        "statsig",
        "debug",
        "shell-snapshots",
        "ide",
        "telemetry",
    ] {
        push_if_ok(&mut out, AgentKind::Cache, root.join(rel));
    }
    out
}

fn push_if_ok(out: &mut Vec<(AgentKind, PathBuf)>, kind: AgentKind, path: PathBuf) {
    if !path.exists() {
        return;
    }
    if is_never_candidate(&path) || looks_like_git_checkout(&path) {
        return;
    }
    if path.components().any(|c| {
        let n = c.as_os_str();
        n == "worktrees" || n == ".worktrees"
    }) {
        return;
    }
    out.push((kind, path));
}

fn list_dir_names(dir: PathBuf) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

pub fn is_never_candidate(path: &Path) -> bool {
    const FILES: &[&str] = &[
        "auth.json",
        "config.toml",
        "settings.json",
        ".credentials.json",
        "argv.json",
        "mcp.json",
    ];
    const DIRS: &[&str] = &[
        "extensions",
        "User",
        "plugins",
        "skills",
        "prompts",
        "memories",
        "rules",
        ".git",
        "worktrees",
    ];
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| FILES.contains(&n))
    {
        return true;
    }
    path.components()
        .any(|c| c.as_os_str().to_str().is_some_and(|n| DIRS.contains(&n)))
}

pub fn is_cwd_excluded(canon: &Path, cwd: &Path) -> bool {
    canon == cwd || cwd.starts_with(canon)
}

pub fn sort_agent_records(rows: &mut [AgentRecord]) {
    rows.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| age_key(a.age_unix).cmp(&age_key(b.age_unix)))
            .then_with(|| a.path.cmp(&b.path))
    });
}

fn age_key(age_unix: i64) -> i64 {
    if age_unix == 0 {
        i64::MAX
    } else {
        age_unix
    }
}

fn kind_word(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Container => "container",
        AgentKind::Session => "session",
        AgentKind::Cache => "cache",
    }
}

fn source_word(source: AgentSource) -> &'static str {
    match source {
        AgentSource::Cursor => "cursor",
        AgentSource::Codex => "codex",
        AgentSource::Claude => "claude",
        AgentSource::Repo => "repo",
    }
}

pub fn format_agent_label(row: &AgentRecord) -> String {
    let blockers = if row.blockers.is_empty() {
        "-".to_string()
    } else {
        row.blockers.join(",")
    };
    format!(
        "{} {} blockers={blockers} {}",
        kind_word(row.kind),
        source_word(row.source),
        row.path.display()
    )
}

pub fn agent_id(kind: AgentKind, canon: &Path) -> String {
    format!("agent:{}:{}", kind_word(kind), canon.display())
}

const COVERAGE_NOTE: &str =
    "agent scan skips git checkouts claimed by worktree; positive removal verdicts are out of scope.";

const DU_TIMEOUT: Duration = Duration::from_millis(800);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentPlanError {
    #[error("HOME not usable: {0}")]
    Home(String),
}

pub struct AgentPlanOptions<'a> {
    pub home: &'a Path,
    pub cwd: &'a Path,
    pub ttl_secs: u64,
    pub now: SystemTime,
    pub search_roots: Option<&'a [PathBuf]>,
    pub budget: Duration,
    pub per_root: Duration,
    pub git: &'a dyn GitProbe,
    pub size_probe: Option<Arc<dyn PathSizeKb>>,
}

pub fn build_agent_plan(
    protection: &AppProtection,
    opts: &AgentPlanOptions<'_>,
) -> Result<ProtoPlan, AgentPlanError> {
    if !opts.home.is_absolute() {
        return Err(AgentPlanError::Home(opts.home.display().to_string()));
    }

    let deadline = Instant::now() + opts.budget;
    let claimed = collect_worktree_claimed_paths(opts.home, opts.cwd, opts.git, opts.search_roots);
    let cwd_canon = opts
        .cwd
        .canonicalize()
        .unwrap_or_else(|_| opts.cwd.to_path_buf());

    let search = match opts.search_roots {
        Some(r) => r.to_vec(),
        None => super::purge_plan::resolve_search_roots(opts.home),
    };
    let repos = discover_git_repos(&search);

    let mut scan_roots = vec![
        opts.home.join(".cursor"),
        opts.home.join(".codex"),
        opts.home.join(".claude"),
    ];
    for repo in &repos {
        scan_roots.push(repo.join(".cursor"));
        scan_roots.push(repo.join(".claude"));
    }

    let mut records: Vec<AgentRecord> = Vec::new();
    let mut seen = BTreeSet::new();

    for root in scan_roots {
        if Instant::now() >= deadline {
            break;
        }
        let root_deadline = Instant::now() + opts.per_root;
        let mut root_rows = Vec::new();
        let mut timed_out = false;
        for (kind, path) in expand_allowlist(&root) {
            if Instant::now() >= deadline || Instant::now() >= root_deadline {
                timed_out = true;
                break;
            }
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
            if is_cwd_excluded(&canon, &cwd_canon) {
                continue;
            }
            if claimed.contains(&canon) {
                continue;
            }
            if looks_like_git_checkout(&canon) {
                continue;
            }
            if canon.components().any(|c| {
                let n = c.as_os_str();
                n == "worktrees" || n == ".worktrees"
            }) {
                continue;
            }
            if !seen.insert(canon.clone()) {
                continue;
            }
            let (size, age_unix, blockers) = measure_candidate(&canon, opts, deadline, root_deadline);
            if Instant::now() >= deadline || Instant::now() >= root_deadline {
                timed_out = true;
                break;
            }
            root_rows.push(AgentRecord {
                path: canon,
                kind,
                source: source_for_path(opts.home, &path),
                size,
                age_unix,
                blockers,
            });
        }
        if timed_out {
            continue;
        }
        records.extend(root_rows);
    }

    sort_agent_records(&mut records);

    let mut entries = Vec::new();
    for row in records {
        if let Some(entry) = record_to_entry(&row, protection) {
            entries.push(entry);
        }
    }

    Ok(ProtoPlan {
        schema_version: SCHEMA_VERSION,
        created_at: opts.now,
        ttl_secs: opts.ttl_secs,
        entries,
        coverage_note: Some(COVERAGE_NOTE.to_string()),
    })
}

fn measure_candidate(
    path: &Path,
    opts: &AgentPlanOptions<'_>,
    deadline: Instant,
    root_deadline: Instant,
) -> (u64, i64, Vec<String>) {
    let mut blockers = Vec::new();
    let meta = fs::symlink_metadata(path);
    let age_unix = match &meta {
        Ok(m) => m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        Err(_) => {
            blockers.push("status-unknown".into());
            0
        }
    };

    let size = if Instant::now() >= deadline || Instant::now() >= root_deadline {
        push_status_unknown(&mut blockers);
        0
    } else if let Some(probe) = &opts.size_probe {
        let timeout = opts.per_root.min(DU_TIMEOUT);
        match probe.size_kb(path, timeout) {
            Some(kb) => kb.saturating_mul(1024),
            None => {
                push_status_unknown(&mut blockers);
                0
            }
        }
    } else {
        0
    };

    (size, age_unix, blockers)
}

fn push_status_unknown(blockers: &mut Vec<String>) {
    if !blockers.iter().any(|b| b == "status-unknown") {
        blockers.push("status-unknown".into());
    }
}

fn record_to_entry(row: &AgentRecord, protection: &AppProtection) -> Option<ProtoPlanEntry> {
    let path_str = row.path.display().to_string();
    validate_path_for_deletion(&path_str, protection).ok()?;
    let identity = capture_plan_entry_identity(&row.path).ok()?;
    Some(ProtoPlanEntry {
        id: agent_id(row.kind, &row.path),
        path: row.path.clone(),
        label: format_agent_label(row),
        size: row.size,
        rule_id: rule_id_for(row.kind).to_string(),
        skip_reason: None,
        dev: identity.dev,
        ino: identity.ino,
        mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
        blockers: row.blockers.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn rule_ids_are_stable() {
        assert_eq!(rule_id_for(AgentKind::Container), "agent:container");
        assert_eq!(rule_id_for(AgentKind::Session), "agent:session");
        assert_eq!(rule_id_for(AgentKind::Cache), "agent:cache");
    }

    #[test]
    fn expand_allowlist_lists_named_paths_only() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("projects/alpha")).unwrap();
        std::fs::create_dir_all(root.path().join("projects/alpha/agent-transcripts")).unwrap();
        std::fs::create_dir_all(root.path().join("Cache")).unwrap();
        std::fs::create_dir_all(root.path().join("worktrees/checkout")).unwrap();
        std::fs::create_dir_all(root.path().join("worktrees/checkout/.git")).unwrap();
        std::fs::create_dir_all(root.path().join("extensions/foo")).unwrap();
        std::fs::write(root.path().join("auth.json"), b"{}").unwrap();
        std::fs::create_dir_all(root.path().join("mystery")).unwrap();
        let got = expand_allowlist(root.path());
        let paths: Vec<_> = got
            .iter()
            .map(|(_, p)| p.strip_prefix(root.path()).unwrap().to_path_buf())
            .collect();
        assert!(paths.contains(&PathBuf::from("projects/alpha")));
        assert!(paths.contains(&PathBuf::from("projects/alpha/agent-transcripts")));
        assert!(paths.contains(&PathBuf::from("Cache")));
        assert!(!paths.iter().any(|p| p.starts_with("worktrees")));
        assert!(!paths.iter().any(|p| p.starts_with("extensions")));
        assert!(!paths.iter().any(|p| p.ends_with("auth.json")));
        assert!(!paths.iter().any(|p| p == Path::new("mystery")));
    }

    #[test]
    fn cwd_inside_candidate_is_excluded() {
        let cand = PathBuf::from("/Users/me/.cursor/projects/app");
        let cwd = PathBuf::from("/Users/me/.cursor/projects/app/src");
        assert!(is_cwd_excluded(&cand, &cwd));
        assert!(is_cwd_excluded(&cand, &cand));
        assert!(!is_cwd_excluded(
            &PathBuf::from("/Users/me/.cursor/Cache"),
            &cwd
        ));
    }

    #[test]
    fn sort_prefers_large_then_old() {
        let mut rows = vec![
            AgentRecord {
                path: PathBuf::from("/b"),
                kind: AgentKind::Cache,
                source: AgentSource::Cursor,
                size: 10,
                age_unix: 100,
                blockers: vec!["status-unknown".into()],
            },
            AgentRecord {
                path: PathBuf::from("/a"),
                kind: AgentKind::Cache,
                source: AgentSource::Cursor,
                size: 50,
                age_unix: 200,
                blockers: vec![],
            },
            AgentRecord {
                path: PathBuf::from("/c"),
                kind: AgentKind::Cache,
                source: AgentSource::Cursor,
                size: 50,
                age_unix: 50,
                blockers: vec![],
            },
        ];
        sort_agent_records(&mut rows);
        assert_eq!(rows[0].path, PathBuf::from("/c"));
        assert_eq!(rows[1].path, PathBuf::from("/a"));
        assert_eq!(rows[2].path, PathBuf::from("/b"));
    }

    #[test]
    fn label_and_id_have_no_safe_words() {
        let row = AgentRecord {
            path: PathBuf::from("/Users/me/.cursor/Cache"),
            kind: AgentKind::Cache,
            source: AgentSource::Cursor,
            size: 1,
            age_unix: 1,
            blockers: vec!["status-unknown".into()],
        };
        let label = format_agent_label(&row);
        assert!(label.starts_with("cache cursor blockers=status-unknown "));
        assert!(!label.contains("safe"));
        assert!(!label.contains("deletable"));
        assert_eq!(
            agent_id(AgentKind::Cache, Path::new("/Users/me/.cursor/Cache")),
            "agent:cache:/Users/me/.cursor/Cache"
        );
    }

    #[test]
    fn build_plan_lists_cache_and_skips_checkout_and_cwd() {
        use crate::ops::worktree_plan::LiveGitProbe;
        use crate::protection::AppProtection;
        use std::sync::Arc;
        use std::time::{Duration, SystemTime};

        let home = tempfile::tempdir().unwrap();
        let cursor = home.path().join(".cursor");
        std::fs::create_dir_all(cursor.join("Cache")).unwrap();
        std::fs::create_dir_all(cursor.join("worktrees/feat/.git")).unwrap();
        std::fs::create_dir_all(cursor.join("projects/demo")).unwrap();
        let cwd = cursor.join("projects/demo");
        std::fs::write(cursor.join("auth.json"), b"{}").unwrap();

        let git = LiveGitProbe;
        let opts = AgentPlanOptions {
            home: home.path(),
            cwd: &cwd,
            ttl_secs: DEFAULT_AGENT_TTL_SECS,
            now: SystemTime::now(),
            search_roots: Some(&[]),
            budget: Duration::from_secs(15),
            per_root: Duration::from_secs(2),
            git: &git,
            size_probe: Some(Arc::new(crate::ops::clean_hints::DuPathSize)),
        };
        let plan = build_agent_plan(&AppProtection::new(), &opts).unwrap();
        assert_eq!(plan.schema_version, 1);
        assert_eq!(plan.ttl_secs, 900);
        let json = serde_json::to_string(&plan).unwrap();
        assert!(!json.contains("safe"));
        assert!(!json.contains("deletable"));
        assert!(plan
            .entries
            .iter()
            .any(|e| e.rule_id == "agent:cache" && e.path.ends_with("Cache")));
        assert!(!plan.entries.iter().any(|e| e.path.ends_with("auth.json")));
        assert!(!plan
            .entries
            .iter()
            .any(|e| e.path.to_string_lossy().contains("worktrees")));
        assert!(!plan.entries.iter().any(|e| e.path == cwd));
        assert!(plan
            .coverage_note
            .as_deref()
            .unwrap()
            .contains("worktree"));
    }

    #[test]
    fn zero_budget_returns_empty_plan_not_partial_root() {
        use crate::ops::worktree_plan::LiveGitProbe;
        use crate::protection::AppProtection;
        use std::time::{Duration, SystemTime};

        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".cursor/Cache")).unwrap();
        let cwd = home.path().join("cwd");
        std::fs::create_dir_all(&cwd).unwrap();
        let opts = AgentPlanOptions {
            home: home.path(),
            cwd: &cwd,
            ttl_secs: 900,
            now: SystemTime::now(),
            search_roots: Some(&[]),
            budget: Duration::ZERO,
            per_root: Duration::from_secs(2),
            git: &LiveGitProbe,
            size_probe: None,
        };
        let plan = build_agent_plan(&AppProtection::new(), &opts).unwrap();
        assert!(plan.entries.is_empty());
    }
}
