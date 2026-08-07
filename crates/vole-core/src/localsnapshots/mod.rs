//! Time Machine 本地快照报告（仅 list，禁止删除）。
//!
//! 对齐 Mole `clean_local_snapshots`：`tmutil listlocalsnapshots /` → 数量 + review 提示。

use std::process::Command;

use regex::Regex;
use vole_sys::macos::MacSysCommand;
use vole_sys::timeouts::SHORT_QUERY;
use vole_sys::vole_proto::status::LocalSnapshotsInfo;
use vole_sys::SysCommand;

/// 本模块自有的 TM 运行态（不耦合 `tmbackup`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalTmRunningState {
    Running,
    Idle,
    Unknown,
}

/// 探测结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalSnapshotReport {
    Quiet,
    Present { count: u64 },
    SkippedBusy,
    SkippedUnknown,
}

/// 可注入依赖。
pub trait LocalSnapshotDeps: Send + Sync {
    fn tmutil_exists(&self) -> bool;
    fn auto_backup_configured(&self) -> bool;
    fn running_state(&self) -> LocalTmRunningState;
    /// 成功返回 stdout；失败/超时 → `None`（fail-closed → Quiet）。
    fn list_localsnapshots(&self) -> Option<String>;
}

/// 生产路径。
pub struct LiveLocalSnapshotDeps;

