//! 操作日志，对齐 Mole `lib/core/log.sh`。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct OperationLogger {
    command: String,
    path: PathBuf,
    enabled: bool,
}

impl OperationLogger {
    pub fn new(command: &str) -> Self {
        let enabled = !oplog_disabled();
        let path = crate::user_paths::operations_log_write_path();
        if enabled {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
        }
        Self {
            command: command.to_string(),
            path,
            enabled,
        }
    }

    pub fn session_start(&mut self) -> io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let ts = format_timestamp(SystemTime::now());
        self.append_line("")?;
        self.append_line(format!(
            "# ========== {} session started at {} ==========",
            self.command, ts
        ))?;
        Ok(())
    }

    pub fn session_end(&mut self, items: u64, size_kb: u64) -> io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let ts = format_timestamp(SystemTime::now());
        let size_human = mole_session_size_human(size_kb);
        self.append_line(format!(
            "# ========== {} session ended at {}, {} items, {} ==========",
            self.command, ts, items, size_human
        ))?;
        Ok(())
    }

    pub fn log(&mut self, action: &str, path: &Path, detail: Option<&str>) -> io::Result<()> {
        if !self.enabled || path.as_os_str().is_empty() {
            return Ok(());
        }
        let ts = format_timestamp(SystemTime::now());
        let mut line = format!("[{}] [{}] {} {}", ts, self.command, action, path.display());
        if let Some(d) = detail {
            line.push_str(&format!(" ({})", d));
        }
        self.append_line(line)?;
        Ok(())
    }

    pub fn log_path(&self) -> &Path {
        &self.path
    }

    fn append_line(&mut self, line: impl AsRef<str>) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", line.as_ref())?;
        Ok(())
    }
}

fn mole_session_size_human(size_kb: u64) -> String {
    // 对齐 mole `bytes_to_human`（base-10，见 lib/core/base.sh）
    let bytes = size_kb * 1024;
    if bytes >= 1_000_000_000 {
        let scaled = (bytes * 100 + 500_000_000) / 1_000_000_000;
        format!("{}.{:02}GB", scaled / 100, scaled % 100)
    } else if bytes >= 1_000_000 {
        let scaled = (bytes * 10 + 500_000) / 1_000_000;
        format!("{}.{:01}MB", scaled / 10, scaled % 10)
    } else if bytes >= 1000 {
        format!("{}KB", (bytes + 500) / 1000)
    } else if bytes > 0 {
        format!("{}B", bytes)
    } else {
        "0B".to_string()
    }
}

fn oplog_disabled() -> bool {
    std::env::var_os("MO_NO_OPLOG").is_some_and(|v| v == "1")
        || std::env::var_os("VOLE_NO_OPLOG").is_some_and(|v| v == "1")
}

fn format_timestamp(time: SystemTime) -> String {
    use std::time::UNIX_EPOCH;
    let dur = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    // 对齐 mole 本地时间格式 YYYY-MM-DD HH:MM:SS（测试环境用 UTC 固定格式亦可解析）
    let secs = dur.as_secs();
    // 简单分解，不引入 chrono
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, m, s)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // 1970-01-01 起算日序转 civil date（足够 oplog / history 解析）
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
    use std::path::Path;

    #[test]
    fn writes_removed_line() {
        let _guard = test_env::lock();
        let home = std::env::temp_dir().join(format!("vole-oplog-{}", std::process::id()));
        std::env::set_var("HOME", home.join("h"));
        let mut log = OperationLogger::new("clean");
        log.session_start().unwrap();
        log.log("REMOVED", Path::new("/tmp/x"), Some("1KB"))
            .unwrap();
        let text = std::fs::read_to_string(log.log_path()).unwrap();
        assert!(text.contains("[clean] REMOVED /tmp/x (1KB)"));
        std::env::remove_var("HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    /// 由 `scripts/verify-oplog-mole.sh` 调用；写入固定 fixture 供 `mo history` 解析。
    #[test]
    #[ignore]
    fn mole_verify_fixture() {
        let _guard = test_env::lock();
        std::env::remove_var("VOLE_NO_OPLOG");
        std::env::remove_var("MO_NO_OPLOG");
        let mut log = OperationLogger::new("clean");
        log.session_start().unwrap();
        log.log("REMOVED", Path::new("/tmp/vole-verify"), Some("1KB"))
            .unwrap();
        log.session_end(1, 1).unwrap();
    }

    #[test]
    fn disabled_by_env() {
        let _guard = test_env::lock();
        let home = std::env::temp_dir().join(format!("vole-oplog-off-{}", std::process::id()));
        std::env::set_var("HOME", home.join("h"));
        std::env::set_var("VOLE_NO_OPLOG", "1");
        let mut log = OperationLogger::new("clean");
        log.log("REMOVED", Path::new("/tmp/x"), None).unwrap();
        assert!(!log.log_path().exists());
        std::env::remove_var("HOME");
        std::env::remove_var("VOLE_NO_OPLOG");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn writes_to_vole_logs_dir() {
        let _guard = test_env::lock();
        let home = std::env::temp_dir().join(format!("vole-oplog-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("h")).unwrap();
        std::env::remove_var("VOLE_OPERATIONS_LOG");
        std::env::remove_var("MOLE_OPERATIONS_LOG");
        std::env::remove_var("OPERATIONS_LOG_FILE");
        std::env::remove_var("VOLE_NO_OPLOG");
        std::env::remove_var("MO_NO_OPLOG");
        std::env::set_var("HOME", home.join("h"));
        let mut log = OperationLogger::new("clean");
        log.log("REMOVED", Path::new("/tmp/x"), Some("1KB"))
            .unwrap();
        let path = log.log_path();
        assert!(
            path.ends_with("Library/Logs/vole/operations.log"),
            "{}",
            path.display()
        );
        assert!(path.is_file());
        std::env::remove_var("HOME");
        std::fs::remove_dir_all(&home).ok();
    }
}
