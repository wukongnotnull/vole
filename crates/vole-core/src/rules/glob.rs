//! 规则 `paths` glob 展开（设计 6.2）。

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::rules::schema::Rule;
use crate::safety::normalize_policy_path;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GlobError {
    #[error("recursive glob `**` is not supported")]
    DoubleStar,
}

/// 展开单条规则路径模式，返回候选路径（无匹配时为空，不保留字面量 glob）。
pub fn expand_rule_path(pattern: &str, home: &Path) -> Result<Vec<PathBuf>, GlobError> {
    if pattern.contains("**") {
        return Err(GlobError::DoubleStar);
    }

    let expanded = expand_home_prefix(pattern, home);
    let normalized = normalize_policy_path(&expanded);
    let (base, segments) = split_normalized(&normalized);

    let wildcard_idx = segments.iter().position(|s| s.contains('*'));
    match wildcard_idx {
        None => {
            let full = segments.iter().fold(base, |p, s| p.join(s));
            Ok(if path_exists(&full) {
                vec![full]
            } else {
                Vec::new()
            })
        }
        Some(idx) => {
            let (literal, wild) = segments.split_at(idx);
            let prefix = literal.iter().fold(base, |p, s| p.join(s));
            if !glob_prefix_exists(&prefix) {
                return Ok(Vec::new());
            }
            Ok(expand_segments(&prefix, wild))
        }
    }
}

/// 展开规则全部 `paths` 字段（跳过含 `**` 的非法模式）。
pub fn collect_path_candidates(rule: &Rule, home: &Path) -> Vec<PathBuf> {
    rule.paths
        .iter()
        .filter_map(|pattern| expand_rule_path(pattern, home).ok())
        .flatten()
        .collect()
}

/// 仅在路径开头展开 `~` 为 `home`；不支持 `~user`。
fn expand_home_prefix(path: &str, home: &Path) -> String {
    let Some(home) = home.to_str() else {
        return path.to_string();
    };
    if let Some(rest) = path.strip_prefix("~/") {
        return format!("{home}/{rest}");
    }
    if path == "~" {
        return home.to_string();
    }
    path.to_string()
}

fn split_normalized(path: &str) -> (PathBuf, Vec<String>) {
    if path.starts_with('/') {
        let segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        (PathBuf::from("/"), segments)
    } else {
        let segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        (PathBuf::from("."), segments)
    }
}

fn expand_segments(base: &Path, segments: &[String]) -> Vec<PathBuf> {
    if segments.is_empty() {
        return if path_exists(base) {
            vec![base.to_path_buf()]
        } else {
            Vec::new()
        };
    }

    let head = &segments[0];
    let tail = &segments[1..];

    if head.contains('*') {
        expand_wildcard_segment(base, head, tail)
    } else {
        let next = base.join(head);
        if tail.is_empty() {
            return if path_exists(&next) {
                vec![next]
            } else {
                Vec::new()
            };
        }
        if is_traversable_dir(&next) {
            expand_segments(&next, tail)
        } else {
            Vec::new()
        }
    }
}

fn expand_wildcard_segment(base: &Path, pattern: &str, tail: &[String]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(base) else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !segment_matches(pattern, &name) {
            continue;
        }

        let path = entry.path();
        if tail.is_empty() {
            if path_exists(&path) {
                results.push(path);
            }
            continue;
        }

        if is_traversable_dir(&path) {
            results.extend(expand_segments(&path, tail));
        }
    }
    results
}

fn path_exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

/// glob 展开的前缀目录必须存在且为真实目录（不跟随 symlink）。
fn glob_prefix_exists(path: &Path) -> bool {
    match path.symlink_metadata() {
        Ok(meta) => meta.is_dir() && !meta.file_type().is_symlink(),
        Err(_) => false,
    }
}

/// 可继续 glob 展开的中间目录：必须是真实目录，不能是 symlink。
fn is_traversable_dir(path: &Path) -> bool {
    match path.symlink_metadata() {
        Ok(meta) => meta.is_dir() && !meta.file_type().is_symlink(),
        Err(_) => false,
    }
}

/// 单层路径段匹配：`*` 不跨 `/`，大小写敏感，默认不匹配隐藏文件。
pub(crate) fn segment_matches(pattern: &str, name: &str) -> bool {
    if pattern.contains('*') && name.starts_with('.') && !pattern.starts_with('.') {
        return false;
    }
    fnmatch_segment(pattern.as_bytes(), name.as_bytes())
}

