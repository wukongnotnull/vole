//! Git worktree 盘点：porcelain 解析、来源判定、启发式排序。

use std::path::{Path, PathBuf};

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
}
