//! Optimize `disk_verify`（对齐 Mole `opt_disk_verify`）。
//!
//! 默认不执行：须 `VOLE_ENABLE_DISK_VERIFY=1`。仅 `diskutil verifyVolume /`，
//! **禁止** `repairVolume` / `repairDisk`。`VOLE_TEST_NO_AUTH` 下永不真跑 diskutil。

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use super::delete_paths::OptimizeCandidate;
use crate::delete::test_no_auth;
use crate::optimize::OptimizeTaskKind;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskVerifyError {
    TestMode,
    Unavailable,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskVerifyOutcome {
    Ok,
    Issues,
    Complete,
}

pub trait DiskVerifyDeps: Send + Sync {
    fn verify_root_volume(&self) -> Result<DiskVerifyOutcome, DiskVerifyError>;
}

pub struct LiveDiskVerifyDeps;

impl DiskVerifyDeps for LiveDiskVerifyDeps {
    fn verify_root_volume(&self) -> Result<DiskVerifyOutcome, DiskVerifyError> {
        if test_no_auth() {
            return Err(DiskVerifyError::TestMode);
        }
        live_verify_volume()
    }
}

pub fn disk_verify_enabled() -> bool {
    match std::env::var("VOLE_ENABLE_DISK_VERIFY") {
        Ok(v) => matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => false,
    }
}

fn timeout_secs() -> u64 {
    std::env::var("VOLE_TIMEOUT_DISK_VERIFY_SEC")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
}

pub fn plan_disk_verify(home: &Path, deps: &dyn DiskVerifyDeps) -> Vec<OptimizeCandidate> {
    let _ = deps; // plan 不跑 verify；deps 保留以便测试/未来扩展
    if test_no_auth() || !disk_verify_enabled() {
        return Vec::new();
    }
    vec![OptimizeCandidate {
        path: home.join(".vole-optimize-action/disk_verify"),
        label: "Disk Health".into(),
        size: 0,
        task_id: "disk_verify",
        kind: OptimizeTaskKind::Action,
    }]
}

pub fn run_disk_verify(deps: &dyn DiskVerifyDeps) -> Result<DiskVerifyOutcome, DiskVerifyError> {
    if test_no_auth() {
        return Err(DiskVerifyError::TestMode);
    }
    if !disk_verify_enabled() {
        return Ok(DiskVerifyOutcome::Complete);
    }
    deps.verify_root_volume()
}

fn live_verify_volume() -> Result<DiskVerifyOutcome, DiskVerifyError> {
    let mut cmd = Command::new("diskutil");
    cmd.arg("verifyVolume")
        .arg("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|_| DiskVerifyError::Unavailable)?;
    let timeout = Duration::from_secs(timeout_secs());
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|_| DiskVerifyError::Unavailable)?;
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                return Ok(classify_verify_output(&text));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(DiskVerifyError::TimedOut);
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return Err(DiskVerifyError::Unavailable),
        }
    }
}

pub fn classify_verify_output(text: &str) -> DiskVerifyOutcome {
    let lower = text.to_ascii_lowercase();
    if lower.contains("appears to be ok") || lower.contains("volume appears to be ok") {
        return DiskVerifyOutcome::Ok;
    }
    if lower.contains("error") || lower.contains("corrupt") || lower.contains("invalid") {
        return DiskVerifyOutcome::Issues;
    }
    DiskVerifyOutcome::Complete
}

pub struct FakeDiskVerifyDeps {
    pub outcome: Mutex<Result<DiskVerifyOutcome, DiskVerifyError>>,
    pub calls: Mutex<u32>,
}

impl Default for FakeDiskVerifyDeps {
    fn default() -> Self {
        Self {
            outcome: Mutex::new(Ok(DiskVerifyOutcome::Ok)),
            calls: Mutex::new(0),
        }
    }
}

impl DiskVerifyDeps for FakeDiskVerifyDeps {
    fn verify_root_volume(&self) -> Result<DiskVerifyOutcome, DiskVerifyError> {
        *self.calls.lock().unwrap() += 1;
        self.outcome.lock().unwrap().clone()
    }
}

