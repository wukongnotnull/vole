//! `safe_remove`（对齐 mole `file_ops.sh`）。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::oplog::OperationLogger;
use crate::safety::{validate_path_for_deletion, PathProtection, ValidationError};
use crate::whitelist;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SafeRemoveError {
    #[error("path validation failed")]
    ValidationFailed,
    #[error("validation: {0}")]
    Validation(#[from] ValidationError),
    #[error("whitelisted path")]
    Whitelisted,
    #[error("IO: {0}")]
    Io(String),
    #[error("interrupted")]
    Interrupted(i32),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SafeRemoveOptions {
    pub silent: bool,
    pub precomputed_size_kb: Option<u64>,
    pub dry_run: bool,
}

pub trait PathRemover: Send + Sync {
    fn remove(&self, path: &Path) -> io::Result<()>;
}

#[derive(Debug, Default)]
pub struct FsRemover;

impl PathRemover for FsRemover {
    fn remove(&self, path: &Path) -> io::Result<()> {
        remove_path(path)
    }
}

#[derive(Debug)]
pub struct ShellRemover;

impl PathRemover for ShellRemover {
    fn remove(&self, path: &Path) -> io::Result<()> {
        let status = Command::new("rm").arg("-rf").arg(path).status()?;
        let code = status.code().unwrap_or(1);
        if code == 0 {
            Ok(())
        } else if code >= 128 {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                format!("rm interrupted with exit {code}"),
            ))
        } else {
            Err(io::Error::other(format!("rm failed with exit {code}")))
        }
    }
}

pub fn safe_remove(
    path: &str,
    protection: &dyn PathProtection,
    whitelist_patterns: &[String],
    options: SafeRemoveOptions,
    logger: &mut OperationLogger,
    remover: &dyn PathRemover,
) -> Result<(), SafeRemoveError> {
    if options.silent {
        if validate_path_for_deletion(path, protection).is_err() {
            return Err(SafeRemoveError::ValidationFailed);
        }
    } else {
        validate_path_for_deletion(path, protection)?;
    }

    if whitelist::is_match(Path::new(path), whitelist_patterns) {
        logger
            .log("SKIPPED", Path::new(path), Some("whitelist"))
            .map_err(|e| SafeRemoveError::Io(e.to_string()))?;
        return Err(SafeRemoveError::Whitelisted);
    }

    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Ok(());
    }

    if options.dry_run {
        return Ok(());
    }

    let size_human = size_human_for_log(path, options.precomputed_size_kb);

    match remover.remove(&path_buf) {
        Ok(()) => {
            logger
                .log("REMOVED", &path_buf, size_human.as_deref())
                .map_err(|e| SafeRemoveError::Io(e.to_string()))?;
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::Interrupted => {
            let code = 130;
            Err(SafeRemoveError::Interrupted(code))
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("Permission denied") || msg.contains("Operation not permitted") {
                logger
                    .log("FAILED", &path_buf, Some("permission denied"))
                    .map_err(|e| SafeRemoveError::Io(e.to_string()))?;
            } else if !options.silent {
                logger
                    .log("FAILED", &path_buf, Some("error"))
                    .map_err(|e| SafeRemoveError::Io(e.to_string()))?;
            }
            Err(SafeRemoveError::Io(msg))
        }
    }
}

pub fn safe_remove_symlink(
    path: &str,
    protection: &dyn PathProtection,
    whitelist_patterns: &[String],
    dry_run: bool,
    logger: &mut OperationLogger,
) -> Result<(), SafeRemoveError> {
    let path_buf = PathBuf::from(path);
    if !path_buf.is_symlink() {
        return Err(SafeRemoveError::Io("not a symlink".into()));
    }

    validate_path_for_deletion(path, protection)?;

    if whitelist::is_match(&path_buf, whitelist_patterns) {
        logger
            .log("SKIPPED", &path_buf, Some("whitelist"))
            .map_err(|e| SafeRemoveError::Io(e.to_string()))?;
        return Err(SafeRemoveError::Whitelisted);
    }

    if dry_run {
        return Ok(());
    }

    fs::remove_file(&path_buf).map_err(|e| SafeRemoveError::Io(e.to_string()))?;
    logger
        .log("REMOVED", &path_buf, Some("symlink"))
        .map_err(|e| SafeRemoveError::Io(e.to_string()))?;
    Ok(())
}

