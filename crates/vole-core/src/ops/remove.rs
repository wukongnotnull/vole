//! 自卸载 `remove`（对照 Mole `lib/manage/remove.sh`）。

use std::path::{Path, PathBuf};

use crate::delete::{safe_remove, FsRemover, SafeRemoveOptions};
use crate::oplog::OperationLogger;
use crate::ops::install_origin::{detect_install_layout, InstallOrigin};
use crate::safety::NoPathProtection;

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
        for dir in ["Library/Logs/vole", "Library/Logs/mole"] {
            for name in ["operations.log", "deletions.log"] {
                let p = opts.home.join(dir).join(name);
                if p.is_file() {
                    push_unique_path(&mut items, &mut seen, RemoveItemKind::Oplog, p);
                }
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

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RemoveError {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug)]
pub enum RemoveOutcome {
    DryRun(RemovePlan),
    NothingFound,
    Removed {
        plan: RemovePlan,
        errors: Vec<String>,
    },
    NeedsConfirmation,
}

pub trait BrewUninstaller {
    fn uninstall_vole(&self) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct FakeBrewUninstaller {
    pub calls: std::sync::Mutex<u32>,
    pub fail: bool,
}

impl BrewUninstaller for FakeBrewUninstaller {
    fn uninstall_vole(&self) -> Result<(), String> {
        *self.calls.lock().unwrap() += 1;
        if self.fail {
            Err("fake brew failed".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Default)]
pub struct LiveBrewUninstaller;

impl BrewUninstaller for LiveBrewUninstaller {
    fn uninstall_vole(&self) -> Result<(), String> {
        let brew = find_brew_cmd().ok_or_else(|| {
            "Homebrew command not found. Manual step: brew uninstall vole".to_string()
        })?;
        let output = std::process::Command::new(&brew)
            .args(["uninstall", "vole"])
            .output()
            .map_err(|e| format!("failed to run brew: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "Homebrew uninstallation failed: {stderr}. Manual step: brew uninstall vole"
            ))
        }
    }
}

fn find_brew_cmd() -> Option<PathBuf> {
    if let Ok(path) = which_brew_on_path() {
        return Some(path);
    }
    for candidate in ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"] {
        let p = PathBuf::from(candidate);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn which_brew_on_path() -> Result<PathBuf, ()> {
    let output = std::process::Command::new("/bin/sh")
        .args(["-c", "command -v brew"])
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        Err(())
    } else {
        Ok(PathBuf::from(path))
    }
}

pub fn run_remove(
    opts: &RemoveOptions,
    brew: &dyn BrewUninstaller,
) -> Result<RemoveOutcome, RemoveError> {
    let plan = plan_remove(opts);
    if opts.dry_run {
        return Ok(RemoveOutcome::DryRun(plan));
    }
    if plan.items.is_empty() {
        return Ok(RemoveOutcome::NothingFound);
    }
    if !opts.yes {
        return Ok(RemoveOutcome::NeedsConfirmation);
    }

    let mut errors = Vec::new();
    if plan.homebrew {
        if let Err(e) = brew.uninstall_vole() {
            errors.push(e);
        }
    }

    let mut logger = OperationLogger::new("remove");
    let _ = logger.session_start();
    let protection = NoPathProtection;
    let remover = FsRemover;
    let mut removed = 0u64;

    for item in &plan.items {
        let Some(path) = &item.path else {
            continue;
        };
        if path_forbidden_for_self_remove(path) {
            errors.push(format!(
                "refusing to delete Homebrew Cellar path: {}",
                path.display()
            ));
            continue;
        }
        let path_str = path.to_string_lossy();
        match safe_remove(
            &path_str,
            &protection,
            &[],
            SafeRemoveOptions {
                silent: false,
                precomputed_size_kb: None,
                dry_run: false,
            },
            &mut logger,
            &remover,
        ) {
            Ok(()) => removed += 1,
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }

    let _ = logger.session_end(removed, 0);
    Ok(RemoveOutcome::Removed { plan, errors })
}

fn path_forbidden_for_self_remove(path: &Path) -> bool {
    path_is_cellar_vole(path) || path.to_string_lossy().contains("/Cellar/vole/")
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

    #[test]
    fn apply_deletes_manual_via_funnel_not_raw_rm() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let local_bin = home.join(".local/bin");
        fs::create_dir_all(&local_bin).unwrap();
        let exe = local_bin.join("vole");
        fs::write(&exe, b"x").unwrap();
        fs::create_dir_all(home.join(".config/vole")).unwrap();
        fs::write(
            home.join(".config/vole/install_channel"),
            b"CHANNEL=stable\n",
        )
        .unwrap();
        let opts = RemoveOptions {
            dry_run: false,
            yes: true,
            purge_oplog: false,
            binary_path: exe.clone(),
            home: home.clone(),
            config_dir: home.join(".config/vole"),
        };
        let brew = FakeBrewUninstaller::default();
        let out = run_remove(&opts, &brew).unwrap();
        match out {
            RemoveOutcome::Removed { errors, .. } => assert!(errors.is_empty(), "{errors:?}"),
            other => panic!("unexpected {other:?}"),
        }
        assert!(!exe.exists());
        assert!(!home.join(".config/vole").exists());
        assert_eq!(*brew.calls.lock().unwrap(), 0);
    }

    #[test]
    fn apply_brew_calls_uninstaller_not_cellar_delete() {
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
            dry_run: false,
            yes: true,
            purge_oplog: false,
            binary_path: link,
            home: home.clone(),
            config_dir: home.join(".config/vole"),
        };
        let brew = FakeBrewUninstaller::default();
        let out = run_remove(&opts, &brew).unwrap();
        match out {
            RemoveOutcome::Removed { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(*brew.calls.lock().unwrap(), 1);
        assert!(real.exists(), "must not hand-tear Cellar");
        assert!(!home.join(".config/vole").exists());
    }

    #[test]
    fn rejects_cellar_path_even_if_forced_into_plan() {
        let dir = tempfile::tempdir().unwrap();
        let cellar = dir.path().join("Cellar/vole/2.4.0/bin");
        fs::create_dir_all(&cellar).unwrap();
        let real = cellar.join("vole");
        fs::write(&real, b"x").unwrap();
        assert!(path_forbidden_for_self_remove(&real));
    }
}
