use std::collections::HashSet;
use std::time::Duration;

use vole_sys::macos::MacSysCommand;
use vole_sys::SysCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Idle,
    Unknown,
}

pub trait ProcessProbe: Send + Sync {
    fn exact_name_running(&self, name: &str) -> ProcessState;
}

#[derive(Debug, Default, Clone)]
pub struct FakeProcessProbe {
    pub running: HashSet<String>,
    pub unknown: HashSet<String>,
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
}

pub fn should_skip_for_not_running(probe: &dyn ProcessProbe, names: &[String]) -> bool {
    names
        .iter()
        .filter(|n| !n.is_empty())
        .any(|n| !matches!(probe.exact_name_running(n), ProcessState::Idle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn empty_names_never_skips() {
        let probe = FakeProcessProbe::default();
        assert!(!should_skip_for_not_running(&probe, &[]));
    }

    #[test]
    fn skips_when_any_exact_name_running() {
        let probe = FakeProcessProbe {
            running: HashSet::from(["Firefox".into()]),
            unknown: HashSet::new(),
        };
        assert!(should_skip_for_not_running(
            &probe,
            &["Chrome".into(), "Firefox".into()]
        ));
    }

    #[test]
    fn idle_when_none_running() {
        let probe = FakeProcessProbe::default();
        assert!(!should_skip_for_not_running(
            &probe,
            &["Firefox".into()]
        ));
    }

    #[test]
    fn unknown_fail_closed_skips() {
        let probe = FakeProcessProbe {
            running: HashSet::new(),
            unknown: HashSet::from(["Mail".into()]),
        };
        assert!(should_skip_for_not_running(&probe, &["Mail".into()]));
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
        assert_eq!(state_from_pgrep_status(Some(2), false), ProcessState::Unknown);
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