impl LocalSnapshotDeps for LiveLocalSnapshotDeps {
    fn tmutil_exists(&self) -> bool {
        Command::new("tmutil")
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn auto_backup_configured(&self) -> bool {
        let output = Command::new("defaults")
            .args([
                "read",
                "/Library/Preferences/com.apple.TimeMachine",
                "AutoBackup",
            ])
            .output();
        let Ok(output) = output else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        let s = String::from_utf8_lossy(&output.stdout);
        let t = s.trim();
        t == "0" || t == "1"
    }

    fn running_state(&self) -> LocalTmRunningState {
        let output = Command::new("tmutil").arg("status").output();
        let Ok(output) = output else {
            return LocalTmRunningState::Unknown;
        };
        if !output.status.success() {
            return LocalTmRunningState::Unknown;
        }
        let st = String::from_utf8_lossy(&output.stdout);
        if !st.contains("Running") {
            return LocalTmRunningState::Unknown;
        }
        for line in st.lines() {
            let t = line.trim();
            if t.contains("Running") && t.contains('=') {
                if t.contains("= 1") || t.contains("=1") {
                    return LocalTmRunningState::Running;
                }
                if t.contains("= 0") || t.contains("=0") {
                    return LocalTmRunningState::Idle;
                }
            }
        }
        LocalTmRunningState::Unknown
    }

    fn list_localsnapshots(&self) -> Option<String> {
        let cmd = MacSysCommand;
        let out = cmd
            .run(&["tmutil", "listlocalsnapshots", "/"], SHORT_QUERY)
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// 统计 Mole 形本地快照行。
pub fn count_tm_snapshot_lines(stdout: &str) -> u64 {
    let re = Regex::new(r"com\.apple\.TimeMachine\.[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}")
        .expect("static regex");
    re.find_iter(stdout).count() as u64
}

/// 按 design §5 门控探测。
pub fn probe_local_snapshots(deps: &dyn LocalSnapshotDeps) -> LocalSnapshotReport {
    if !deps.tmutil_exists() {
        return LocalSnapshotReport::Quiet;
    }
    if !deps.auto_backup_configured() {
        return LocalSnapshotReport::Quiet;
    }
    match deps.running_state() {
        LocalTmRunningState::Unknown => return LocalSnapshotReport::SkippedUnknown,
        LocalTmRunningState::Running => return LocalSnapshotReport::SkippedBusy,
        LocalTmRunningState::Idle => {}
    }
    let Some(stdout) = deps.list_localsnapshots() else {
        return LocalSnapshotReport::Quiet;
    };
    let count = count_tm_snapshot_lines(&stdout);
    if count == 0 {
        LocalSnapshotReport::Quiet
    } else {
        LocalSnapshotReport::Present { count }
    }
}

/// Present / Skip → 文案；Quiet → None。
pub fn format_message(report: &LocalSnapshotReport) -> Option<String> {
    match report {
        LocalSnapshotReport::Quiet => None,
        LocalSnapshotReport::Present { count } => Some(format!(
            "Time Machine local snapshots · {count} (review: tmutil listlocalsnapshots /)"
        )),
        LocalSnapshotReport::SkippedUnknown => {
            Some("Snapshot check · skipped (Time Machine status unknown)".into())
        }
        LocalSnapshotReport::SkippedBusy => {
            Some("Snapshot check · skipped (backup in progress)".into())
        }
    }
}

/// 协议字段；Quiet → None。
pub fn to_info(report: LocalSnapshotReport) -> Option<LocalSnapshotsInfo> {
    let message = format_message(&report)?;
    let count = match report {
        LocalSnapshotReport::Present { count } => Some(count),
        _ => None,
    };
    Some(LocalSnapshotsInfo { count, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        tmutil: bool,
        auto: bool,
        running: LocalTmRunningState,
        list: Option<String>,
    }

    impl LocalSnapshotDeps for Fake {
        fn tmutil_exists(&self) -> bool {
            self.tmutil
        }
        fn auto_backup_configured(&self) -> bool {
            self.auto
        }
        fn running_state(&self) -> LocalTmRunningState {
            self.running
        }
        fn list_localsnapshots(&self) -> Option<String> {
            self.list.clone()
        }
    }

    fn idle_ok() -> Fake {
        Fake {
            tmutil: true,
            auto: true,
            running: LocalTmRunningState::Idle,
            list: Some(String::new()),
        }
    }

    #[test]
    fn no_tmutil_is_quiet() {
        let mut f = idle_ok();
        f.tmutil = false;
        assert_eq!(probe_local_snapshots(&f), LocalSnapshotReport::Quiet);
    }

    #[test]
    fn auto_backup_bad_is_quiet() {
        let mut f = idle_ok();
        f.auto = false;
        assert_eq!(probe_local_snapshots(&f), LocalSnapshotReport::Quiet);
    }

    #[test]
    fn running_is_skipped_busy() {
        let mut f = idle_ok();
        f.running = LocalTmRunningState::Running;
        assert_eq!(probe_local_snapshots(&f), LocalSnapshotReport::SkippedBusy);
    }

    #[test]
    fn unknown_is_skipped_unknown() {
        let mut f = idle_ok();
        f.running = LocalTmRunningState::Unknown;
        assert_eq!(
            probe_local_snapshots(&f),
            LocalSnapshotReport::SkippedUnknown
        );
    }

    #[test]
    fn list_err_is_quiet_fail_closed() {
        let mut f = idle_ok();
        f.list = None;
        assert_eq!(probe_local_snapshots(&f), LocalSnapshotReport::Quiet);
    }

    #[test]
    fn parses_mole_shaped_lines() {
        let out = "Snapshots for volume group containing disk /:\n\
                   com.apple.TimeMachine.2026-08-01-120000.local\n\
                   com.apple.TimeMachine.2026-08-02-130000.local\n";
        assert_eq!(count_tm_snapshot_lines(out), 2);
        let mut f = idle_ok();
        f.list = Some(out.into());
        assert_eq!(
            probe_local_snapshots(&f),
            LocalSnapshotReport::Present { count: 2 }
        );
        let info = to_info(LocalSnapshotReport::Present { count: 2 }).unwrap();
        assert_eq!(info.count, Some(2));
        assert!(info.message.contains("review: tmutil listlocalsnapshots /"));
    }

    #[test]
    fn zero_matches_quiet() {
        let mut f = idle_ok();
        f.list = Some("Snapshots for volume group...\n".into());
        assert_eq!(probe_local_snapshots(&f), LocalSnapshotReport::Quiet);
        assert!(to_info(LocalSnapshotReport::Quiet).is_none());
    }

    #[test]
    fn module_source_forbids_delete_subcommand() {
        let src = include_str!("mod.rs");
        assert!(
            !src.contains("\"deletelocalsnapshots\"") && !src.contains(".arg(\"delete\")"),
            "must not invoke tmutil delete / deletelocalsnapshots"
        );
    }
}
