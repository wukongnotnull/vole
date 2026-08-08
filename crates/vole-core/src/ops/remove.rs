//! 自卸载 `remove`（对照 Mole `lib/manage/remove.sh`）。

use std::path::{Path, PathBuf};

use crate::ops::install_origin::{detect_install_layout, InstallOrigin};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveItemKind {
    BrewUninstall,
    ManualBinary,
    ShareTree,
    AliasOrCompletion,
    Config,
    Cache,
    ToolLogs,
    Oplog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveItem {
    pub kind: RemoveItemKind,
    pub path: Option<PathBuf>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovePlan {
    pub items: Vec<RemoveItem>,
    pub homebrew: bool,
}

#[derive(Debug, Clone)]
pub struct RemoveOptions {
    pub dry_run: bool,
    pub yes: bool,
    pub purge_oplog: bool,
    pub binary_path: PathBuf,
    pub home: PathBuf,
    pub config_dir: PathBuf,
}

pub fn plan_remove(opts: &RemoveOptions) -> RemovePlan {
    let layout = detect_install_layout(&opts.binary_path, Some(&opts.config_dir));
    let homebrew = layout.origin == InstallOrigin::Homebrew;
    let mut items = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    if homebrew {
        items.push(RemoveItem {
            kind: RemoveItemKind::BrewUninstall,
            path: None,
            note: Some("brew uninstall vole".into()),
        });
    } else if is_existing_non_cellar_vole(&opts.binary_path) {
        push_unique_path(
            &mut items,
            &mut seen,
            RemoveItemKind::ManualBinary,
            opts.binary_path.clone(),
        );
        if let Some(share) = share_vole_beside_binary(&opts.binary_path) {
            push_unique_path(&mut items, &mut seen, RemoveItemKind::ShareTree, share);
        }
    }

    for candidate in manual_fallback_bins(&opts.home) {
        if paths_equal(&candidate, &opts.binary_path) {
            continue;
        }
        if !is_existing_non_cellar_vole(&candidate) {
            continue;
        }
        push_unique_path(
            &mut items,
            &mut seen,
            RemoveItemKind::ManualBinary,
            candidate.clone(),
        );
        if let Some(share) = share_vole_beside_binary(&candidate) {
            push_unique_path(&mut items, &mut seen, RemoveItemKind::ShareTree, share);
        }
    }

    for candidate in completion_residue_paths(&opts.home) {
        if candidate.is_file() {
            push_unique_path(
                &mut items,
                &mut seen,
                RemoveItemKind::AliasOrCompletion,
                candidate,
            );
        }
    }

    if opts.config_dir.is_dir() {
        push_unique_path(
            &mut items,
            &mut seen,
            RemoveItemKind::Config,
            opts.config_dir.clone(),
        );
    }

    let cache = opts.home.join(".cache/vole");
    if cache.is_dir() {
        push_unique_path(&mut items, &mut seen, RemoveItemKind::Cache, cache);
    }

    let tool_logs = opts.home.join("Library/Logs/vole");
    if tool_logs.is_dir() {
        push_unique_path(&mut items, &mut seen, RemoveItemKind::ToolLogs, tool_logs);
    }

    if opts.purge_oplog {
        for name in ["operations.log", "deletions.log"] {
            let p = opts.home.join("Library/Logs/mole").join(name);
            if p.is_file() {
                push_unique_path(&mut items, &mut seen, RemoveItemKind::Oplog, p);
            }
        }
    }

    RemovePlan { items, homebrew }
}

fn push_unique_path(
    items: &mut Vec<RemoveItem>,
    seen: &mut std::collections::BTreeSet<String>,
    kind: RemoveItemKind,
    path: PathBuf,
) {
    let key = path.to_string_lossy().into_owned();
    if !seen.insert(key) {
        return;
    }
    items.push(RemoveItem {
        kind,
        path: Some(path),
        note: None,
    });
}

fn manual_fallback_bins(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/bin/vole"),
        PathBuf::from("/usr/local/bin/vole"),
        PathBuf::from("/opt/local/bin/vole"),
    ]
}

fn completion_residue_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".config/fish/completions/vole.fish"),
        home.join(".local/share/bash-completion/completions/vole"),
        home.join(".zfunc/_vole"),
    ]
}

