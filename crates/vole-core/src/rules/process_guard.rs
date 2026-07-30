use std::collections::HashSet;
use std::time::Duration;

use vole_sys::macos::MacSysCommand;
use vole_sys::SysCommand;

use crate::rules::schema::GuardsConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Idle,
    Unknown,
}

pub trait ProcessProbe: Send + Sync {
    fn exact_name_running(&self, name: &str) -> ProcessState;
    fn cmdline_substring_running(&self, needle: &str) -> ProcessState;
}

#[derive(Debug, Default, Clone)]
pub struct FakeProcessProbe {
    pub running: HashSet<String>,
    pub unknown: HashSet<String>,
    pub cmdline_running: HashSet<String>,
    pub cmdline_unknown: HashSet<String>,
}

impl ProcessProbe for FakeProcessProbe {
    fn exact_name_running(&self, name: &str) -> ProcessState {
        if self.running.contains(name) {
            ProcessState::Running
        } else if self.unknown.contains(name) {
            ProcessState::Unknown
        } else {
            ProcessState::Idle
        }
    }

    fn cmdline_substring_running(&self, needle: &str) -> ProcessState {
        if self.cmdline_running.contains(needle) {
            ProcessState::Running
        } else if self.cmdline_unknown.contains(needle) {
            ProcessState::Unknown
        } else {
            ProcessState::Idle
        }
    }
}

pub(crate) fn state_from_pgrep_status(code: Option<i32>, timed_out: bool) -> ProcessState {
    if timed_out {
        return ProcessState::Unknown;
    }
    match code {
        Some(0) => ProcessState::Running,
        Some(1) => ProcessState::Idle,
        _ => ProcessState::Unknown,
    }
}

pub struct PgrepProcessProbe;

impl ProcessProbe for PgrepProcessProbe {
    fn exact_name_running(&self, name: &str) -> ProcessState {
        let cmd = MacSysCommand;
        match cmd.run(&["pgrep", "-x", name], Duration::from_secs(2)) {
            Ok(output) => state_from_pgrep_status(output.status.code(), false),
            Err(vole_sys::traits::SysCommandError::Timeout) => ProcessState::Unknown,
            Err(_) => ProcessState::Unknown,
        }
    }

    fn cmdline_substring_running(&self, needle: &str) -> ProcessState {
        let cmd = MacSysCommand;
        match cmd.run(&["pgrep", "-f", needle], Duration::from_secs(2)) {
            Ok(output) => state_from_pgrep_status(output.status.code(), false),
            Err(vole_sys::traits::SysCommandError::Timeout) => ProcessState::Unknown,
            Err(_) => ProcessState::Unknown,
        }
    }
}

pub fn should_skip_for_not_running(probe: &dyn ProcessProbe, names: &[String]) -> bool {
    names
        .iter()
        .filter(|n| !n.is_empty())
        .any(|n| !matches!(probe.exact_name_running(n), ProcessState::Idle))
}

pub fn should_skip_for_cmdline(probe: &dyn ProcessProbe, needles: &[String]) -> bool {
    needles
        .iter()
        .filter(|n| !n.is_empty())
        .any(|n| !matches!(probe.cmdline_substring_running(n), ProcessState::Idle))
}

pub fn should_skip_for_guards(probe: &dyn ProcessProbe, guards: &GuardsConfig) -> bool {
    should_skip_for_not_running(probe, &guards.not_running)
        || should_skip_for_cmdline(probe, &guards.not_running_cmdline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_names_never_skips() {
        let probe = FakeProcessProbe::default();
        assert!(!should_skip_for_not_running(&probe, &[]));
        assert!(!should_skip_for_guards(&probe, &GuardsConfig::default()));
    }

    #[test]
    fn skips_when_any_exact_name_running() {
        let probe = FakeProcessProbe {
            running: HashSet::from(["Firefox".into()]),
            ..Default::default()
        };
        assert!(should_skip_for_not_running(
            &probe,
            &["Chrome".into(), "Firefox".into()]
        ));
    }

    #[test]
    fn idle_when_none_running() {
        let probe = FakeProcessProbe::default();
        assert!(!should_skip_for_not_running(&probe, &["Firefox".into()]));
    }

    #[test]
    fn unknown_fail_closed_skips() {
        let probe = FakeProcessProbe {
            unknown: HashSet::from(["Mail".into()]),
            ..Default::default()
        };
        assert!(should_skip_for_not_running(&probe, &["Mail".into()]));
    }

    #[test]
    fn skips_when_cmdline_substring_running() {
        let probe = FakeProcessProbe {
            cmdline_running: HashSet::from(["/Final Cut Pro.app/".into()]),
            ..Default::default()
        };
        let guards = GuardsConfig {
            not_running_cmdline: vec!["/Final Cut Pro.app/".into()],
            ..Default::default()
        };
        assert!(should_skip_for_guards(&probe, &guards));
    }

    #[test]
    fn cmdline_unknown_fail_closed_skips() {
        let probe = FakeProcessProbe {
            cmdline_unknown: HashSet::from(["/Final Cut Pro.app/".into()]),
            ..Default::default()
        };
        assert!(should_skip_for_cmdline(
            &probe,
            &["/Final Cut Pro.app/".into()]
        ));
    }

    #[test]
    fn state_from_pgrep_status_exit_zero_is_running() {
        assert_eq!(
            state_from_pgrep_status(Some(0), false),
            ProcessState::Running
        );
    }

    #[test]
    fn state_from_pgrep_status_exit_one_is_idle() {
        assert_eq!(state_from_pgrep_status(Some(1), false), ProcessState::Idle);
    }

    #[test]
    fn state_from_pgrep_status_other_exit_is_unknown() {
        assert_eq!(
            state_from_pgrep_status(Some(2), false),
            ProcessState::Unknown
        );
        assert_eq!(state_from_pgrep_status(None, false), ProcessState::Unknown);
    }

    #[test]
    fn state_from_pgrep_status_timeout_is_unknown() {
        assert_eq!(
            state_from_pgrep_status(Some(0), true),
            ProcessState::Unknown
        );
        assert_eq!(state_from_pgrep_status(None, true), ProcessState::Unknown);
    }
}
