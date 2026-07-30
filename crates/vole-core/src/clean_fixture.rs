//! Clean-rule JSON fixtures (`tests/fixtures/clean/`) for plan verification.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum FixtureStep {
    Mkdir {
        mkdir: String,
        #[serde(default)]
        mtime: Option<String>,
    },
    Write {
        write: String,
        content: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct CleanFixture {
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub source_bats: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub source_test: Option<String>,
    pub fixture: Vec<FixtureStep>,
    #[serde(default)]
    pub expect_selected: Vec<String>,
    #[serde(default)]
    pub expect_not_selected: Vec<String>,
}

impl CleanFixture {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn materialize(&self, home: &Path) -> io::Result<()> {
        for step in &self.fixture {
            match step {
                FixtureStep::Mkdir { mkdir, mtime } => {
                    let path = expand_tilde(mkdir, home);
                    fs::create_dir_all(&path)?;
                    if let Some(ts) = mtime {
                        set_mtime(&path, ts)?;
                    }
                }
                FixtureStep::Write { write, content } => {
                    let path = expand_tilde(write, home);
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&path, content.as_bytes())?;
                }
            }
        }
        Ok(())
    }
}

pub fn expand_tilde(path: &str, home: &Path) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else if path == "~" {
        home.to_path_buf()
    } else {
        PathBuf::from(path)
    }
}

pub fn normalize_fixture_path(path: &str, home: &Path) -> String {
    let expanded = expand_tilde(path, home);
    let home_str = home.to_string_lossy();
    expanded.to_string_lossy().replace(home_str.as_ref(), "~")
}

fn set_mtime(path: &Path, mtime: &str) -> io::Result<()> {
    let parsed = parse_fixture_mtime(mtime).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid fixture mtime: {mtime}"),
        )
    })?;
    filetime::set_file_mtime(path, filetime::FileTime::from_system_time(parsed))
}

fn parse_fixture_mtime(value: &str) -> Option<SystemTime> {
    let (date, time) = value.split_once('T')?;
    let (year, month, day) = parse_ymd(date)?;
    let (hour, minute) = parse_hm(time)?;
    datetime_to_system_time(year, month, day, hour, minute)
}

fn parse_ymd(date: &str) -> Option<(i32, u32, u32)> {
    let mut parts = date.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    Some((year, month, day))
}

fn parse_hm(time: &str) -> Option<(u32, u32)> {
    let mut parts = time.split(':');
    let hour: u32 = parts.next()?.parse().ok()?;
    let minute: u32 = parts.next()?.parse().ok()?;
    Some((hour, minute))
}

fn datetime_to_system_time(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
) -> Option<SystemTime> {
    use std::time::Duration;
    const UNIX_EPOCH_YEAR: i32 = 1970;
    if year < UNIX_EPOCH_YEAR {
        return None;
    }
    let mut days = 0i64;
    for y in UNIX_EPOCH_YEAR..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for (idx, md) in month_days.iter().enumerate() {
        let m = (idx + 1) as u32;
        if m >= month {
            break;
        }
        days += i64::from(*md);
        if m == 2 && is_leap_year(year) {
            days += 1;
        }
    }
    days += i64::from(day.saturating_sub(1));
    let secs = days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60;
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs.max(0) as u64))
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod verify_clean_fixtures {
    use super::*;
    use crate::ops::Orchestrator;
    use crate::protection::AppProtection;
    use crate::rules::{load_rules_from_dir, FakeProcessProbe};
    use crate::test_env;
    use std::sync::Arc;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/clean")
    }

    fn rules_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/rules")
    }

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("vole-clean-fx-{tag}-{}", std::process::id()))
    }

    #[test]
    fn all_extracted_fixtures_satisfy_plan_expectations() {
        let _guard = test_env::lock();
        let rules = load_rules_from_dir(rules_dir()).expect("load data/rules");
        assert!(
            rules.iter().any(|r| r.id == "claude-desktop-bundled-code"),
            "expected claude desktop rules"
        );
        assert!(
            rules.iter().any(|r| r.id == "codex-stale-runtimes"),
            "expected codex runtime rule"
        );
        let staging = rules
            .iter()
            .find(|r| r.id == "codex-desktop-stale-update-staging")
            .expect("expected codex desktop staging rule");
        assert_eq!(staging.strategy.days, Some(30));
        assert!(!staging.guards.not_running_cmdline.is_empty());

        let dir = fixtures_dir();
        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("missing {}: {e}", dir.display()))
            .map(|e| e.expect("read_dir").path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        assert!(
            !entries.is_empty(),
            "expected fixtures under {}",
            dir.display()
        );

        for path in entries {
            let fx =
                CleanFixture::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            verify_fixture(&fx, &rules);
        }
    }

    fn verify_fixture(fx: &CleanFixture, rules: &[crate::rules::Rule]) {
        let home = scratch(&fx.id);
        fs::remove_dir_all(&home).ok();
        fs::create_dir_all(&home).expect("create scratch home");
        fx.materialize(&home)
            .unwrap_or_else(|e| panic!("{} materialize: {e}", fx.id));

        let _home_guard = TestHomeGuard::set(&home);

        // Hermetic: fixtures assert path selection with idle process probe so
        // developer machines with Chrome/etc. running do not skip guarded rules.
        let orch = Orchestrator::with_process_probe(
            crate::cancel::CancelToken::new(),
            None,
            Arc::new(FakeProcessProbe::default()),
        );
        let plan = orch
            .build_plan(rules, &AppProtection::new(), &[])
            .unwrap_or_else(|e| panic!("{} build_plan: {e}", fx.id));

        let selected: Vec<(String, String)> = plan
            .entries
            .iter()
            .map(|entry| {
                (
                    normalize_fixture_path(&entry.path.display().to_string(), &home),
                    entry.label.clone(),
                )
            })
            .collect();

        for path in &fx.expect_not_selected {
            let normalized = normalize_fixture_path(path, &home);
            assert!(
                !selected.iter().any(|(p, _)| p == &normalized),
                "{}: path must not be selected: {normalized}",
                fx.id
            );
        }

        for expected in &fx.expect_selected {
            let (path, label) = expected
                .split_once('|')
                .unwrap_or_else(|| panic!("{}: invalid expect_selected: {expected}", fx.id));
            let normalized = normalize_fixture_path(path, &home);
            assert!(
                selected.iter().any(|(p, l)| p == &normalized && l == label),
                "{}: expected selected {normalized}|{label}, got {:?}",
                fx.id,
                selected
            );
        }

        fs::remove_dir_all(&home).ok();
    }

    struct TestHomeGuard {
        prev: Option<std::ffi::OsString>,
    }

    impl TestHomeGuard {
        fn set(home: &Path) -> Self {
            let prev = std::env::var_os("VOLE_TEST_HOME");
            std::env::set_var("VOLE_TEST_HOME", home);
            Self { prev }
        }
    }

    impl Drop for TestHomeGuard {
        fn drop(&mut self) {
            if let Some(prev) = &self.prev {
                std::env::set_var("VOLE_TEST_HOME", prev);
            } else {
                std::env::remove_var("VOLE_TEST_HOME");
            }
        }
    }
}