#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn plan_empty_without_opt_in() {
        let _guard = test_env_lock();
        std::env::remove_var("VOLE_ENABLE_DISK_VERIFY");
        std::env::remove_var("VOLE_TEST_NO_AUTH");
        let home = tempdir().unwrap();
        let fake = FakeDiskVerifyDeps::default();
        assert!(plan_disk_verify(home.path(), &fake).is_empty());
        assert_eq!(*fake.calls.lock().unwrap(), 0);
    }

    #[test]
    fn plan_sentinel_when_enabled() {
        let _guard = test_env_lock();
        std::env::set_var("VOLE_ENABLE_DISK_VERIFY", "1");
        std::env::remove_var("VOLE_TEST_NO_AUTH");
        let home = tempdir().unwrap();
        let fake = FakeDiskVerifyDeps::default();
        let plan = plan_disk_verify(home.path(), &fake);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].task_id, "disk_verify");
        assert!(plan[0].path.ends_with(".vole-optimize-action/disk_verify"));
        assert_eq!(plan[0].label, "Disk Health");
        std::env::remove_var("VOLE_ENABLE_DISK_VERIFY");
    }

    #[test]
    fn plan_empty_under_test_no_auth() {
        let _guard = test_env_lock();
        std::env::set_var("VOLE_ENABLE_DISK_VERIFY", "1");
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        let home = tempdir().unwrap();
        let fake = FakeDiskVerifyDeps::default();
        assert!(plan_disk_verify(home.path(), &fake).is_empty());
        std::env::remove_var("VOLE_ENABLE_DISK_VERIFY");
        std::env::remove_var("VOLE_TEST_NO_AUTH");
    }

    #[test]
    fn run_noop_without_opt_in() {
        let _guard = test_env_lock();
        std::env::remove_var("VOLE_ENABLE_DISK_VERIFY");
        std::env::remove_var("VOLE_TEST_NO_AUTH");
        let fake = FakeDiskVerifyDeps {
            outcome: Mutex::new(Ok(DiskVerifyOutcome::Ok)),
            ..Default::default()
        };
        assert_eq!(run_disk_verify(&fake).unwrap(), DiskVerifyOutcome::Complete);
        assert_eq!(*fake.calls.lock().unwrap(), 0);
    }

    #[test]
    fn run_uses_deps_when_enabled() {
        let _guard = test_env_lock();
        std::env::set_var("VOLE_ENABLE_DISK_VERIFY", "1");
        std::env::remove_var("VOLE_TEST_NO_AUTH");
        let fake = FakeDiskVerifyDeps {
            outcome: Mutex::new(Ok(DiskVerifyOutcome::Issues)),
            ..Default::default()
        };
        assert_eq!(run_disk_verify(&fake).unwrap(), DiskVerifyOutcome::Issues);
        assert_eq!(*fake.calls.lock().unwrap(), 1);
        std::env::remove_var("VOLE_ENABLE_DISK_VERIFY");
    }

    #[test]
    fn run_timeout_is_error() {
        let _guard = test_env_lock();
        std::env::set_var("VOLE_ENABLE_DISK_VERIFY", "1");
        std::env::remove_var("VOLE_TEST_NO_AUTH");
        let fake = FakeDiskVerifyDeps {
            outcome: Mutex::new(Err(DiskVerifyError::TimedOut)),
            ..Default::default()
        };
        assert_eq!(run_disk_verify(&fake), Err(DiskVerifyError::TimedOut));
        std::env::remove_var("VOLE_ENABLE_DISK_VERIFY");
    }

    #[test]
    fn run_test_no_auth_is_test_mode() {
        let _guard = test_env_lock();
        std::env::set_var("VOLE_ENABLE_DISK_VERIFY", "1");
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        let fake = FakeDiskVerifyDeps::default();
        assert_eq!(run_disk_verify(&fake), Err(DiskVerifyError::TestMode));
        assert_eq!(*fake.calls.lock().unwrap(), 0);
        std::env::remove_var("VOLE_ENABLE_DISK_VERIFY");
        std::env::remove_var("VOLE_TEST_NO_AUTH");
    }

    #[test]
    fn classify_ok_and_issues() {
        assert_eq!(
            classify_verify_output("File system check exit code is 0\nVolume appears to be OK"),
            DiskVerifyOutcome::Ok
        );
        assert_eq!(
            classify_verify_output("error: corrupt catalog detected"),
            DiskVerifyOutcome::Issues
        );
        assert_eq!(
            classify_verify_output("something else"),
            DiskVerifyOutcome::Complete
        );
    }

    #[test]
    fn source_forbids_repair_command_invocations() {
        let src = include_str!("disk_verify.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(!prod.contains("arg(\"repair"));
        let repair = format!("{}{}", "repair", "Volume");
        let repair_disk = format!("{}{}", "repair", "Disk");
        assert!(!prod.contains(&format!("\"{repair}\"")));
        assert!(!prod.contains(&format!("\"{repair_disk}\"")));
        assert!(prod.contains("verifyVolume"));
    }
}