fn fnmatch_segment(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.first() {
        None => name.is_empty(),
        Some(b'*') => (0..=name.len()).any(|i| fnmatch_segment(&pattern[1..], &name[i..])),
        Some(&p) => {
            name.first().is_some_and(|&n| p == n) && fnmatch_segment(&pattern[1..], &name[1..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn rejects_double_star() {
        let home = Path::new("/Users/test");
        assert_eq!(
            expand_rule_path("~/foo/**/bar", home),
            Err(GlobError::DoubleStar)
        );
    }

    #[test]
    fn tilde_expand_only_at_start() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        touch(&home.join("Library/Caches/app"));

        let got = expand_rule_path("~/Library/Caches/app", home).unwrap();
        assert_eq!(got, vec![home.join("Library/Caches/app")]);

        let mid = expand_rule_path("/tmp/~not-expanded", home).unwrap();
        assert!(mid.is_empty());
    }

    #[test]
    fn star_matches_single_segment_not_across_slash() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("a/one"));
        touch(&root.join("a/two"));
        touch(&root.join("b/three"));

        let pattern = root.join("a/*").to_string_lossy().into_owned();
        let mut got = expand_rule_path(&pattern, root).unwrap();
        got.sort();
        let mut want = vec![root.join("a/one"), root.join("a/two")];
        want.sort();
        assert_eq!(got, want);

        let deep = root.join("a/*/nope").to_string_lossy().into_owned();
        assert!(expand_rule_path(&deep, root).unwrap().is_empty());
    }

    #[test]
    fn star_does_not_match_hidden_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("visible"));
        touch(&root.join(".hidden"));

        let pattern = root.join("*").to_string_lossy().into_owned();
        let got = expand_rule_path(&pattern, root).unwrap();
        assert_eq!(got, vec![root.join("visible")]);

        let dot_pattern = root.join(".*").to_string_lossy().into_owned();
        let got_hidden = expand_rule_path(&dot_pattern, root).unwrap();
        assert_eq!(got_hidden, vec![root.join(".hidden")]);
    }

    #[test]
    fn no_match_returns_empty_not_literal() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let pattern = home.join("no/such/*").to_string_lossy().into_owned();
        assert!(expand_rule_path(&pattern, home).unwrap().is_empty());

        let literal = home.join("missing/file").to_string_lossy().into_owned();
        assert!(expand_rule_path(&literal, home).unwrap().is_empty());
    }

    #[test]
    fn matching_is_case_sensitive() {
        assert!(!segment_matches("Foo", "foo"));
        assert!(!segment_matches("Caches", "caches"));
        assert!(segment_matches("Foo", "Foo"));

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("CaseFile"));
        let pattern = root.join("case*").to_string_lossy().into_owned();
        assert!(expand_rule_path(&pattern, root).unwrap().is_empty());
    }

    #[test]
    fn glob_does_not_follow_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let real = root.join("real");
        fs::create_dir_all(real.join("inside")).unwrap();
        touch(&real.join("inside/file"));
        symlink(&real, root.join("link")).unwrap();

        let link_only = root.join("link").to_string_lossy().into_owned();
        let got = expand_rule_path(&link_only, root).unwrap();
        assert_eq!(got, vec![root.join("link")]);

        let wildcard = root.join("link/*").to_string_lossy().into_owned();
        assert!(expand_rule_path(&wildcard, root).unwrap().is_empty());
    }

    #[test]
    fn normalizes_before_expand() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        touch(&home.join("Library/Caches/app"));

        let pattern = format!("{}/Library//Caches//app/", home.display());
        let got = expand_rule_path(&pattern, home).unwrap();
        assert_eq!(got, vec![home.join("Library/Caches/app")]);
    }

    #[test]
    fn collect_path_candidates_flattens_rule_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        touch(&home.join("a/one"));
        touch(&home.join("b/two"));

        let rule = Rule {
            id: "t".into(),
            category: None,
            label: "t".into(),
            platform: vec![],
            paths: vec![
                format!("{}/a/*", home.display()),
                format!("{}/b/two", home.display()),
                "~/missing/**/x".into(),
            ],
            impact: None,
            disabled: false,
            last_verified: None,
            strategy: Default::default(),
            guards: Default::default(),
        };

        let got = collect_path_candidates(&rule, home);
        assert_eq!(got.len(), 2);
        assert!(got.contains(&home.join("a/one")));
        assert!(got.contains(&home.join("b/two")));
    }
}
