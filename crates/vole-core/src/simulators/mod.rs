//! Xcode 不可用模拟器（unavailable simulators）清理。
//!
//! 对齐 Mole `clean_dev_mobile`：plan 用 `xcrun simctl list devices unavailable`
//! 列出 UDID，只把 `~/Library/Developer/CoreSimulator/Devices/<UDID>` 仍存在的条目
//! 选入候选；apply 重新列一次确认仍不可用，再交给 `xcrun simctl delete <UDID>`。
//! 模拟器状态归 simctl 管：simctl 失败时绝不回退为直接删目录。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use vole_sys::macos::MacSysCommand;
use vole_sys::SysCommand;

/// `xcode-unavailable-simulators` 规则 id。
pub const XCODE_UNAVAILABLE_SIMULATORS_RULE_ID: &str = "xcode-unavailable-simulators";

/// simctl 探测与删除依赖（测试可注入）。
pub trait SimctlDeps: Send + Sync {
    /// 用户域 `CoreSimulator/Devices` 根目录。
    fn devices_root(&self) -> PathBuf;
    /// `simctl list devices unavailable` stdout；simctl 不可用、失败或超时为 `None`。
    fn list_unavailable(&self) -> Option<String>;
    /// `simctl delete <udid>`。
    fn delete_device(&self, udid: &str) -> Result<(), String>;
}

/// 是否为 simctl UDID 形状（8-4-4-4-12 十六进制）。
pub fn is_simulator_udid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8usize, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// 从 `simctl list devices unavailable` 输出解析 UDID。
///
/// 行形如 `    iPhone 8 (0A1B...) (Shutdown) (unavailable, runtime profile not found)`；
/// 取 `(unavailable` 之前最后一个 UDID 形状的括号段（同 Mole 的贪婪 sed）。
pub fn parse_unavailable_udids(output: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in output.lines() {
        let Some(cut) = line.find("(unavailable") else {
            continue;
        };
        let head = &line[..cut];
        let udid = head
            .split('(')
            .skip(1)
            .filter_map(|seg| seg.split_once(')').map(|(inner, _)| inner.trim()))
            .filter(|inner| is_simulator_udid(inner))
            .last();
        if let Some(udid) = udid {
            if !out.iter().any(|u: &String| u.eq_ignore_ascii_case(udid)) {
                out.push(udid.to_string());
            }
        }
    }
    out
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}

/// plan 候选：仍落在 `Devices/<UDID>` 的不可用模拟器目录。
///
/// `Devices` 根不存在时不调用 simctl（未装 Xcode / 测试 HOME 下零开销）。
pub fn select_unavailable_simulators(deps: &dyn SimctlDeps) -> Vec<PathBuf> {
    let root = deps.devices_root();
    if !is_real_dir(&root) {
        return Vec::new();
    }
    let Some(output) = deps.list_unavailable() else {
        return Vec::new();
    };
    parse_unavailable_udids(&output)
        .into_iter()
        .map(|udid| root.join(udid))
        .filter(|p| is_real_dir(p))
        .collect()
}

