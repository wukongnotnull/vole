//! Optimize `shared_file_list_repair`（对齐 Mole `opt_shared_file_list_repair`）。
//!
//! **禁止** `sfltool`（含 dumpbtm）。与 clean `recent-items-list` 边界：本任务仅删
//! `plutil -lint` 失败的 `.sfl2`/`.sfl3`，并永久跳过 `ApplicationRecentDocuments`。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use super::delete_paths::OptimizeCandidate;
use crate::delete::test_no_auth;
use crate::optimize::OptimizeTaskKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedFileListError {
    TestMode,
    Unavailable,
}

pub trait SharedFileListDeps: Send + Sync {
    fn list_candidates(&self, home: &Path) -> Result<Vec<PathBuf>, SharedFileListError>;
    fn is_corrupt(&self, path: &Path) -> bool;
    fn remove(&self, path: &Path) -> Result<(), SharedFileListError>;
}

pub struct LiveSharedFileListDeps;

impl SharedFileListDeps for LiveSharedFileListDeps {
    fn list_candidates(&self, home: &Path) -> Result<Vec<PathBuf>, SharedFileListError> {
        if test_no_auth() {
            return Err(SharedFileListError::TestMode);
        }
        Ok(scan_shared_file_lists(home))
    }

    fn is_corrupt(&self, path: &Path) -> bool {
        if test_no_auth() {
            return false;
        }
        live_plutil_corrupt(path)
    }

    fn remove(&self, path: &Path) -> Result<(), SharedFileListError> {
        if test_no_auth() {
            return Err(SharedFileListError::TestMode);
        }
        fs::remove_file(path).map_err(|_| SharedFileListError::Unavailable)
    }
}

pub fn scan_shared_file_lists(home: &Path) -> Vec<PathBuf> {
    let root = home.join("Library/Application Support/com.apple.sharedfilelist");
    if !root.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    walk_sfl(&root, &mut out);
    out.sort();
    out
}

fn walk_sfl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let path_str = path.to_string_lossy();
        if path_str.contains("ApplicationRecentDocuments") {
            continue;
        }
        if path.is_dir() {
            walk_sfl(&path, out);
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext == "sfl2" || ext == "sfl3" {
            out.push(path);
        }
    }
}

fn live_plutil_corrupt(path: &Path) -> bool {
    let status = Command::new("plutil")
        .arg("-lint")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) => !s.success(),
        Err(_) => false, // fail-closed: cannot lint → do not delete
    }
}

pub fn plan_shared_file_list_repair(
    home: &Path,
    deps: &dyn SharedFileListDeps,
) -> Vec<OptimizeCandidate> {
    let paths = match deps.list_candidates(home) {
        Ok(p) => p,
        Err(SharedFileListError::TestMode) | Err(SharedFileListError::Unavailable) => {
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for path in paths {
        if deps.is_corrupt(&path) {
            out.push(OptimizeCandidate {
                path,
                label: "Corrupted shared file list".into(),
                size: 0,
                task_id: "shared_file_list_repair",
                kind: OptimizeTaskKind::Action,
            });
        }
    }
    out
}

pub fn run_shared_file_list_repair(
    path: &Path,
    deps: &dyn SharedFileListDeps,
) -> Result<(), SharedFileListError> {
    if path
        .to_string_lossy()
        .contains("ApplicationRecentDocuments")
    {
        return Ok(());
    }
    if !deps.is_corrupt(path) {
        return Ok(());
    }
    deps.remove(path)
}

#[derive(Default)]
pub struct FakeSharedFileListDeps {
    pub files: Mutex<Vec<PathBuf>>,
    pub corrupt: Mutex<Vec<PathBuf>>,
    pub removed: Mutex<Vec<PathBuf>>,
    pub list_error: Mutex<Option<SharedFileListError>>,
}

impl SharedFileListDeps for FakeSharedFileListDeps {
    fn list_candidates(&self, _home: &Path) -> Result<Vec<PathBuf>, SharedFileListError> {
        if let Some(err) = self.list_error.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(self.files.lock().unwrap().clone())
    }

    fn is_corrupt(&self, path: &Path) -> bool {
        self.corrupt.lock().unwrap().iter().any(|p| p == path)
    }

    fn remove(&self, path: &Path) -> Result<(), SharedFileListError> {
        self.removed.lock().unwrap().push(path.to_path_buf());
        self.files.lock().unwrap().retain(|p| p != path);
        self.corrupt.lock().unwrap().retain(|p| p != path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn scan_skips_application_recent_documents() {
        let home = tempdir().unwrap();
        let root = home
            .path()
            .join("Library/Application Support/com.apple.sharedfilelist");
        fs::create_dir_all(root.join("ApplicationRecentDocuments")).unwrap();
        fs::write(root.join("favorites.sfl2"), b"x").unwrap();
        fs::write(root.join("ApplicationRecentDocuments/foo.sfl2"), b"y").unwrap();
        let scanned = scan_shared_file_lists(home.path());
        assert_eq!(scanned.len(), 1);
        assert!(scanned[0].ends_with("favorites.sfl2"));
    }

    #[test]
    fn plan_emits_only_corrupt() {
        let home = tempdir().unwrap();
        let good = home.path().join("good.sfl2");
        let bad = home.path().join("bad.sfl2");
        let fake = FakeSharedFileListDeps {
            files: Mutex::new(vec![good.clone(), bad.clone()]),
            corrupt: Mutex::new(vec![bad.clone()]),
            ..Default::default()
        };
        let plan = plan_shared_file_list_repair(home.path(), &fake);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].path, bad);
        assert_eq!(plan[0].task_id, "shared_file_list_repair");
    }

    #[test]
    fn apply_removes_corrupt_only() {
        let home = tempdir().unwrap();
        let bad = home.path().join("bad.sfl2");
        let fake = FakeSharedFileListDeps {
            files: Mutex::new(vec![bad.clone()]),
            corrupt: Mutex::new(vec![bad.clone()]),
            ..Default::default()
        };
        run_shared_file_list_repair(&bad, &fake).unwrap();
        assert_eq!(fake.removed.lock().unwrap().clone(), vec![bad]);
    }

    #[test]
    fn apply_noop_when_healthy() {
        let home = tempdir().unwrap();
        let good = home.path().join("good.sfl2");
        let fake = FakeSharedFileListDeps {
            files: Mutex::new(vec![good.clone()]),
            corrupt: Mutex::new(vec![]),
            ..Default::default()
        };
        run_shared_file_list_repair(&good, &fake).unwrap();
        assert!(fake.removed.lock().unwrap().is_empty());
    }

    #[test]
    fn apply_skips_application_recent_documents_path() {
        let path = PathBuf::from("/tmp/com.apple.sharedfilelist/ApplicationRecentDocuments/x.sfl2");
        let fake = FakeSharedFileListDeps {
            corrupt: Mutex::new(vec![path.clone()]),
            ..Default::default()
        };
        run_shared_file_list_repair(&path, &fake).unwrap();
        assert!(fake.removed.lock().unwrap().is_empty());
    }
}