fn share_vole_beside_binary(binary: &Path) -> Option<PathBuf> {
    let prefix_bin = binary.parent()?;
    let share = prefix_bin
        .parent()
        .map(|p| p.join("share/vole"))
        .unwrap_or_else(|| prefix_bin.join("share/vole"));
    if share.is_dir() {
        Some(share)
    } else {
        None
    }
}

fn is_existing_non_cellar_vole(path: &Path) -> bool {
    if !path.is_file() && !path.is_symlink() {
        return false;
    }
    if path_is_cellar_vole(path) {
        return false;
    }
    true
}

fn path_is_cellar_vole(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("/Cellar/vole/") {
        return true;
    }
    if path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        if let Ok(target) = std::fs::read_link(path) {
            if target.to_string_lossy().contains("Cellar/vole") {
                return true;
            }
        }
    }
    if let Ok(canon) = std::fs::canonicalize(path) {
        if canon.to_string_lossy().contains("/Cellar/vole/") {
            return true;
        }
    }
    false
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn dry_run_lists_brew_not_cellar_path() {
        let dir = tempfile::tempdir().unwrap();
        let cellar = dir.path().join("Cellar/vole/2.4.0/bin");
        fs::create_dir_all(&cellar).unwrap();
        let real = cellar.join("vole");
        fs::write(&real, b"x").unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let link = bin.join("vole");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(home.join(".config/vole")).unwrap();
        let opts = RemoveOptions {
            dry_run: true,
            yes: true,
            purge_oplog: false,
            binary_path: link,
            home: home.clone(),
            config_dir: home.join(".config/vole"),
        };
        let plan = plan_remove(&opts);
        assert!(plan.homebrew);
        assert!(plan
            .items
            .iter()
            .any(|i| i.kind == RemoveItemKind::BrewUninstall));
        assert!(!plan.items.iter().any(|i| {
            i.path
                .as_ref()
                .is_some_and(|p| p.to_string_lossy().contains("Cellar/vole"))
        }));
        assert!(plan.items.iter().any(|i| i.kind == RemoveItemKind::Config));
    }

    #[test]
    fn dry_run_lists_manual_and_completion_residue() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let local_bin = home.join(".local/bin");
        fs::create_dir_all(&local_bin).unwrap();
        let exe = local_bin.join("vole");
        fs::write(&exe, b"x").unwrap();
        let fish = home.join(".config/fish/completions");
        fs::create_dir_all(&fish).unwrap();
        fs::write(fish.join("vole.fish"), b"#").unwrap();
        let opts = RemoveOptions {
            dry_run: true,
            yes: true,
            purge_oplog: false,
            binary_path: exe.clone(),
            home: home.clone(),
            config_dir: home.join(".config/vole"),
        };
        let plan = plan_remove(&opts);
        assert!(!plan.homebrew);
        assert!(plan
            .items
            .iter()
            .any(|i| i.kind == RemoveItemKind::ManualBinary));
        assert!(plan
            .items
            .iter()
            .any(|i| i.kind == RemoveItemKind::AliasOrCompletion));
    }

    #[test]
    fn oplog_only_when_flag() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let logs = home.join("Library/Logs/mole");
        fs::create_dir_all(&logs).unwrap();
        fs::write(logs.join("operations.log"), b"#").unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("vole");
        fs::write(&exe, b"x").unwrap();
        let mut opts = RemoveOptions {
            dry_run: true,
            yes: true,
            purge_oplog: false,
            binary_path: exe.clone(),
            home: home.clone(),
            config_dir: home.join(".config/vole"),
        };
        assert!(!plan_remove(&opts)
            .items
            .iter()
            .any(|i| i.kind == RemoveItemKind::Oplog));
        opts.purge_oplog = true;
        assert!(plan_remove(&opts)
            .items
            .iter()
            .any(|i| i.kind == RemoveItemKind::Oplog));
    }
}
