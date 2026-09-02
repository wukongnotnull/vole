use std::path::{Path, PathBuf};

use super::worktree_plan::looks_like_git_checkout;

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
}
