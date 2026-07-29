//! 法医审计日志（对齐 mole `_mole_delete_log`）。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use super::config::deletion_log_path;

static LOG_WARNED: AtomicBool = AtomicBool::new(false);

pub struct DeletionLogger {
    path: PathBuf,
}

impl DeletionLogger {
    pub fn from_env() -> Self {
        Self {
            path: deletion_log_path(),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn log(&self, mode: &str, size_kb: &str, status: &str, target: &str) {
        let Some(parent) = self.path.parent() else {
            warn_broken("invalid log path");
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            warn_broken(&format!("create directory: {}", parent.display()));
            return;
        }

        let ts = format_iso_timestamp(SystemTime::now());
        let line = format!("{ts}\t{mode}\t{size_kb}\t{status}\t{target}\n");
        if append_line(&self.path, &line).is_err() {
            warn_broken(&format!("write to: {}", self.path.display()));
        }
    }
}

fn append_line(path: &Path, line: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

fn warn_broken(detail: &str) {
    if LOG_WARNED.swap(true, Ordering::SeqCst) {
        return;
    }
    eprintln!(
        "Warning: deletions audit log unavailable ({detail}). Forensic trail incomplete this session."
    );
}

fn format_iso_timestamp(time: SystemTime) -> String {
    use std::time::UNIX_EPOCH;
    let dur = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, d) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+0000",
        y, mo, d, h, m, s
    )
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut y = 1970;
    let mut d = days;
    while d >= days_in_year(y) {
        d -= days_in_year(y);
        y += 1;
    }
    let mut m = 1;
    while d >= days_in_month(y, m) {
        d -= days_in_month(y, m);
        m += 1;
    }
    (y, m, d + 1)
}

fn is_leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}

fn days_in_year(y: u64) -> u64 {
    if is_leap(y) {
        366
    } else {
        365
    }
}

fn days_in_month(y: u64, m: u64) -> u64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;

    #[test]
    fn writes_tab_separated_line() {
        let _guard = test_env::lock();
        let dir = std::env::temp_dir().join(format!("vole-del-log-{}", std::process::id()));
        let log_path = dir.join("deletions.log");
        let logger = DeletionLogger::with_path(log_path.clone());
        logger.log("trash", "42", "ok", "/tmp/victim");
        let text = fs::read_to_string(&log_path).unwrap();
        assert!(text.contains("\ttrash\t42\tok\t/tmp/victim\n"));
        fs::remove_dir_all(&dir).ok();
    }
}