/// apply 复验：路径必须是 `Devices/<UDID>` 直接子目录，返回 UDID。
pub fn simulator_udid_for_path(deps: &dyn SimctlDeps, path: &Path) -> Option<String> {
    if path.parent()? != deps.devices_root() {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    if !is_simulator_udid(name) || !is_real_dir(path) {
        return None;
    }
    Some(name.to_string())
}

/// apply 复验：UDID 此刻仍被 simctl 报告为不可用（列不出来则 fail-closed）。
pub fn udid_still_unavailable(deps: &dyn SimctlDeps, udid: &str) -> bool {
    deps.list_unavailable().is_some_and(|out| {
        parse_unavailable_udids(&out)
            .iter()
            .any(|u| u.eq_ignore_ascii_case(udid))
    })
}

/// 生产路径：`xcrun simctl`，不改动全局 `xcode-select`。
pub struct LiveSimctlDeps;

const SIMCTL_LIST_TIMEOUT: Duration = Duration::from_secs(10);
const SIMCTL_DELETE_TIMEOUT: Duration = Duration::from_secs(60);
const QUICK_TIMEOUT: Duration = Duration::from_secs(3);

/// 测试 HOME 下 live simctl 一律不跑，防止误删开发机上真实的模拟器。
fn live_simctl_blocked() -> bool {
    std::env::var_os("VOLE_TEST_HOME").is_some()
}

fn live_home() -> PathBuf {
    if let Some(home) = std::env::var_os("VOLE_TEST_HOME") {
        return PathBuf::from(home);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn run_ok(argv: &[&str], timeout: Duration) -> Option<String> {
    let output = MacSysCommand.run(argv, timeout).ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn developer_dir_has_simctl(dir: &str) -> bool {
    let env_arg = format!("DEVELOPER_DIR={dir}");
    Path::new(dir).is_dir()
        && run_ok(
            &["env", &env_arg, "xcrun", "--find", "simctl"],
            QUICK_TIMEOUT,
        )
        .is_some()
}

/// 解析 simctl 所用 `DEVELOPER_DIR`（对齐 Mole `_resolve_simctl_developer_dir`）：
/// 显式 `DEVELOPER_DIR` 优先且无效时不换别的 Xcode；`xcode-select` 指向 Xcode 时用它；
/// 指向 CommandLineTools 时仅在 `/Applications`、`~/Applications` 恰有一个可用 Xcode 时用它。
fn resolve_developer_dir() -> Option<String> {
    if let Some(explicit) = std::env::var_os("DEVELOPER_DIR") {
        let explicit = explicit.to_string_lossy().into_owned();
        return developer_dir_has_simctl(&explicit).then_some(explicit);
    }
    let selected = run_ok(&["xcode-select", "-p"], QUICK_TIMEOUT)?;
    let selected = selected.trim().trim_end_matches('/').to_string();
    if selected.is_empty() {
        return None;
    }
    if selected != "/Library/Developer/CommandLineTools" {
        return developer_dir_has_simctl(&selected).then_some(selected);
    }
    let mut candidates = Vec::new();
    for root in [
        PathBuf::from("/Applications"),
        live_home().join("Applications"),
    ] {
        let Ok(rd) = fs::read_dir(&root) else {
            continue;
        };
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("Xcode") && name.ends_with(".app")) {
                continue;
            }
            let dev = ent.path().join("Contents/Developer");
            let dev = dev.to_string_lossy().into_owned();
            if developer_dir_has_simctl(&dev) {
                candidates.push(dev);
            }
        }
    }
    (candidates.len() == 1).then(|| candidates.remove(0))
}

fn run_simctl(args: &[&str], timeout: Duration) -> Result<String, String> {
    let dev = resolve_developer_dir().ok_or_else(|| "simctl unavailable".to_string())?;
    let env_arg = format!("DEVELOPER_DIR={dev}");
    let mut argv = vec!["env", env_arg.as_str(), "xcrun", "simctl"];
    argv.extend_from_slice(args);
    let output = MacSysCommand
        .run(&argv, timeout)
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

impl SimctlDeps for LiveSimctlDeps {
    fn devices_root(&self) -> PathBuf {
        live_home().join("Library/Developer/CoreSimulator/Devices")
    }

    fn list_unavailable(&self) -> Option<String> {
        if live_simctl_blocked() {
            return None;
        }
        run_simctl(&["list", "devices", "unavailable"], SIMCTL_LIST_TIMEOUT).ok()
    }

    fn delete_device(&self, udid: &str) -> Result<(), String> {
        if live_simctl_blocked() {
            return Err("simctl blocked in test mode".into());
        }
        run_simctl(&["delete", udid], SIMCTL_DELETE_TIMEOUT).map(|_| ())
    }
}

/// 测试替身：固定 list 输出，delete 删目录并记录 UDID。
pub struct FakeSimctlDeps {
    pub root: PathBuf,
    pub list_output: Option<String>,
    pub delete_result: Result<(), String>,
    pub deleted: Mutex<Vec<String>>,
}

impl FakeSimctlDeps {
    pub fn new(root: PathBuf, list_output: Option<String>) -> Self {
        Self {
            root,
            list_output,
            delete_result: Ok(()),
            deleted: Mutex::new(Vec::new()),
        }
    }
}

impl SimctlDeps for FakeSimctlDeps {
    fn devices_root(&self) -> PathBuf {
        self.root.clone()
    }

    fn list_unavailable(&self) -> Option<String> {
        self.list_output.clone()
    }

    fn delete_device(&self, udid: &str) -> Result<(), String> {
        self.delete_result.clone()?;
        fs::remove_dir_all(self.root.join(udid)).map_err(|e| e.to_string())?;
        if let Ok(mut d) = self.deleted.lock() {
            d.push(udid.to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UDID_A: &str = "0A1B2C3D-4E5F-6071-8293-A4B5C6D7E8F9";
    const UDID_B: &str = "11111111-2222-3333-4444-555555555555";

    fn sample_output() -> String {
        format!(
            "== Devices ==\n\
             -- iOS 15.0 --\n\
             \x20   iPhone 8 ({UDID_A}) (Shutdown) (unavailable, runtime profile not found)\n\
             -- Unavailable: com.apple.CoreSimulator.SimRuntime.iOS-14-5 --\n\
             \x20   iPad (9th generation) ({UDID_B}) (Shutdown) (unavailable, runtime profile not found)\n\
             \x20   iPhone 15 (99999999-AAAA-BBBB-CCCC-DDDDDDDDDDDD) (Booted)\n"
        )
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vole-simctl-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn udid_shape() {
        assert!(is_simulator_udid(UDID_A));
        assert!(!is_simulator_udid("Shutdown"));
        assert!(!is_simulator_udid("9th generation"));
        assert!(!is_simulator_udid("0A1B2C3D-4E5F-6071-8293-A4B5C6D7E8F"));
        assert!(!is_simulator_udid("../../../../etc/passwd-0000-0000-0000"));
    }

    #[test]
    fn parses_only_unavailable_lines() {
        let udids = parse_unavailable_udids(&sample_output());
        assert_eq!(udids, vec![UDID_A.to_string(), UDID_B.to_string()]);
    }

    #[test]
    fn select_requires_devices_root_and_existing_dir() {
        let root = scratch("select");
        let devices = root.join("Devices");
        let deps = FakeSimctlDeps::new(devices.clone(), Some(sample_output()));
        assert!(select_unavailable_simulators(&deps).is_empty());

        fs::create_dir_all(devices.join(UDID_A)).unwrap();
        let got = select_unavailable_simulators(&deps);
        assert_eq!(got, vec![devices.join(UDID_A)]);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn select_empty_when_simctl_unavailable() {
        let root = scratch("nosimctl");
        let devices = root.join("Devices");
        fs::create_dir_all(devices.join(UDID_A)).unwrap();
        let deps = FakeSimctlDeps::new(devices, None);
        assert!(select_unavailable_simulators(&deps).is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn udid_for_path_rejects_off_shape() {
        let root = scratch("shape");
        let devices = root.join("Devices");
        fs::create_dir_all(devices.join(UDID_A)).unwrap();
        fs::create_dir_all(devices.join(UDID_A).join("data")).unwrap();
        fs::create_dir_all(root.join(UDID_B)).unwrap();
        let deps = FakeSimctlDeps::new(devices.clone(), Some(sample_output()));
        assert_eq!(
            simulator_udid_for_path(&deps, &devices.join(UDID_A)).as_deref(),
            Some(UDID_A)
        );
        assert!(simulator_udid_for_path(&deps, &devices.join(UDID_A).join("data")).is_none());
        assert!(simulator_udid_for_path(&deps, &root.join(UDID_B)).is_none());
        assert!(simulator_udid_for_path(&deps, &devices.join(UDID_B)).is_none());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn still_unavailable_fails_closed() {
        let deps = FakeSimctlDeps::new(PathBuf::from("/nonexistent"), Some(sample_output()));
        assert!(udid_still_unavailable(&deps, UDID_A));
        assert!(udid_still_unavailable(&deps, &UDID_A.to_ascii_lowercase()));
        assert!(!udid_still_unavailable(
            &deps,
            "99999999-AAAA-BBBB-CCCC-DDDDDDDDDDDD"
        ));
        let none = FakeSimctlDeps::new(PathBuf::from("/nonexistent"), None);
        assert!(!udid_still_unavailable(&none, UDID_A));
    }
}
