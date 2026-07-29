//! Mole 兼容的 clean whitelist 配置。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const HEADER: &str = "# Mole Whitelist - Protected paths won't be deleted";

fn config_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".config/mole/whitelist"))
        .unwrap_or_else(|| PathBuf::from(".config/mole/whitelist"))
}

pub fn load_clean() -> io::Result<Vec<String>> {
    let path = config_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = fs::read_to_string(&path)?;
    Ok(parse_lines(&text))
}

pub fn save_clean(patterns: &[String]) -> io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut unique: Vec<String> = Vec::new();
    for p in patterns {
        if !unique.iter().any(|u| u == p) {
            unique.push(p.clone());
        }
    }
    let mut out = String::from(HEADER);
    out.push('\n');
    for p in &unique {
        out.push_str(p);
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

fn parse_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// 简化 glob：精确相等，或 pattern 为 `prefix*` 前缀匹配。
pub fn is_match(path: &Path, patterns: &[String]) -> bool {
    let s = path.to_string_lossy();
    for pat in patterns {
        if pat.ends_with('*') {
            let prefix = &pat[..pat.len() - 1];
            if s.starts_with(prefix) {
                return true;
            }
        } else if s == pat.as_str() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;
    use std::path::Path;

    #[test]
    fn roundtrip_patterns() {
        let _guard = test_env::lock();
        let home = std::env::temp_dir().join(format!("vole-wl-{}", std::process::id()));
        std::env::set_var("HOME", home.join("h"));
        save_clean(&["/tmp/a*".into(), "/tmp/b".into()]).unwrap();
        let loaded = load_clean().unwrap();
        assert!(loaded.contains(&"/tmp/a*".to_string()));
        assert!(loaded.contains(&"/tmp/b".to_string()));
        std::env::remove_var("HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn prefix_star_matches() {
        assert!(is_match(Path::new("/tmp/abc"), &["/tmp/a*".into()]));
        assert!(!is_match(Path::new("/other"), &["/tmp/a*".into()]));
    }
}
