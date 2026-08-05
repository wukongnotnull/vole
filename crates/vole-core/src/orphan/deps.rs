//! 可注入的 orphan 外部依赖（安装扫描 / Spotlight / mdfind）。

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::installed::{collect_launch_agent_ids, scan_app_dirs_for_bundle_ids};
use super::MAX_MDFIND_CALLS;

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// Orphan 判定所需的外部探针。
pub trait OrphanDeps: Send + Sync {
    fn spotlight_available(&self) -> bool;
    /// `Ok(true)` = 找到安装；`Ok(false)` = Spotlight 可用且明确空；`Err` = 超时/失败/预算耗尽 → fail-closed。
    fn mdfind_bundle(&self, bundle_id: &str) -> Result<bool, ()>;
    /// `Err` = 扫描失败（不可当成零安装）。
    fn scan_installed_bundle_ids(&self, home: &Path) -> Result<HashSet<String>, ()>;
}

/// 可单测的 mdfind 调用预算。
#[derive(Debug, Default)]
pub struct MdfindBudget {
    count: AtomicUsize,
    limit: usize,
}

impl MdfindBudget {
    pub fn new(limit: usize) -> Self {
        Self {
            count: AtomicUsize::new(0),
            limit,
        }
    }

    pub fn try_consume(&self) -> bool {
        let prev = self.count.fetch_add(1, Ordering::Relaxed);
        prev < self.limit
    }

    pub fn calls(&self) -> usize {
        self.count.load(Ordering::Relaxed).min(self.limit + 1)
    }
}

/// 生产实现：真机扫描 + mdfind/mdutil/lsappinfo。
pub struct LiveOrphanDeps {
    budget: MdfindBudget,
    mdfind_cache: Mutex<HashMap<String, Result<bool, ()>>>,
}

impl Default for LiveOrphanDeps {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveOrphanDeps {
    pub fn new() -> Self {
        Self {
            budget: MdfindBudget::new(MAX_MDFIND_CALLS),
            mdfind_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn mdfind_calls(&self) -> usize {
        self.budget.calls()
    }
}

impl OrphanDeps for LiveOrphanDeps {
    fn spotlight_available(&self) -> bool {
        live_spotlight_available()
    }

    fn mdfind_bundle(&self, bundle_id: &str) -> Result<bool, ()> {
        if let Ok(guard) = self.mdfind_cache.lock() {
            if let Some(cached) = guard.get(bundle_id) {
                return cached.clone();
            }
        }
        if !self.budget.try_consume() {
            return Err(());
        }
        let result = live_mdfind_bundle(bundle_id);
        if let Ok(mut guard) = self.mdfind_cache.lock() {
            // 超时/错误不写入「未找到」缓存（对齐 Mole）。
            if result.is_ok() {
                guard.insert(bundle_id.to_string(), result);
            }
        }
        result
    }

    fn scan_installed_bundle_ids(&self, home: &Path) -> Result<HashSet<String>, ()> {
        let mut set = scan_app_dirs_for_bundle_ids(home)?;
        set.extend(collect_launch_agent_ids(home));
        set.extend(live_running_bundle_ids());
        Ok(set)
    }
}

/// 测试用假依赖。
#[derive(Debug, Clone, Default)]
pub struct FakeOrphanDeps {
    pub spotlight: bool,
    pub installed: HashSet<String>,
    pub mdfind: HashMap<String, Result<bool, ()>>,
    pub scan_error: bool,
}

impl OrphanDeps for FakeOrphanDeps {
    fn spotlight_available(&self) -> bool {
        self.spotlight
    }

    fn mdfind_bundle(&self, bundle_id: &str) -> Result<bool, ()> {
        self.mdfind.get(bundle_id).copied().unwrap_or(Ok(false))
    }

    fn scan_installed_bundle_ids(&self, _home: &Path) -> Result<HashSet<String>, ()> {
        if self.scan_error {
            return Err(());
        }
        Ok(self.installed.clone())
    }
}

fn live_spotlight_available() -> bool {
    let mut cmd = std::process::Command::new("mdutil");
    cmd.args(["-s", "/"]);
    let output = run_command_timeout(cmd, PROBE_TIMEOUT);
    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            let err = String::from_utf8_lossy(&out.stderr).to_ascii_lowercase();
            let combined = format!("{text}{err}");
            !combined.contains("disabled")
        }
        Err(_) => false,
    }
}

fn live_mdfind_bundle(bundle_id: &str) -> Result<bool, ()> {
    // 仅允许相对安全的 reverse-DNS 字符，避免 query 注入。
    if !bundle_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err(());
    }
    let query = format!("kMDItemCFBundleIdentifier == '{bundle_id}'");
    let mut cmd = std::process::Command::new("mdfind");
    cmd.arg(query);
    let output = run_command_timeout(cmd, PROBE_TIMEOUT).map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|l| !l.trim().is_empty()))
}

fn live_running_bundle_ids() -> HashSet<String> {
    let mut out = HashSet::new();
    let mut cmd = std::process::Command::new("lsappinfo");
    cmd.arg("list");
    let Ok(output) = run_command_timeout(cmd, PROBE_TIMEOUT) else {
        return out;
    };
    if !output.status.success() {
        return out;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(rest) = line.split("CFBundleIdentifier\"=\"").nth(1) {
            if let Some(id) = rest.split('"').next() {
                if !id.is_empty() {
                    out.insert(id.to_string());
                }
            }
        }
    }
    out
}

fn run_command_timeout(
    mut cmd: std::process::Command,
    timeout: Duration,
) -> std::io::Result<std::process::Output> {
    let handle = std::thread::spawn(move || cmd.output());
    let start = Instant::now();
    while !handle.is_finished() {
        if start.elapsed() >= timeout {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "orphan probe timeout",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    handle
        .join()
        .map_err(|_| std::io::Error::other("orphan probe thread panicked"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn fake_deps_installed_set_used_as_is() {
        let mut installed = HashSet::new();
        installed.insert("com.keep.me".into());
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed,
            mdfind: HashMap::new(),
            scan_error: false,
        };
        assert!(deps
            .scan_installed_bundle_ids(Path::new("/tmp"))
            .unwrap()
            .contains("com.keep.me"));
    }

    #[test]
    fn mdfind_budget_returns_false_after_cap() {
        let budget = MdfindBudget::new(MAX_MDFIND_CALLS);
        for _ in 0..MAX_MDFIND_CALLS {
            assert!(budget.try_consume());
        }
        assert!(!budget.try_consume());
    }

    #[test]
    fn fake_scan_error_propagates() {
        let deps = FakeOrphanDeps {
            scan_error: true,
            ..Default::default()
        };
        assert!(deps.scan_installed_bundle_ids(Path::new("/tmp")).is_err());
    }
}
