//! `mole_delete`（对齐 mole `file_ops.sh`）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use thiserror::Error;
use vole_sys::Trash;

use crate::oplog::OperationLogger;
use crate::safety::{
    validate_path_for_deletion, verify_plan_entry, PathProtection, PlanEntryIdentity,
    PlanVerifyError,
};

use super::config::{dry_run_enabled, test_no_auth, DeleteMode, DeleteModeParseError};
use super::deletion_log::DeletionLogger;
use super::safe_remove::{
    safe_remove, safe_remove_symlink, FsRemover, SafeRemoveError, SafeRemoveOptions,
};
use super::size::{measure_path_size_bytes, measure_path_size_kb, size_kb_field};
use super::trash::{move_to_trash, TrashMoveError};

static INVALID_MODE_WARNED: AtomicBool = AtomicBool::new(false);
static TRASH_UNAVAILABLE_WARNED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoleDeleteError {
    #[error("invalid delete mode")]
    InvalidMode,
    #[error("path rejected by policy")]
    Rejected,
    #[error("whitelisted path")]
    Whitelisted,
    #[error("plan identity mismatch")]
    IdentityMismatch,
    #[error("path vanished")]
    Vanished,
    #[error("sudo required but blocked in test mode")]
    SudoBlockedTestMode,
    #[error("trash unavailable")]
    TrashUnavailable,
    #[error("safe remove failed")]
    SafeRemove(#[from] SafeRemoveError),
}

/// 删除成功时回传实测体积，供 Report 分口径统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteOutcome {
    pub bytes: u64,
}

pub struct MoleDeleteOptions {
    pub mode: DeleteMode,
    pub dry_run: bool,
    pub needs_sudo: bool,
}

impl Default for MoleDeleteOptions {
    fn default() -> Self {
        Self {
            mode: DeleteMode::Permanent,
            dry_run: dry_run_enabled(),
            needs_sudo: false,
        }
    }
}

impl MoleDeleteOptions {
    pub fn from_env() -> Result<Self, DeleteModeParseError> {
        Ok(Self {
            mode: super::config::delete_mode_from_env()?,
            dry_run: dry_run_enabled(),
            needs_sudo: false,
        })
    }
}

pub fn mole_delete(
    path: &str,
    protection: &dyn PathProtection,
    whitelist_patterns: &[String],
    options: MoleDeleteOptions,
    trash: &dyn Trash,
    deletion_log: &DeletionLogger,
    oplog: &mut OperationLogger,
) -> Result<(), MoleDeleteError> {
    mole_delete_inner(
        path,
        None,
        protection,
        whitelist_patterns,
        options,
        trash,
        deletion_log,
        oplog,
    )
    .map(|_| ())
}

/// apply 路径：在删除 syscall 前立刻重验 `(dev, ino, mtime)`，缩小 TOCTOU 窗口。
#[allow(clippy::too_many_arguments)]
pub fn mole_delete_verified(
    path: &str,
    expect: &PlanEntryIdentity,
    protection: &dyn PathProtection,
    whitelist_patterns: &[String],
    options: MoleDeleteOptions,
    trash: &dyn Trash,
    deletion_log: &DeletionLogger,
    oplog: &mut OperationLogger,
) -> Result<DeleteOutcome, MoleDeleteError> {
    mole_delete_inner(
        path,
        Some(expect),
        protection,
        whitelist_patterns,
        options,
        trash,
        deletion_log,
        oplog,
    )
}

