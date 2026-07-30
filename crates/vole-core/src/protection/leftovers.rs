//! Uninstall 残留发现（`find_app_files` 用户域子集）+ sibling guard。

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use std::sync::LazyLock;

static REVERSE_DNS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9][-A-Za-z0-9]*(\.[A-Za-z0-9][-A-Za-z0-9]*)+$").expect("regex")
});

static VERSION_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(.+)\s+(Nightly|Beta|Alpha|Dev|Canary|Preview|Insider|Edge|Stable|Release|RC|LTS|Developer Edition|Technology Preview)$",
    )
    .expect("regex")
});

/// mole `LAUNCH_AGENT_NAME_COMMON_WORDS`（大小写不敏感匹配整名）。
const COMMON_WORDS: &[&str] = &[
    "Music",
    "Notes",
    "Photos",
    "Finder",
    "Safari",
    "Preview",
    "Calendar",
    "Contacts",
    "Messages",
    "Reminders",
    "Clock",
    "Weather",
    "Stocks",
    "Books",
    "News",
    "Podcasts",
    "Voice",
    "Files",
    "Store",
    "System",
    "Helper",
    "Agent",
    "Daemon",
    "Service",
    "Update",
    "Sync",
    "Backup",
    "Cloud",
    "Manager",
    "Monitor",
    "Server",
    "Client",
    "Worker",
    "Runner",
    "Launcher",
    "Driver",
    "Plugin",
    "Extension",
    "Widget",
    "Utility",
];

/// 独立 CLI 状态目录：卸载 GUI 时不得当作残留（mole #993）。
const PRESERVED_HOME_REL: &[&str] = &[".claude", ".codex", ".local/share/opencode"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub app_path: PathBuf,
    pub bundle_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiblingPresence {
    pub other_app_paths: Vec<PathBuf>,
}

impl SiblingPresence {
    pub fn has_siblings(&self) -> bool {
        !self.other_app_paths.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftoverHit {
    pub path: PathBuf,
    pub label: String,
}

pub fn is_reverse_dns_bundle_id(bundle_id: &str) -> bool {
    !bundle_id.is_empty() && bundle_id != "unknown" && REVERSE_DNS.is_match(bundle_id)
}

pub fn is_rejected_generic_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.len() < 2 {
        return true;
    }
    COMMON_WORDS
        .iter()
        .any(|w| w.eq_ignore_ascii_case(trimmed))
}

pub fn naming_variants(bundle_id: &str, display_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let app_name = display_name.trim();
    if app_name.len() >= 2 && !is_rejected_generic_name(app_name) {
        push_unique(&mut out, app_name.to_string());
        let nospace: String = app_name.chars().filter(|c| !c.is_whitespace()).collect();
        let underscore = app_name.replace(' ', "_");
        let hyphen = app_name.replace(' ', "-");
        let lower = app_name.to_ascii_lowercase();
        let lower_nospace = nospace.to_ascii_lowercase();
        let lower_hyphen = hyphen.to_ascii_lowercase();
        let lower_underscore = underscore.to_ascii_lowercase();

        if app_name.contains(' ') && app_name.len() > 3 {
            push_unique(&mut out, nospace.clone());
            push_unique(&mut out, underscore);
            push_unique(&mut out, hyphen);
            push_unique(&mut out, lower_nospace);
            push_unique(&mut out, lower_hyphen);
            push_unique(&mut out, lower_underscore);
        }
        push_unique(&mut out, lower);

        if let Some(caps) = VERSION_SUFFIX.captures(app_name) {
            let base = caps.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            if base.len() > 2 {
                push_unique(&mut out, base.to_string());
                push_unique(&mut out, base.to_ascii_lowercase());
            }
        }
    }
    if is_reverse_dns_bundle_id(bundle_id) {
        push_unique(&mut out, bundle_id.to_string());
    }
    out
}

pub fn find_bundle_siblings(
    bundle_id: &str,
    except_app: &Path,
    search_roots: &[PathBuf],
) -> SiblingPresence {
    let mut other = Vec::new();
    if !is_reverse_dns_bundle_id(bundle_id) {
        return SiblingPresence {
            other_app_paths: other,
        };
    }
    let except = fs::canonicalize(except_app).unwrap_or_else(|_| except_app.to_path_buf());
    for root in search_roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            let canon = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if canon == except {
                continue;
            }
            if read_bundle_id(&path).as_deref() == Some(bundle_id) {
                other.push(path);
            }
        }
    }
    SiblingPresence {
        other_app_paths: other,
    }
}

