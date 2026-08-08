//! Uninstall 系统 LaunchDaemons / `/Library` sudo 残留发现（W2a③）。
//!
//! 对齐 Mole `find_app_system_files` 主路径子集；广谱边缘不做。

use std::fs;
use std::path::{Path, PathBuf};

use crate::login_items::{percent_decode_token, percent_encode_token};
use crate::protection::{
    is_rejected_generic_name, is_reverse_dns_bundle_id, naming_variants, AppIdentity, SiblingPresence,
};

pub const SYSTEM_LEFTOVER_PREFIX: &str = "uninstall:system-leftover:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemLeftoverKind {
    Launchd,
    Pht,
    Library,
    Receipt,
}

impl SystemLeftoverKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Launchd => "launchd",
            Self::Pht => "pht",
            Self::Library => "library",
            Self::Receipt => "receipt",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "launchd" => Some(Self::Launchd),
            "pht" => Some(Self::Pht),
            "library" => Some(Self::Library),
            "receipt" => Some(Self::Receipt),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemLeftoverHit {
    pub path: PathBuf,
    pub kind: SystemLeftoverKind,
    pub label: String,
}

pub fn encode_system_leftover_rule_id(kind: SystemLeftoverKind, path: &Path) -> String {
    let token = percent_encode_token(&path.to_string_lossy());
    format!("{SYSTEM_LEFTOVER_PREFIX}{}:{token}", kind.as_str())
}

pub fn parse_system_leftover_rule_id(rule_id: &str) -> Option<(SystemLeftoverKind, PathBuf)> {
    let rest = rule_id.strip_prefix(SYSTEM_LEFTOVER_PREFIX)?;
    let (kind_s, token) = rest.split_once(':')?;
    let kind = SystemLeftoverKind::parse(kind_s)?;
    let path = percent_decode_token(token)?;
    if path.is_empty() {
        return None;
    }
    Some((kind, PathBuf::from(path)))
}

/// Mole `mole_name_starts_with_bundle_id_boundary`：basename 等于 id 或以 `id.` 开头。
pub fn name_starts_with_bundle_id_boundary(name_or_path: &str, bundle_id: &str) -> bool {
    if !is_reverse_dns_bundle_id(bundle_id) {
        return false;
    }
    let name = Path::new(name_or_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name_or_path);
    name == bundle_id || name.starts_with(&format!("{bundle_id}."))
}

pub fn system_library_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        return PathBuf::from(base);
    }
    PathBuf::from("/Library")
}

pub fn receipts_root() -> PathBuf {
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        if let Some(parent) = Path::new(&base).parent() {
            return parent.join("private/var/db/receipts");
        }
    }
    PathBuf::from("/private/var/db/receipts")
}

fn is_apple_basename(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("com.apple.")
}

/// 有 sibling 时返回空（对齐用户域 leftovers）。
pub fn find_system_leftovers(
    identity: &AppIdentity,
    siblings: &SiblingPresence,
) -> Vec<SystemLeftoverHit> {
    if siblings.has_siblings() {
        return Vec::new();
    }
    let root = system_library_root();
    let mut hits = Vec::new();

    scan_launchd(&root, identity, &mut hits);
    scan_pht(&root, identity, &mut hits);
    scan_library_exact(&root, identity, &mut hits);
    scan_receipts(identity, &mut hits);

    hits.sort_by(|a, b| a.path.cmp(&b.path));
    hits.dedup_by(|a, b| a.path == b.path);
    hits
}

