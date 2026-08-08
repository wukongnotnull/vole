//! `flock` 进程互斥。

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use rustix::fs::{flock, FlockOperation};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MutexError {
    #[error("另一个 vole 操作正在运行")]
    AlreadyRunning,
    #[error("锁文件 IO 失败: {0}")]
    Io(#[from] io::Error),
    #[error("flock 失败: {0}")]
    Rustix(#[from] rustix::io::Errno),
}

#[allow(dead_code)] // `file` 持有 flock 直到 drop
pub struct CleanLock {
    file: File,
    path: PathBuf,
}

#[allow(dead_code)] // `file` 持有 flock 直到 drop
pub struct ConfigLock {
    file: File,
    path: PathBuf,
}

fn cache_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".cache/vole"))
        .unwrap_or_else(|| PathBuf::from(".cache/vole"))
}

fn try_lock_path(path: &Path) -> Result<File, MutexError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    flock(&file, FlockOperation::NonBlockingLockExclusive)?;
    Ok(file)
}

pub fn try_lock_clean() -> Result<CleanLock, MutexError> {
    let path = cache_dir().join("clean.lock");
    let file = try_lock_path(&path)?;
    Ok(CleanLock { file, path })
}

pub fn try_lock_uninstall() -> Result<ConfigLock, MutexError> {
    try_lock_config("uninstall")
}

pub fn try_lock_optimize() -> Result<ConfigLock, MutexError> {
    try_lock_config("optimize")
}

pub fn try_lock_purge() -> Result<ConfigLock, MutexError> {
    try_lock_config("purge")
}

pub fn try_lock_config(name: &str) -> Result<ConfigLock, MutexError> {
    let path = cache_dir().join(format!("{}.lock", name));
    let file = try_lock_path(&path)?;
    Ok(ConfigLock { file, path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;

    #[test]
    fn second_clean_lock_fails_nonblocking() {
        let _guard = test_env::lock();
        let dir = std::env::temp_dir().join(format!("vole-mutex-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::env::set_var("HOME", dir.join("home"));
        let _a = try_lock_clean().expect("first lock");
        let b = try_lock_clean();
        assert!(matches!(b, Err(MutexError::Rustix(_))));
        std::env::remove_var("HOME");
        std::fs::remove_dir_all(&dir).ok();
    }
}
