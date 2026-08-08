//! `purge` plan：项目构建物发现 → ProtoPlan（`rule_id` 前缀 `purge:`）。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::protection::AppProtection;
use crate::safety::{capture_plan_entry_identity, validate_path_for_deletion, PathProtection};
use crate::vole_proto::{Plan as ProtoPlan, PlanEntry as ProtoPlanEntry, SCHEMA_VERSION};

/// Mole `MOLE_PURGE_TARGETS`（钉版 1.48.1）原样钉死。
pub const PURGE_TARGETS: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    "venv",
    ".venv",
    ".pytest_cache",
    ".mypy_cache",
    ".tox",
    ".nox",
    ".ruff_cache",
    ".gradle",
    ".terragrunt-cache",
    "__pycache__",
    ".next",
    ".nuxt",
    ".output",
    "vendor",
    "bin",
    "obj",
    ".turbo",
    ".parcel-cache",
    ".dart_tool",
    ".zig-cache",
    "zig-out",
    ".angular",
    ".svelte-kit",
    ".astro",
    "coverage",
    "DerivedData",
    "Pods",
    ".cxx",
    ".expo",
    ".build",
];

/// Mole 默认搜索根相对 `$HOME` 的片段（不含裸 `$HOME`）。
const DEFAULT_SEARCH_REL: &[&str] = &[
    "www",
    "dev",
    "Projects",
    "GitHub",
    "Code",
    "Workspace",
    "Repos",
    "Development",
    "Library/CloudStorage",
    ".codex/worktrees",
    ".claude/worktrees",
];

const PROJECT_INDICATORS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "requirements.txt",
    "pom.xml",
    "build.gradle",
    "Gemfile",
    "composer.json",
    "Package.swift",
    "Makefile",
    ".git",
];

const MAX_DEPTH: usize = 6;
pub const DEFAULT_PURGE_MIN_AGE_DAYS: u64 = 7;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PurgePlanError {
    #[error("HOME not usable: {0}")]
    Home(String),
}

pub struct PurgePlanOptions<'a> {
    pub home: &'a Path,
    pub ttl_secs: u64,
    /// 测试注入：覆盖搜索根；`None` 则用默认 + `$HOME/*/` 容器探测。
    pub search_roots: Option<&'a [PathBuf]>,
    pub include_empty: bool,
    pub min_age_days: u64,
    pub now: SystemTime,
}

pub fn build_purge_plan(
    protection: &AppProtection,
    opts: &PurgePlanOptions<'_>,
) -> Result<ProtoPlan, PurgePlanError> {
    if !opts.home.is_absolute() {
        return Err(PurgePlanError::Home(opts.home.display().to_string()));
    }

    let roots = match opts.search_roots {
        Some(r) => r.to_vec(),
        None => resolve_search_roots(opts.home),
    };

    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();

    for root in &roots {
        if !root.is_dir() {
            continue;
        }
        for candidate in discover_under_root(root, opts) {
            let Ok(canon) = candidate.canonicalize() else {
                continue;
            };
            if !seen.insert(canon.clone()) {
                continue;
            }
            if is_protected_purge_artifact(&canon) {
                continue;
            }
            if !opts.include_empty && dir_is_empty(&canon) {
                continue;
            }
            let target = canon
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("artifact");
            let rule_id = format!("purge:{target}");
            if let Some(entry) = try_plan_entry(&canon, target, &rule_id, protection) {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(ProtoPlan {
        schema_version: SCHEMA_VERSION,
        created_at: opts.now,
        ttl_secs: opts.ttl_secs,
        entries,
        coverage_note: Some(
            "purge long-tail skipped: TTY multi-select UI; full Mole activity classifier; \
cloud sync interactive confirm (fail-closed skip uncertain ages)."
                .into(),
        ),
    })
}

fn resolve_search_roots(home: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = BTreeSet::new();

    let config = home.join(".config/vole/purge_paths");
    if let Ok(text) = fs::read_to_string(&config) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let p = if line.starts_with('/') {
                PathBuf::from(line)
            } else {
                home.join(line)
            };
            if p.is_dir() && seen.insert(p.clone()) {
                roots.push(p);
            }
        }
        if !roots.is_empty() {
            return roots;
        }
    }

    for rel in DEFAULT_SEARCH_REL {
        let p = home.join(rel);
        if p.is_dir() && seen.insert(p.clone()) {
            roots.push(p);
        }
    }

    if let Ok(rd) = fs::read_dir(home) {
        for ent in rd.flatten() {
            let p = ent.path();
            if !p.is_dir() {
                continue;
            }
            let name = ent.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            if matches!(
                name.as_ref(),
                "Library"
                    | "Applications"
                    | "Movies"
                    | "Music"
                    | "Pictures"
                    | "Public"
                    | "Downloads"
            ) {
                continue;
            }
            if is_project_container(&p) && seen.insert(p.clone()) {
                roots.push(p);
            }
        }
    }

    roots
}

fn is_project_container(dir: &Path) -> bool {
    for depth in 0..=2 {
        if dir_has_indicator_at_depth(dir, depth) {
            return true;
        }
    }
    false
}

fn dir_has_indicator_at_depth(dir: &Path, depth: usize) -> bool {
    if depth == 0 {
        return PROJECT_INDICATORS.iter().any(|ind| dir.join(ind).exists());
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() && dir_has_indicator_at_depth(&p, depth - 1) {
            return true;
        }
    }
    false
}

fn discover_under_root(root: &Path, opts: &PurgePlanOptions<'_>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let walker = jwalk::WalkDir::new(root)
        .max_depth(MAX_DEPTH)
        .skip_hidden(false);
    for entry in walker.into_iter().flatten() {
        let path = entry.path();
        if !entry.file_type().is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !PURGE_TARGETS.contains(&name) {
            continue;
        }
        if !ancestor_has_project_indicator(&path, root) {
            continue;
        }
        if !is_old_enough(&path, opts.min_age_days, opts.now) {
            continue;
        }
        out.push(path);
    }
    out
}

fn ancestor_has_project_indicator(path: &Path, root: &Path) -> bool {
    let mut cur = path.parent();
    while let Some(dir) = cur {
        if PROJECT_INDICATORS.iter().any(|ind| dir.join(ind).exists()) {
            return true;
        }
        if dir == root {
            break;
        }
        if !dir.starts_with(root) {
            break;
        }
        cur = dir.parent();
    }
    // 搜索根本身是项目根时也算
    PROJECT_INDICATORS.iter().any(|ind| root.join(ind).exists())
}

fn is_old_enough(path: &Path, min_age_days: u64, now: SystemTime) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(age) = now.duration_since(modified) else {
        return false;
    };
    age >= Duration::from_secs(min_age_days.saturating_mul(86_400))
}

