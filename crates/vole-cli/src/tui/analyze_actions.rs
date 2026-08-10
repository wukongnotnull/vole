//! analyze 删除 / Open / Preview 副作用（保护 + 废纸篓漏斗）。

use std::path::{Component, Path};

use vole_core::delete::{
    mole_delete, DeleteMode, DeletionLogger, MoleDeleteError, MoleDeleteOptions,
};
use vole_core::oplog::OperationLogger;
use vole_core::protection::AppProtection;
use vole_core::vole_proto::AnalyzeOutput;
use vole_sys::macos::MacTrash;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TrashAnalyzeReport {
    pub removed: Vec<String>,
    pub errors: Vec<String>,
}

/// Move analyze-selected paths to Trash via `mole_delete` only (no parallel `rm`).
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
    let options = MoleDeleteOptions {
        mode: DeleteMode::Trash,
        dry_run: false,
        needs_sudo: false,
        privilege: None,
    };

    let mut report = TrashAnalyzeReport::default();
    for path in ordered {
        match mole_delete(
            &path,
            &protection,
            &[],
            MoleDeleteOptions {
                mode: options.mode,
                dry_run: options.dry_run,
                needs_sudo: options.needs_sudo,
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

pub fn apply_removals(out: &mut AnalyzeOutput, removed: &[String]) {
    let removed_set: std::collections::BTreeSet<&str> =
        removed.iter().map(String::as_str).collect();
    out.entries.retain(|e| !removed_set.contains(e.path.as_str()));
    out.large_files
        .retain(|e| !removed_set.contains(e.path.as_str()));
    out.total_size = out.entries.iter().map(|e| e.size.max(0)).sum();
}

pub const MAX_BATCH_OPEN: usize = 20;

pub fn open_argv(path: &str) -> Vec<String> {
    vec!["/usr/bin/open".into(), path.to_string()]
}

pub fn preview_target(path: &str, is_dir: bool) -> Option<Vec<String>> {
    if is_dir {
        return None;
    }
    Some(vec![
        "/usr/bin/qlmanage".into(),
        "-p".into(),
        path.to_string(),
    ])
}

pub fn spawn_detached(argv: &[String]) -> Result<(), String> {
    if argv.is_empty() {
        return Err("empty argv".into());
    }
    let mut cmd = std::process::Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[allow(dead_code)]
fn path_depth(path: &str) -> usize {
    Path::new(path)
        .components()
        .filter(|c| !matches!(c, Component::RootDir))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use vole_core::vole_proto::{AnalyzeEntry, AnalyzeFileEntry, AnalyzeOutput};

    #[test]
    fn trash_analyze_paths_uses_test_trash_dir() {
        let root = std::env::temp_dir().join(format!(
            "vole-analyze-del-{}-{}",
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
        assert!(report.removed.len() == 1);

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        std::env::remove_var("MOLE_DELETE_LOG");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn trash_analyze_paths_rejects_protected() {
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        let report = trash_analyze_paths(&["/System/Library".into()]);
        assert!(report.removed.is_empty());
        assert!(!report.errors.is_empty());
    }

    #[test]
    fn apply_removals_updates_entries_and_total() {
        let mut out = AnalyzeOutput {
            total_size: 300,
            entries: vec![
                AnalyzeEntry {
                    name: "a".into(),
                    path: "/tmp/a".into(),
                    size: 100,
                    is_dir: false,
                    ..Default::default()
                },
                AnalyzeEntry {
                    name: "b".into(),
                    path: "/tmp/b".into(),
                    size: 200,
                    is_dir: false,
                    ..Default::default()
                },
            ],
            large_files: vec![AnalyzeFileEntry {
                name: "a".into(),
                path: "/tmp/a".into(),
                size: 100,
            }],
            ..Default::default()
        };
        apply_removals(&mut out, &["/tmp/a".into()]);
        assert_eq!(out.entries.len(), 1);
        assert_eq!(out.entries[0].path, "/tmp/b");
        assert!(out.large_files.is_empty());
        assert_eq!(out.total_size, 200);
    }

    #[test]
    fn open_and_preview_argv_shapes() {
        assert_eq!(
            open_argv("/tmp/a"),
            vec!["/usr/bin/open".to_string(), "/tmp/a".to_string()]
        );
        assert_eq!(
            preview_target("/tmp/a.txt", false),
            Some(vec![
                "/usr/bin/qlmanage".to_string(),
                "-p".to_string(),
                "/tmp/a.txt".to_string()
            ])
        );
        assert!(preview_target("/tmp/dir", true).is_none());
    }
}
