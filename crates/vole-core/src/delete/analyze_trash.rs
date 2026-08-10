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
    use std::sync::Mutex;

    // Serialize env mutations within this module even if callers forget test_env.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let prev = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn trash_analyze_paths_uses_test_trash_dir() {
        let _suite = test_env::lock();
        let _local = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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

        let _trash_env = EnvGuard::set("MOLE_TEST_TRASH_DIR", &trash);
        let _log_env = EnvGuard::set("MOLE_DELETE_LOG", &log_path);

        let report = trash_analyze_paths(&[victim.to_string_lossy().into_owned()]);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert!(!victim.exists());
        assert_eq!(report.removed.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn trash_analyze_paths_rejects_protected() {
        let _suite = test_env::lock();
        let _local = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Do not set VOLE_TEST_NO_AUTH — that env poisons sudo-path mole_delete tests.
        let report = trash_analyze_paths(&["/System/Library".into()]);
        assert!(report.removed.is_empty());
        assert!(!report.errors.is_empty());
    }
}