fn scan_launchd(root: &Path, identity: &AppIdentity, hits: &mut Vec<SystemLeftoverHit>) {
    for dir_name in ["LaunchAgents", "LaunchDaemons"] {
        let dir = root.join(dir_name);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
                continue;
            };
            if !name.ends_with(".plist") {
                continue;
            }
            if is_apple_basename(&name) {
                continue;
            }

            let mut matched = false;
            if is_reverse_dns_bundle_id(&identity.bundle_id) {
                if name == format!("{}.plist", identity.bundle_id)
                    || (name.starts_with(&format!("{}.", identity.bundle_id))
                        && name.ends_with(".plist"))
                {
                    matched = true;
                }
            }
            let display = identity.display_name.trim();
            if !matched
                && display.len() >= 5
                && !is_rejected_generic_name(display)
                && name.contains(display)
            {
                matched = true;
            }
            if matched {
                hits.push(SystemLeftoverHit {
                    path,
                    kind: SystemLeftoverKind::Launchd,
                    label: name,
                });
            }
        }
    }
}

fn scan_pht(root: &Path, identity: &AppIdentity, hits: &mut Vec<SystemLeftoverHit>) {
    if !is_reverse_dns_bundle_id(&identity.bundle_id) {
        return;
    }
    let dir = root.join("PrivilegedHelperTools");
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            continue;
        };
        if is_apple_basename(&name) {
            continue;
        }
        if name_starts_with_bundle_id_boundary(&name, &identity.bundle_id) {
            hits.push(SystemLeftoverHit {
                path,
                kind: SystemLeftoverKind::Pht,
                label: name,
            });
        }
    }
}

fn scan_library_exact(root: &Path, identity: &AppIdentity, hits: &mut Vec<SystemLeftoverHit>) {
    let variants = naming_variants(&identity.bundle_id, &identity.display_name);
    let mut candidates: Vec<PathBuf> = Vec::new();
    for v in &variants {
        if v.is_empty() || v.contains('/') || v.contains("..") {
            continue;
        }
        candidates.push(root.join("Application Support").join(v));
        candidates.push(root.join("Preferences").join(v));
        candidates.push(root.join("Preferences").join(format!("{v}.plist")));
        candidates.push(root.join("Caches").join(v));
        candidates.push(root.join("Logs").join(v));
    }
    if is_reverse_dns_bundle_id(&identity.bundle_id) {
        let id = &identity.bundle_id;
        candidates.push(root.join("Receipts").join(format!("{id}.bom")));
        candidates.push(root.join("Receipts").join(format!("{id}.plist")));
    }
    for path in candidates {
        // 空 variant 已跳过；要求存在且为单层叶
        if path.file_name().is_none() || !path.exists() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_apple_basename(name) {
            continue;
        }
        hits.push(SystemLeftoverHit {
            label: name.to_string(),
            path,
            kind: SystemLeftoverKind::Library,
        });
    }
}

