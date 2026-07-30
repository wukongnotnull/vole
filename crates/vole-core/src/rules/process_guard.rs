use std::collections::HashSet;

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
}
