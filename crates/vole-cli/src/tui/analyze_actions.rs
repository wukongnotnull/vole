//! analyze Open / Preview 副作用与列表更新（删除漏斗在 `vole_core::delete`）。

use vole_core::vole_proto::AnalyzeOutput;

pub use vole_core::delete::trash_analyze_paths;

pub fn apply_removals(out: &mut AnalyzeOutput, removed: &[String]) {
    let removed_set: std::collections::BTreeSet<&str> =
        removed.iter().map(String::as_str).collect();
    out.entries
        .retain(|e| !removed_set.contains(e.path.as_str()));
    out.large_files
        .retain(|e| !removed_set.contains(e.path.as_str()));
    out.total_size = out.entries.iter().map(|e| e.size.max(0)).sum();
}

pub const MAX_BATCH_OPEN: usize = 20;

pub fn open_argv(path: &str) -> Vec<String> {
    vec!["/usr/bin/open".into(), path.to_string()]
}

pub fn reveal_argv(path: &str) -> Vec<String> {
    vec!["/usr/bin/open".into(), "-R".into(), path.to_string()]
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

#[cfg(test)]
mod tests {
    use super::*;
    use vole_core::vole_proto::{AnalyzeEntry, AnalyzeFileEntry, AnalyzeOutput};

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

    #[test]
    fn reveal_argv_uses_open_r() {
        assert_eq!(
            reveal_argv("/tmp/a"),
            vec![
                "/usr/bin/open".to_string(),
                "-R".to_string(),
                "/tmp/a".to_string()
            ]
        );
    }
}