fn remove_path(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn size_human_for_log(path: &str, precomputed_kb: Option<u64>) -> Option<String> {
    if let Some(kb) = precomputed_kb {
        if kb > 0 {
            return Some(format!("{kb}KB"));
        }
        return None;
    }
    let path = Path::new(path);
    if !path.exists() {
        return None;
    }
    fs::metadata(path)
        .ok()
        .map(|m| m.len())
        .filter(|&len| len > 0)
        .map(|len| format!("{}KB", len.div_ceil(1024)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protection::AppProtection;
    use crate::safety::NoPathProtection;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingRemover {
        calls: Arc<AtomicUsize>,
        fail_with: Option<i32>,
    }

    impl PathRemover for CountingRemover {
        fn remove(&self, _path: &Path) -> io::Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(code) = self.fail_with {
                if code >= 128 {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        format!("exit {code}"),
                    ));
                }
                return Err(io::Error::other(format!("exit {code}")));
            }
            Ok(())
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vole-safe-remove-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn validates_before_deletion() {
        let p = NoPathProtection;
        let mut log = OperationLogger::new("clean");
        let remover = FsRemover;
        assert_eq!(
            safe_remove(
                "/System/test",
                &p,
                &[],
                SafeRemoveOptions {
                    silent: true,
                    ..Default::default()
                },
                &mut log,
                &remover
            ),
            Err(SafeRemoveError::ValidationFailed)
        );
    }

    #[test]
    fn removes_file_and_directory() {
        let dir = scratch("ok");
        let file = dir.join("file.txt");
        fs::write(&file, b"x").unwrap();
        let sub = dir.join("subdir");
        fs::create_dir_all(sub.join("inner")).unwrap();

        let protection = AppProtection::new();
        let mut log = OperationLogger::new("clean");
        let remover = FsRemover;

        safe_remove(
            &file.to_string_lossy(),
            &protection,
            &[],
            SafeRemoveOptions::default(),
            &mut log,
            &remover,
        )
        .unwrap();
        assert!(!file.exists());

        safe_remove(
            &sub.to_string_lossy(),
            &protection,
            &[],
            SafeRemoveOptions::default(),
            &mut log,
            &remover,
        )
        .unwrap();
        assert!(!sub.exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_path_is_ok() {
        let dir = scratch("missing");
        let protection = AppProtection::new();
        let mut log = OperationLogger::new("clean");
        safe_remove(
            &dir.join("nope").to_string_lossy(),
            &protection,
            &[],
            SafeRemoveOptions::default(),
            &mut log,
            &FsRemover,
        )
        .unwrap();
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preserves_interrupt_exit_codes() {
        let dir = scratch("interrupt");
        let file = dir.join("file.txt");
        fs::write(&file, b"x").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let remover = CountingRemover {
            calls: calls.clone(),
            fail_with: Some(130),
        };
        let protection = AppProtection::new();
        let mut log = OperationLogger::new("clean");
        assert_eq!(
            safe_remove(
                &file.to_string_lossy(),
                &protection,
                &[],
                SafeRemoveOptions::default(),
                &mut log,
                &remover,
            ),
            Err(SafeRemoveError::Interrupted(130))
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(file.exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn symlink_remove_keeps_target() {
        let dir = scratch("symlink");
        let target = dir.join("target.txt");
        fs::write(&target, b"keep").unwrap();
        let link = dir.join("link");
        symlink(&target, &link).unwrap();
        let protection = AppProtection::new();
        let mut log = OperationLogger::new("clean");
        safe_remove_symlink(&link.to_string_lossy(), &protection, &[], false, &mut log).unwrap();
        assert!(!link.exists());
        assert!(target.exists());
        fs::remove_dir_all(&dir).ok();
    }
}
