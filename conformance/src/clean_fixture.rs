//! Clean-rule fixture schema (design doc §7B).
//!
//! JSON files under `tests/fixtures/clean/` are produced by
//! `scripts/extract-clean-fixtures.py` from Mole bats tests.

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
    #[allow(dead_code)]
    pub expect_selected: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
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
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("clean")
    }

    #[test]
    fn deserializes_extracted_clean_fixtures() {
        let dir = fixtures_dir();
        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("missing {}: {e}", dir.display()))
            .map(|e| e.expect("read_dir").path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        assert!(
            !entries.is_empty(),
            "expected at least one fixture under {}",
            dir.display()
        );

        for path in entries {
            let fx = CleanFixture::load(&path)
                .unwrap_or_else(|e| panic!("{} is not a valid CleanFixture: {e}", path.display()));
            assert!(!fx.id.is_empty(), "{}", path.display());
            assert!(
                !fx.fixture.is_empty(),
                "{} must declare at least one fixture step",
                path.display()
            );
            for sel in &fx.expect_selected {
                if !sel.is_empty() {
                    assert!(
                        sel.contains('|'),
                        "{} expect_selected entry must be path|label: {sel:?}",
                        path.display()
                    );
                }
            }
        }
    }
}
