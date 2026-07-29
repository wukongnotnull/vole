//! Clean-rule fixture schema (design doc §7B).
//!
//! JSON files under `tests/fixtures/clean/` are produced by
//! `scripts/extract-clean-fixtures.py` from Mole bats tests.

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
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
#[allow(dead_code)]
pub struct CleanFixture {
    pub id: String,
    #[serde(default)]
    pub source_bats: Option<String>,
    #[serde(default)]
    pub source_test: Option<String>,
    pub fixture: Vec<FixtureStep>,
    #[serde(default)]
    pub expect_selected: Vec<String>,
    #[serde(default)]
    pub expect_not_selected: Vec<String>,
}

impl CleanFixture {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = std::fs::read_to_string(path)?;
        let fx: Self = serde_json::from_str(&text)?;
        Ok(fx)
    }
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
        let entries: Vec<_> = std::fs::read_dir(&dir)
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
            let fx = CleanFixture::load(&path).unwrap_or_else(|e| {
                panic!("{} is not a valid CleanFixture: {e}", path.display())
            });
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