fn scan_receipts(identity: &AppIdentity, hits: &mut Vec<SystemLeftoverHit>) {
    if !is_reverse_dns_bundle_id(&identity.bundle_id) {
        return;
    }
    let dir = receipts_root();
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            continue;
        };
        if is_apple_basename(&name) {
            continue;
        }
        if name_starts_with_bundle_id_boundary(&name, &identity.bundle_id) {
            hits.push(SystemLeftoverHit {
                path,
                kind: SystemLeftoverKind::Receipt,
                label: name,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;
    use std::fs;
    use std::path::PathBuf;

    fn identity(bundle_id: &str, display_name: &str) -> AppIdentity {
        AppIdentity {
            app_path: PathBuf::from("/Applications/Foo.app"),
            bundle_id: bundle_id.into(),
            display_name: display_name.into(),
        }
    }

    #[test]
    fn rule_id_roundtrip_encodes_path() {
        let p = PathBuf::from("/Library/LaunchDaemons/com.example.plist");
        let id = encode_system_leftover_rule_id(SystemLeftoverKind::Launchd, &p);
        assert!(id.starts_with("uninstall:system-leftover:launchd:"));
        let (k, out) = parse_system_leftover_rule_id(&id).unwrap();
        assert_eq!(k, SystemLeftoverKind::Launchd);
        assert_eq!(out, p);
    }

    #[test]
    fn bundle_id_boundary_rejects_prefix_collision() {
        assert!(name_starts_with_bundle_id_boundary("com.foo.helper", "com.foo"));
        assert!(name_starts_with_bundle_id_boundary("com.foo", "com.foo"));
        assert!(!name_starts_with_bundle_id_boundary("com.foobar.plist", "com.foo"));
        assert!(!name_starts_with_bundle_id_boundary("com.foo", "not-dns"));
    }

    #[test]
    fn find_launchd_pht_library_and_skips_sibling() {
        let _guard = test_env::lock();
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("Library");
        for d in [
            "LaunchDaemons",
            "LaunchAgents",
            "PrivilegedHelperTools",
            "Application Support",
        ] {
            fs::create_dir_all(lib.join(d)).unwrap();
        }
        let receipts = tmp.path().join("private/var/db/receipts");
        fs::create_dir_all(&receipts).unwrap();

        fs::write(
            lib.join("LaunchDaemons/com.example.app.plist"),
            b"{}",
        )
        .unwrap();
        fs::write(
            lib.join("LaunchDaemons/com.example.app.helper.plist"),
            b"{}",
        )
        .unwrap();
        fs::write(
            lib.join("LaunchDaemons/com.example.other.plist"),
            b"{}",
        )
        .unwrap();
        fs::write(lib.join("LaunchDaemons/com.apple.evil.plist"), b"{}").unwrap();
        fs::write(lib.join("PrivilegedHelperTools/com.example.app.helper"), b"x").unwrap();
        fs::write(lib.join("PrivilegedHelperTools/com.example.other"), b"x").unwrap();
        fs::create_dir_all(lib.join("Application Support/Example App")).unwrap();
        fs::write(receipts.join("com.example.app.bom"), b"x").unwrap();

        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        let id = identity("com.example.app", "Example App");
        let hits = find_system_leftovers(&id, &SiblingPresence::default());
        assert!(hits.iter().any(|h| {
            h.kind == SystemLeftoverKind::Launchd
                && h.path.ends_with("com.example.app.plist")
        }));
        assert!(hits.iter().any(|h| {
            h.kind == SystemLeftoverKind::Launchd
                && h.path.ends_with("com.example.app.helper.plist")
        }));
        assert!(!hits
            .iter()
            .any(|h| h.path.ends_with("com.example.other.plist")));
        assert!(!hits
            .iter()
            .any(|h| h.path.ends_with("com.apple.evil.plist")));
        assert!(hits.iter().any(|h| {
            h.kind == SystemLeftoverKind::Pht && h.path.ends_with("com.example.app.helper")
        }));
        assert!(!hits
            .iter()
            .any(|h| h.path.ends_with("com.example.other")));
        assert!(hits.iter().any(|h| {
            h.kind == SystemLeftoverKind::Library && h.path.ends_with("Example App")
        }));
        assert!(hits.iter().any(|h| {
            h.kind == SystemLeftoverKind::Receipt && h.path.ends_with("com.example.app.bom")
        }));

        let sib = SiblingPresence {
            other_app_paths: vec![PathBuf::from("/Applications/Other.app")],
        };
        assert!(find_system_leftovers(&id, &sib).is_empty());

        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
    }

    #[test]
    fn name_glob_skips_short_and_common() {
        let _guard = test_env::lock();
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("Library");
        fs::create_dir_all(lib.join("LaunchDaemons")).unwrap();
        fs::write(lib.join("LaunchDaemons/foo-Helper-bar.plist"), b"{}").unwrap();
        std::env::set_var("VOLE_TEST_SYSTEM_LIBRARY", &lib);

        // "Helper" is COMMON_WORDS → rejected；无 reverse-dns 匹配
        let id = identity("unknown", "Helper");
        assert!(find_system_leftovers(&id, &SiblingPresence::default()).is_empty());

        std::env::remove_var("VOLE_TEST_SYSTEM_LIBRARY");
    }
}
