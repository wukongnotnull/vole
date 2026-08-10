//! analyze TUI 删除：固定 Trash 模式，经 `mole_delete`（无平行 `rm`）。

use std::path::Path;

use vole_sys::macos::MacTrash;

use crate::oplog::OperationLogger;
use crate::protection::AppProtection;

use super::{mole_delete, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TrashAnalyzeReport {
    pub removed: Vec<String>,
    pub errors: Vec<String>,
}

/// Move analyze-selected paths to Trash via `mole_delete` only.
pub fn trash_analyze_paths(paths: &[String]) -> TrashAnalyzeReport {
    let mut ordered = paths.to_vec();
    ordered.sort_by(|a, b| {
        let da = Path::new(a).components().count();
        let db = Path::new(b).components().count();
        db.cmp(&da).then_with(|| a.cmp(b))
    });

    let protection = AppProtection::new();
    let deletion_log = DeletionLogger::from_env();
    let mut oplog = OperationLogger::new("analyze");
    let trash = MacTrash;

    let mut report = TrashAnalyzeReport::default();
    for path in ordered {
        match mole_delete(
            &path,
            &protection,
            &[],
            MoleDeleteOptions {
                mode: DeleteMode::Trash,
                dry_run: false,
                needs_sudo: false,
                privilege: None,
            },
            &trash,
            &deletion_log,
            &mut oplog,
        ) {
            Ok(()) => report.removed.push(path),
            Err(MoleDeleteError::Vanished) => report.removed.push(path),
            Err(err) => report.errors.push(format!("{path}: {err}")),
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;
    use std::fs;

    #[test]
    fn trash_analyze_paths_uses_test_trash_dir() {
        let _guard = test_env::lock();
        let root = std::env::temp_dir().join(format!(
            "vole-core-analyze-del-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let victim = root.join("victim.txt");
        let trash = root.join("Trash");
        let log_path = root.join("deletions.log");
        fs::create_dir_all(&root).unwrap();
        fs::write(&victim, b"x").unwrap();
        fs::create_dir_all(&trash).unwrap();
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash);
        std::env::set_var("MOLE_DELETE_LOG", &log_path);

        let report = trash_analyze_paths(&[victim.to_string_lossy().into_owned()]);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert!(!victim.exists());
        assert_eq!(report.removed.len(), 1);

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn trash_analyze_paths_rejects_protected() {
        let _guard = test_env::lock();
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        let report = trash_analyze_paths(&["/System/Library".into()]);
        assert!(report.removed.is_empty());
        assert!(!report.errors.is_empty());
    }
}
