//! 安装中的 .app / LaunchAgents 扫描（无磁盘缓存）。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::protection::read_bundle_id;

/// 默认扫描根（对齐 Mole `scan_installed_apps`）。
pub fn default_app_scan_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        home.join("Applications"),
        PathBuf::from("/opt/homebrew/Caskroom"),
        PathBuf::from("/usr/local/Caskroom"),
        home.join("Library/Application Support/Setapp/Applications"),
    ]
}

/// 扫描各根下 `*.app` 的 CFBundleIdentifier。任一可读根成功即 Ok；全部不可访问仍 Ok(空)——
/// 调用方应结合 Library/Caches 可读性决定是否整体跳过 orphan。
pub fn scan_app_dirs_for_bundle_ids(home: &Path) -> Result<HashSet<String>, ()> {
    let mut set = HashSet::new();
    for root in default_app_scan_roots(home) {
        if !root.is_dir() {
            continue;
        }
        collect_apps_under(&root, 0, 3, &mut set);
    }
    Ok(set)
}

fn collect_apps_under(dir: &Path, depth: usize, max_depth: usize, out: &mut HashSet<String>) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_app = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".app"));
        if is_app && path.is_dir() {
            if let Some(id) = read_bundle_id(&path) {
                if !id.is_empty() && id != "missing value" {
                    out.insert(id);
                }
            }
            continue;
        }
        if path.is_dir() {
            collect_apps_under(&path, depth + 1, max_depth, out);
        }
    }
}

/// LaunchAgents plist basename（strip `.plist`）作为「仍活跃」证据。
pub fn collect_launch_agent_ids(home: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    for dir in [
        home.join("Library/LaunchAgents"),
        PathBuf::from("/Library/LaunchAgents"),
    ] {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("plist") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !stem.is_empty() {
                    set.insert(stem.to_string());
                }
            }
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_roots_include_home_applications() {
        let home = Path::new("/Users/test");
        let roots = default_app_scan_roots(home);
        assert!(roots.iter().any(|p| p.ends_with("Applications")));
        assert!(roots.iter().any(|p| p == Path::new("/Applications")));
    }

    #[test]
    fn scan_fixture_app_reads_bundle_id() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("Applications/Demo.app/Contents");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.demo.app</string>
</dict></plist>"#,
        )
        .unwrap();

        // 只扫 fixture 的 Applications，避免真机 /Applications。
        let mut set = HashSet::new();
        collect_apps_under(&tmp.path().join("Applications"), 0, 3, &mut set);
        assert!(set.contains("com.demo.app"));
    }
}