#[allow(clippy::too_many_arguments)]
fn mole_delete_inner(
    path: &str,
    expect: Option<&PlanEntryIdentity>,
    protection: &dyn PathProtection,
    whitelist_patterns: &[String],
    options: MoleDeleteOptions,
    trash: &dyn Trash,
    deletion_log: &DeletionLogger,
    oplog: &mut OperationLogger,
) -> Result<DeleteOutcome, MoleDeleteError> {
    if path.is_empty() {
        return Err(MoleDeleteError::Rejected);
    }

    let mode_label = match options.mode {
        DeleteMode::Permanent => "permanent",
        DeleteMode::Trash => "trash",
    };

    if !path_exists_for_mole_delete(path) {
        // 无身份约束时保持 mole 兼容 no-op；apply 身份路径必须报 vanished。
        return if expect.is_some() {
            Err(MoleDeleteError::Vanished)
        } else {
            Ok(DeleteOutcome { bytes: 0 })
        };
    }

    if validate_path_for_deletion(path, protection).is_err() {
        deletion_log.log(mode_label, "0", "rejected", path);
        return Err(MoleDeleteError::Rejected);
    }

    if let Some(identity) = expect {
        // 紧挨删除前重验，避免 rename 竞态把受保护对象换到计划路径上。
        if let Err(err) = verify_plan_entry(path, identity) {
            deletion_log.log(mode_label, "0", "identity-mismatch", path);
            return Err(map_verify_error(err));
        }
    }

    if options.needs_sudo {
        if test_no_auth() {
            let size = size_kb_field(measure_path_size_kb(path));
            deletion_log.log(mode_label, &size, "sudo-blocked-test-mode", path);
            return Err(MoleDeleteError::SudoBlockedTestMode);
        }
        deletion_log.log(mode_label, "unknown", "sudo-not-implemented", path);
        return Err(MoleDeleteError::SudoBlockedTestMode);
    }

    let size = measure_path_size_kb(path);
    let size_field = size_kb_field(size);
    let bytes = measure_path_size_bytes(path).unwrap_or(0);

    if options.dry_run {
        deletion_log.log(mode_label, &size_field, "dry-run", path);
        return Ok(DeleteOutcome { bytes });
    }

    // 再验一次：测量与删除之间仍可能被替换。
    if let Some(identity) = expect {
        if let Err(err) = verify_plan_entry(path, identity) {
            deletion_log.log(mode_label, &size_field, "identity-mismatch", path);
            return Err(map_verify_error(err));
        }
    }

    if options.mode == DeleteMode::Trash {
        mole_delete_trash(path, &size_field, trash, deletion_log, oplog)?;
        return Ok(DeleteOutcome { bytes });
    }

    mole_delete_permanent(
        path,
        protection,
        whitelist_patterns,
        mode_label,
        &size_field,
        deletion_log,
        oplog,
    )?;
    Ok(DeleteOutcome { bytes })
}

fn map_verify_error(err: PlanVerifyError) -> MoleDeleteError {
    match err {
        PlanVerifyError::StatFailed(_) => MoleDeleteError::Vanished,
        PlanVerifyError::InodeMismatch
        | PlanVerifyError::MtimeMismatch
        | PlanVerifyError::CrossDevice
        | PlanVerifyError::SegmentOpen(_) => MoleDeleteError::IdentityMismatch,
        _ => MoleDeleteError::IdentityMismatch,
    }
}

fn mole_delete_trash(
    path: &str,
    size_field: &str,
    trash: &dyn Trash,
    deletion_log: &DeletionLogger,
    oplog: &mut OperationLogger,
) -> Result<(), MoleDeleteError> {
    let path_buf = PathBuf::from(path);
    let trash_detail = if size_field == "unknown" {
        "unknown".to_string()
    } else {
        format!("{size_field}KB")
    };
    match move_to_trash(&path_buf, trash, Duration::from_secs(30)) {
        Ok(()) => {
            deletion_log.log("trash", size_field, "ok", path);
            oplog.log("TRASHED", &path_buf, Some(&trash_detail)).ok();
            Ok(())
        }
        Err(TrashMoveError::BlockedTestMode) | Err(TrashMoveError::Io(_)) => {
            deletion_log.log("trash", size_field, "trash-failed", path);
            oplog.log("SKIPPED", &path_buf, Some("trash-failed")).ok();
            warn_trash_unavailable_once();
            Err(MoleDeleteError::TrashUnavailable)
        }
    }
}

fn mole_delete_permanent(
    path: &str,
    protection: &dyn PathProtection,
    whitelist_patterns: &[String],
    mode_label: &str,
    size_field: &str,
    deletion_log: &DeletionLogger,
    oplog: &mut OperationLogger,
) -> Result<(), MoleDeleteError> {
    let path_buf = Path::new(path);
    let result = if path_buf.is_symlink() {
        safe_remove_symlink(path, protection, whitelist_patterns, false, oplog)
    } else {
        safe_remove(
            path,
            protection,
            whitelist_patterns,
            SafeRemoveOptions {
                silent: true,
                dry_run: false,
                ..Default::default()
            },
            oplog,
            &FsRemover,
        )
    };

    let status = if result.is_ok() { "ok" } else { "error" };
    deletion_log.log(mode_label, size_field, status, path);
    match result {
        Ok(()) => Ok(()),
        Err(SafeRemoveError::Whitelisted) => Err(MoleDeleteError::Whitelisted),
        Err(e) => Err(MoleDeleteError::SafeRemove(e)),
    }
}

fn path_exists_for_mole_delete(path: &str) -> bool {
    let path = Path::new(path);
    path.exists()
        || path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
}

fn warn_trash_unavailable_once() {
    if TRASH_UNAVAILABLE_WARNED.swap(true, Ordering::SeqCst) {
        return;
    }
    eprintln!(
        "Error: Trash unavailable; refusing permanent delete. Retry after fixing Trash permissions, or rescan."
    );
}

pub fn warn_invalid_delete_mode_once(mode: &str) {
    if INVALID_MODE_WARNED.swap(true, Ordering::SeqCst) {
        return;
    }
    eprintln!("Error: invalid MOLE_DELETE_MODE: {mode} (expected \"permanent\" or \"trash\")");
}