/// 对齐 Mole `is_protected_purge_artifact`：返回 true = 保护（不可进 plan）。
pub fn is_protected_purge_artifact(path: &Path) -> bool {
    let Some(base) = path.file_name().and_then(|s| s.to_str()) else {
        return true;
    };
    match base {
        "bin" => !is_dotnet_bin_dir(path),
        "vendor" => is_protected_vendor_dir(path),
        "DerivedData" => path
            .to_string_lossy()
            .contains("/Library/Developer/Xcode/DerivedData"),
        _ => false,
    }
}

fn is_dotnet_bin_dir(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(rd) = fs::read_dir(parent) else {
        return false;
    };
    rd.flatten().any(|e| {
        let n = e.file_name();
        let s = n.to_string_lossy();
        s.ends_with(".csproj") || s.ends_with(".sln") || s.ends_with(".fsproj")
    })
}

fn is_protected_vendor_dir(path: &Path) -> bool {
    // Mole：非 Composer 的 vendor 受保护。有 composer.json 才可 purge。
    let Some(parent) = path.parent() else {
        return true;
    };
    !parent.join("composer.json").exists()
}

fn dir_is_empty(path: &Path) -> bool {
    fs::read_dir(path)
        .map(|mut rd| rd.next().is_none())
        .unwrap_or(true)
}

fn try_plan_entry(
    path: &Path,
    label: &str,
    rule_id: &str,
    protection: &dyn PathProtection,
) -> Option<ProtoPlanEntry> {
    let path_str = path.display().to_string();
    validate_path_for_deletion(&path_str, protection).ok()?;
    let identity = capture_plan_entry_identity(path).ok()?;
    let size = path_size_shallow(path);
    Some(ProtoPlanEntry {
        id: format!("{rule_id}:{}", path.display()),
        path: path.to_path_buf(),
        label: label.to_string(),
        size,
        rule_id: rule_id.to_string(),
        skip_reason: None,
        dev: identity.dev,
        ino: identity.ino,
        mtime: UNIX_EPOCH + Duration::from_secs(identity.mtime.max(0) as u64),
    })
}

fn path_size_shallow(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    let mut total = 0u64;
    let walker = jwalk::WalkDir::new(path).max_depth(3).skip_hidden(false);
    for entry in walker.into_iter().flatten() {
        if let Ok(m) = entry.metadata() {
            if m.is_file() {
                total = total.saturating_add(m.len());
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;

    use crate::protection::AppProtection;

    fn age_path(path: &Path, days: u64) {
        let modified = SystemTime::now()
            .checked_sub(Duration::from_secs(days * 86_400))
            .unwrap();
        fs::File::open(path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(modified))
            .unwrap();
    }

    #[test]
    fn plan_includes_old_node_modules_skips_fresh_target() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let project = home.join("Projects/demo");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("package.json"), b"{}").unwrap();

        let old_nm = project.join("node_modules");
        fs::create_dir_all(old_nm.join("leftpad")).unwrap();
        fs::write(old_nm.join("leftpad/index.js"), b"x").unwrap();
        age_path(&old_nm, 14);

        let fresh = project.join("target");
        fs::create_dir_all(&fresh).unwrap();
        fs::write(fresh.join("x"), b"y").unwrap();
        // fresh: default mtime = now → skipped

        let protection = AppProtection::new();
        let roots = [home.join("Projects")];
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
            plan.entries
                .iter()
                .any(|e| e.rule_id == "purge:node_modules" && e.path.ends_with("node_modules")),
            "entries={:?}",
            plan.entries
        );
        assert!(
            !plan.entries.iter().any(|e| e.rule_id == "purge:target"),
            "fresh target must be skipped"
        );
    }

    #[test]
    fn protects_non_dotnet_bin() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        assert!(is_protected_purge_artifact(&bin));

        let proj = dir.path().join("proj");
        fs::create_dir_all(&proj).unwrap();
        fs::write(proj.join("App.csproj"), b"<Project/>").unwrap();
        let dotnet_bin = proj.join("bin");
        fs::create_dir_all(&dotnet_bin).unwrap();
        assert!(!is_protected_purge_artifact(&dotnet_bin));
    }
}