/// 有 sibling 时跳过共享残留域，只允许调用方单独处理当前 `.app`。
pub fn find_app_leftovers(
    identity: &AppIdentity,
    home: &Path,
    siblings: &SiblingPresence,
) -> Vec<LeftoverHit> {
    if siblings.has_siblings() {
        return Vec::new();
    }
    if identity.display_name.trim().len() < 2 && !is_reverse_dns_bundle_id(&identity.bundle_id) {
        return Vec::new();
    }

    let variants = naming_variants(&identity.bundle_id, &identity.display_name);
    let mut hits = Vec::new();
    let library = home.join("Library");

    for v in &variants {
        for rel in [
            format!("Application Support/{v}"),
            format!("Caches/{v}"),
            format!("Logs/{v}"),
            format!("Preferences/{v}"),
            format!("Preferences/{v}.plist"),
            format!("Saved Application State/{v}.savedState"),
            format!("Containers/{v}"),
            format!("HTTPStorages/{v}"),
            format!("Cookies/{v}.binarycookies"),
            format!("WebKit/{v}"),
            format!("Application Scripts/{v}"),
        ] {
            push_if_exists(&mut hits, library.join(&rel), v);
        }
        push_if_exists(&mut hits, home.join(".config").join(v), v);
        push_if_exists(&mut hits, home.join(".cache").join(v), v);
        push_if_exists(&mut hits, home.join(".local/share").join(v), v);
        if !v.contains('/') {
            push_if_exists(&mut hits, home.join(format!(".{v}")), v);
        }
    }

    if is_reverse_dns_bundle_id(&identity.bundle_id) {
        let agents = library.join("LaunchAgents");
        if agents.is_dir() {
            if let Ok(entries) = fs::read_dir(&agents) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name == format!("{}.plist", identity.bundle_id)
                        || name.starts_with(&format!("{}.", identity.bundle_id))
                            && name.ends_with(".plist")
                    {
                        let path = entry.path();
                        if !is_preserved_home_state(home, &path) {
                            hits.push(LeftoverHit {
                                path,
                                label: identity.bundle_id.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    hits.retain(|h| !is_preserved_home_state(home, &h.path));
    hits.sort_by(|a, b| a.path.cmp(&b.path));
    hits.dedup_by(|a, b| a.path == b.path);
    hits
}

pub fn read_bundle_id(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    let data = fs::read(&plist_path).ok()?;
    let value = plist::Value::from_reader(std::io::Cursor::new(data)).ok()?;
    value
        .as_dictionary()
        .and_then(|d| d.get("CFBundleIdentifier"))
        .and_then(|v| v.as_string())
        .map(str::to_string)
}

pub fn read_display_name(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    let data = fs::read(&plist_path).ok()?;
    let value = plist::Value::from_reader(std::io::Cursor::new(data)).ok()?;
    let dict = value.as_dictionary()?;
    dict.get("CFBundleDisplayName")
        .or_else(|| dict.get("CFBundleName"))
        .and_then(|v| v.as_string())
        .map(str::to_string)
        .or_else(|| {
            app_path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
}

fn push_unique(out: &mut Vec<String>, s: String) {
    if !s.is_empty() && !out.iter().any(|x| x == &s) {
        out.push(s);
    }
}

fn push_if_exists(hits: &mut Vec<LeftoverHit>, path: PathBuf, label: &str) {
    if path.exists() {
        hits.push(LeftoverHit {
            path,
            label: label.to_string(),
        });
    }
}

fn is_preserved_home_state(home: &Path, path: &Path) -> bool {
    for rel in PRESERVED_HOME_REL {
        let preserved = home.join(rel);
        if path == preserved || path.starts_with(&preserved) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn naming_variants_hyphen_and_nospace() {
        let v = naming_variants("com.example.maestro", "Maestro Studio");
        assert!(v.iter().any(|s| s == "maestro-studio"));
        assert!(v.iter().any(|s| s == "MaestroStudio"));
    }

    #[test]
    fn empty_or_generic_name_rejected() {
        assert!(is_rejected_generic_name(""));
        assert!(is_rejected_generic_name("a"));
        assert!(is_rejected_generic_name("Helper"));
        assert!(!is_rejected_generic_name("Maestro Studio"));
    }

    #[test]
    fn leftovers_finds_hyphen_variant_and_skips_empty_name() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        fs::create_dir_all(home.join("Library/Application Support/maestro-studio")).unwrap();
        let identity = AppIdentity {
            app_path: home.join("Apps/Maestro Studio.app"),
            bundle_id: "com.example.maestro".into(),
            display_name: "Maestro Studio".into(),
        };
        let hits = find_app_leftovers(&identity, home, &SiblingPresence::default());
        assert!(hits.iter().any(|h| h
            .path
            .ends_with("Library/Application Support/maestro-studio")));

        let empty = AppIdentity {
            app_path: home.join("Apps/X.app"),
            bundle_id: "unknown".into(),
            display_name: "".into(),
        };
        assert!(find_app_leftovers(&empty, home, &SiblingPresence::default()).is_empty());
    }

    #[test]
    fn sibling_skips_shared_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let apps = home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        let a = apps.join("Foo.app");
        let b = apps.join("Foo Copy.app");
        write_minimal_app(&a, "com.example.foo");
        write_minimal_app(&b, "com.example.foo");
        fs::create_dir_all(home.join("Library/Caches/com.example.foo")).unwrap();

        let siblings = find_bundle_siblings("com.example.foo", &a, &[apps]);
        assert!(siblings.has_siblings());
        let identity = AppIdentity {
            app_path: a,
            bundle_id: "com.example.foo".into(),
            display_name: "Foo".into(),
        };
        assert!(find_app_leftovers(&identity, home, &siblings).is_empty());
    }

    #[test]
    fn preserves_claude_home_state() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(home.join("Library/Caches/Claude")).unwrap();
        let identity = AppIdentity {
            app_path: home.join("Applications/Claude.app"),
            bundle_id: "com.anthropic.claudefordesktop".into(),
            display_name: "Claude".into(),
        };
        let hits = find_app_leftovers(&identity, home, &SiblingPresence::default());
        assert!(!hits.iter().any(|h| h.path.ends_with(".claude")));
        assert!(hits
            .iter()
            .any(|h| h.path.ends_with("Library/Caches/Claude")));
    }

    fn write_minimal_app(app: &Path, bundle_id: &str) {
        let contents = app.join("Contents");
        fs::create_dir_all(&contents).unwrap();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{bundle_id}</string>
<key>CFBundleName</key><string>Foo</string>
</dict></plist>"#
        );
        fs::write(contents.join("Info.plist"), plist).unwrap();
    }
}
