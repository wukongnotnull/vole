use std::io;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("IO: {0}")]
    Io(#[from] io::Error),
    #[error("超时")]
    Timeout,
}

#[derive(Debug, Error)]
pub enum SysCommandError {
    #[error("IO: {0}")]
    Io(#[from] io::Error),
    #[error("超时")]
    Timeout,
    #[error("子进程失败: {0}")]
    Failed(i32),
}

pub trait Fs: Send + Sync {
    fn metadata_len(&self, path: &Path, timeout: Duration) -> Result<u64, FsError>;
}

pub trait SysCommand: Send + Sync {
    fn run(&self, argv: &[&str], timeout: Duration) -> Result<Output, SysCommandError>;
}

pub trait Plist: Send + Sync {
    fn read_file(&self, path: &Path, timeout: Duration) -> Result<plist::Value, io::Error>;
}

pub trait Sqlite: Send + Sync {
    fn query_count(&self, path: &Path, sql: &str, timeout: Duration) -> Result<i64, io::Error>;
}

pub trait Trash: Send + Sync {
    fn trash_path(&self, path: &Path, timeout: Duration) -> Result<(), io::Error>;
}

pub trait Metrics: Send + Sync {
    fn total_disk_bytes(&self) -> u64;
}