pub fn mole_delete_with_env_mode(
    path: &str,
    protection: &dyn PathProtection,
    whitelist_patterns: &[String],
    trash: &dyn Trash,
    deletion_log: &DeletionLogger,
    oplog: &mut OperationLogger,
) -> Result<(), MoleDeleteError> {
    let options = match MoleDeleteOptions::from_env() {
        Ok(opts) => opts,
        Err(DeleteModeParseError::Invalid(mode)) => {
            deletion_log.log(&mode, "unknown", "invalid-mode", path);
            warn_invalid_delete_mode_once(&mode);
            return Err(MoleDeleteError::InvalidMode);
        }
    };
    mole_delete(
        path,
        protection,
        whitelist_patterns,
        options,
        trash,
        deletion_log,
        oplog,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protection::AppProtection;
    use crate::test_env;
    use std::fs;
    use vole_sys::macos::MacTrash;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vole-mole-delete-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn permanent_mode_removes_target() {
        let _guard = test_env::lock();
        let root = scratch("permanent");
        let victim = root.join("victim");
        fs::create_dir_all(&victim).unwrap();
        fs::write(victim.join("keep.txt"), b"x").unwrap();
        let log_path = root.join("deletions.log");
        let trash_dir = root.join("Trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        std::env::set_var("MOLE_DELETE_LOG", &log_path);

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(log_path.clone());
        let mut oplog = OperationLogger::new("uninstall");
        let options = MoleDeleteOptions {
            mode: DeleteMode::Permanent,
            dry_run: false,
            needs_sudo: false,
        };

        mole_delete(
            &victim.to_string_lossy(),
            &protection,
            &[],
            options,
            &MacTrash,
            &deletion_log,
            &mut oplog,
        )
        .unwrap();

        assert!(!victim.exists());
        let text = fs::read_to_string(&log_path).unwrap();
        assert!(text.contains("\tpermanent\t"));
        assert!(text.contains("\tok\t"));

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn trash_mode_moves_to_test_trash() {
        let _guard = test_env::lock();
        let root = scratch("trash");
        let victim = root.join("victim_trash");
        fs::create_dir_all(&victim).unwrap();
        fs::write(victim.join("data.txt"), b"x").unwrap();
        let log_path = root.join("deletions.log");
        let trash_dir = root.join("Trash");
        fs::create_dir_all(&trash_dir).unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash_dir);
        std::env::set_var("MOLE_DELETE_LOG", &log_path);

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(log_path);
        let mut oplog = OperationLogger::new("uninstall");
        let options = MoleDeleteOptions {
            mode: DeleteMode::Trash,
            dry_run: false,
            needs_sudo: false,
        };

        mole_delete(
            &victim.to_string_lossy(),
            &protection,
            &[],
            options,
            &MacTrash,
            &deletion_log,
            &mut oplog,
        )
        .unwrap();

        assert!(!victim.exists());
        assert!(fs::read_dir(&trash_dir).unwrap().next().is_some());

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_invalid_paths_and_logs() {
        let _guard = test_env::lock();
        let root = scratch("reject");
        let log_path = root.join("deletions.log");
        std::env::set_var("MOLE_DELETE_LOG", &log_path);
        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(log_path.clone());
        let mut oplog = OperationLogger::new("uninstall");
        let options = MoleDeleteOptions::default();

        assert_eq!(
            mole_delete(
                "/tmp/../etc/hosts",
                &protection,
                &[],
                options,
                &MacTrash,
                &deletion_log,
                &mut oplog,
            ),
            Err(MoleDeleteError::Rejected)
        );
        let text = fs::read_to_string(&log_path).unwrap();
        assert!(text.contains("\trejected\t"));

        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn dry_run_does_not_remove() {
        let _guard = test_env::lock();
        let root = scratch("dry");
        let victim = root.join("victim");
        fs::write(&victim, b"x").unwrap();
        let log_path = root.join("deletions.log");
        std::env::set_var("MOLE_DELETE_LOG", &log_path);

        let protection = AppProtection::new();
        let deletion_log = DeletionLogger::with_path(log_path.clone());
        let mut oplog = OperationLogger::new("uninstall");
        let options = MoleDeleteOptions {
            mode: DeleteMode::Permanent,
            dry_run: true,
            needs_sudo: false,
        };

        mole_delete(
            &victim.to_string_lossy(),
            &protection,
            &[],
            options,
            &MacTrash,
            &deletion_log,
            &mut oplog,
        )
        .unwrap();
        assert!(victim.exists());
        let text = fs::read_to_string(&log_path).unwrap();
        assert!(text.contains("\tdry-run\t"));

        std::env::remove_var("MOLE_DELETE_LOG");
        fs::remove_dir_all(&root).ok();
    }
}
