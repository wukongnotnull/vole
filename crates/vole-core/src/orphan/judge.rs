//! Orphan 判定用的纯函数（年龄、敏感族、路径 → bundle id）与完整 judge。

use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::protection::{is_reverse_dns_bundle_id, should_protect_data, ProtectionCatalog};

use super::deps::OrphanDeps;
use super::{
    CLAUDE_DESKTOP_BUNDLE_ID, DEFAULT_CLAUDE_VM_ORPHAN_AGE_DAYS, DEFAULT_ORPHAN_AGE_DAYS,
    MIN_ORPHAN_AGE_DAYS,
};

/// 一次 orphan 判定上下文（安装集合已预计算）。
pub struct OrphanJudge<'a> {
    pub catalog: &'a ProtectionCatalog,
    pub deps: &'a dyn OrphanDeps,
    pub installed: &'a HashSet<String>,
    pub age_days: u32,
    pub now: SystemTime,
}

impl OrphanJudge<'_> {
    /// `true` = 可标为 orphan（候选删除）。
    pub fn is_bundle_orphaned(&self, bundle_id: &str, _path: &Path, mtime: SystemTime) -> bool {
        if should_protect_data(bundle_id, self.catalog) {
            return false;
        }
        if is_sensitive_orphan_bundle(bundle_id) {
            return false;
        }
        if self.installed.contains(bundle_id) {
            return false;
        }
        if is_system_component_bundle(bundle_id) {
            return false;
        }
        let age = match self.now.duration_since(mtime) {
            Ok(d) => d,
            Err(_) => return false, // 未来 mtime → 不删
        };
        let threshold = Duration::from_secs(u64::from(self.age_days) * 86400);
        if age < threshold {
            return false;
        }
        if is_reverse_dns_bundle_id(bundle_id) {
            if !self.deps.spotlight_available() {
                return false;
            }
            match self.deps.mdfind_bundle(bundle_id) {
                Ok(true) => return false,
                Ok(false) => {}
                Err(_) => return false,
            }
        }
        true
    }

    /// `true` = Claude workspace VM bundle 可标为 orphan（对齐 Mole `is_claude_vm_bundle_orphaned`）。
    pub fn is_claude_vm_bundle_orphaned(
        &self,
        _path: &Path,
        mtime: SystemTime,
        age_days: u32,
    ) -> bool {
        if self.deps.claude_desktop_running() {
            return false;
        }
        if self.installed.contains(CLAUDE_DESKTOP_BUNDLE_ID) {
            return false;
        }
        let age = match self.now.duration_since(mtime) {
            Ok(d) => d,
            Err(_) => return false,
        };
        let threshold = Duration::from_secs(u64::from(age_days) * 86400);
        if age < threshold {
            return false;
        }
        if !self.deps.spotlight_available() {
            return false;
        }
        match self.deps.mdfind_bundle(CLAUDE_DESKTOP_BUNDLE_ID) {
            Ok(true) => false,
            Ok(false) => true,
            Err(_) => false,
        }
    }
}

/// 读 `MOLE_ORPHAN_AGE_DAYS`；非法或低于下限则回退默认。
pub fn orphan_age_days_from_env() -> u32 {
    orphan_age_days_from_raw(std::env::var("MOLE_ORPHAN_AGE_DAYS").ok().as_deref())
}

pub fn orphan_age_days_from_raw(raw: Option<&str>) -> u32 {
    let Some(s) = raw else {
        return DEFAULT_ORPHAN_AGE_DAYS;
    };
    let Ok(n) = s.trim().parse::<u32>() else {
        return DEFAULT_ORPHAN_AGE_DAYS;
    };
    if n < MIN_ORPHAN_AGE_DAYS {
        return DEFAULT_ORPHAN_AGE_DAYS;
    }
    n
}

/// 读 `MOLE_CLAUDE_VM_ORPHAN_AGE_DAYS`；非法或空则回退默认 7。
pub fn claude_vm_orphan_age_days_from_env() -> u32 {
    claude_vm_orphan_age_days_from_raw(
        std::env::var("MOLE_CLAUDE_VM_ORPHAN_AGE_DAYS")
            .ok()
            .as_deref(),
    )
}

pub fn claude_vm_orphan_age_days_from_raw(raw: Option<&str>) -> u32 {
    let Some(s) = raw.filter(|s| !s.trim().is_empty()) else {
        return DEFAULT_CLAUDE_VM_ORPHAN_AGE_DAYS;
    };
    match s.trim().parse::<u32>() {
        Ok(n) if n >= 1 => n,
        _ => DEFAULT_CLAUDE_VM_ORPHAN_AGE_DAYS,
    }
}

/// Claude Desktop Application Support 下的 `*.bundle`（B4.1）。
pub fn is_claude_vm_bundle_path(path: &Path, home: &Path) -> bool {
    let claude_root = home.join("Library/Application Support/Claude");
    if !path.starts_with(&claude_root) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.ends_with(".bundle")
}

/// 对齐 Mole `ORPHAN_NEVER_DELETE_PATTERNS`（大小写不敏感）。
pub fn is_sensitive_orphan_bundle(bundle_id: &str) -> bool {
    let lower = bundle_id.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "1password",
        "keychain",
        "bitwarden",
        "lastpass",
        "keepass",
        "dashlane",
        "enpass",
        "ssh",
        "gpg",
        "gnupg",
    ];
    if NEEDLES.iter().any(|n| lower.contains(n)) {
        return true;
    }
    lower.starts_with("com.apple.keychain")
}

/// 对齐 Mole `is_bundle_orphaned` 系统组件 case。
pub fn is_system_component_bundle(bundle_id: &str) -> bool {
    matches!(
        bundle_id.to_ascii_lowercase().as_str(),
        "loginwindow"
            | "dock"
            | "systempreferences"
            | "systemsettings"
            | "settings"
            | "controlcenter"
            | "finder"
            | "safari"
    )
}

/// 仅 com / org / net / io 顶层名（刻意不含 dev.* / app.*）。
pub fn matches_orphan_name_prefix(name: &str) -> bool {
    let base = name.strip_suffix(".savedState").unwrap_or(name);
    let base = base.strip_suffix(".plist").unwrap_or(base);
    let base = base.strip_suffix(".binarycookies").unwrap_or(base);
    base.starts_with("com.")
        || base.starts_with("org.")
        || base.starts_with("net.")
        || base.starts_with("io.")
}

/// 从 orphan 候选路径抽出 bundle id。
pub fn bundle_id_from_orphan_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let mut id = name.to_string();
    for suffix in [".savedState", ".plist", ".binarycookies"] {
        if let Some(stripped) = id.strip_suffix(suffix) {
            id = stripped.to_string();
            break;
        }
    }
    if id.is_empty() || !matches_orphan_name_prefix(&id) {
        return None;
    }
    Some(id)
}

pub fn resource_kind_label(path: &Path) -> &'static str {
    let s = path.to_string_lossy();
    if s.contains("/Library/Caches/") || s.ends_with("/Library/Caches") {
        "Caches"
    } else if s.contains("/Library/Logs/") || s.ends_with("/Library/Logs") {
        "Logs"
    } else if s.contains("/Saved Application State/") {
        "States"
    } else {
        "Data"
    }
}

pub fn orphan_label(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.contains("/Application Support/Claude/") && s.contains(".bundle") {
        return "Orphaned Claude workspace VM".into();
    }
    let kind = resource_kind_label(path);
    let id = bundle_id_from_orphan_path(path).unwrap_or_else(|| "unknown".into());
    format!("Orphaned {kind}: {id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orphan::FakeOrphanDeps;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn age_clamp_rejects_zero_and_garbage() {
        assert_eq!(orphan_age_days_from_raw(None), 30);
        assert_eq!(orphan_age_days_from_raw(Some("0")), 30);
        assert_eq!(orphan_age_days_from_raw(Some("6")), 30);
        assert_eq!(orphan_age_days_from_raw(Some("7")), 7);
        assert_eq!(orphan_age_days_from_raw(Some("30")), 30);
        assert_eq!(orphan_age_days_from_raw(Some("nope")), 30);
        assert_eq!(orphan_age_days_from_raw(Some("-1")), 30);
    }

    #[test]
    fn sensitive_and_system_denylists() {
        assert!(is_sensitive_orphan_bundle("com.1password.1password"));
        assert!(is_sensitive_orphan_bundle("com.apple.keychain"));
        assert!(is_sensitive_orphan_bundle("org.gpg.agent"));
        assert!(is_system_component_bundle("finder"));
        assert!(is_system_component_bundle("safari"));
        assert!(!is_sensitive_orphan_bundle("com.example.cache"));
    }

    #[test]
    fn bundle_id_and_prefix_from_path() {
        let p = Path::new("/tmp/Library/Caches/com.example.app");
        assert_eq!(
            bundle_id_from_orphan_path(p).as_deref(),
            Some("com.example.app")
        );
        assert!(matches_orphan_name_prefix("com.example.app"));
        assert!(!matches_orphan_name_prefix("dev.orbstack.OrbStack"));
        let s = Path::new("/tmp/Library/Saved Application State/com.foo.savedState");
        assert_eq!(bundle_id_from_orphan_path(s).as_deref(), Some("com.foo"));
        assert_eq!(orphan_label(p), "Orphaned Caches: com.example.app");
    }

    #[test]
    fn orphan_when_old_and_not_installed() {
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            mdfind: HashMap::from([("com.gone.app".into(), Ok(false))]),
            scan_error: false,
            ..Default::default()
        };
        let installed = HashSet::new();
        let catalog = ProtectionCatalog::embedded();
        let judge = OrphanJudge {
            catalog: &catalog,
            deps: &deps,
            installed: &installed,
            age_days: 30,
            now: SystemTime::now(),
        };
        let mtime = SystemTime::now() - Duration::from_secs(40 * 86400);
        assert!(judge.is_bundle_orphaned(
            "com.gone.app",
            Path::new("/tmp/Library/Caches/com.gone.app"),
            mtime
        ));
    }

    #[test]
    fn not_orphan_when_spotlight_disabled() {
        let deps = FakeOrphanDeps {
            spotlight: false,
            installed: HashSet::new(),
            mdfind: HashMap::from([("com.gone.app".into(), Ok(false))]),
            scan_error: false,
            ..Default::default()
        };
        let installed = HashSet::new();
        let catalog = ProtectionCatalog::embedded();
        let judge = OrphanJudge {
            catalog: &catalog,
            deps: &deps,
            installed: &installed,
            age_days: 30,
            now: SystemTime::now(),
        };
        let mtime = SystemTime::now() - Duration::from_secs(40 * 86400);
        assert!(!judge.is_bundle_orphaned(
            "com.gone.app",
            Path::new("/tmp/Library/Caches/com.gone.app"),
            mtime
        ));
    }

    #[test]
    fn not_orphan_when_mdfind_errors() {
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            mdfind: HashMap::from([(
                "com.gone.app".into(),
                Err(crate::orphan::OrphanProbeError::Unavailable),
            )]),
            scan_error: false,
            ..Default::default()
        };
        let installed = HashSet::new();
        let catalog = ProtectionCatalog::embedded();
        let judge = OrphanJudge {
            catalog: &catalog,
            deps: &deps,
            installed: &installed,
            age_days: 30,
            now: SystemTime::now(),
        };
        let mtime = SystemTime::now() - Duration::from_secs(40 * 86400);
        assert!(!judge.is_bundle_orphaned(
            "com.gone.app",
            Path::new("/tmp/Library/Caches/com.gone.app"),
            mtime
        ));
    }

    #[test]
    fn not_orphan_when_installed() {
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::from(["com.keep.app".into()]),
            mdfind: HashMap::new(),
            scan_error: false,
            ..Default::default()
        };
        let installed = deps.installed.clone();
        let catalog = ProtectionCatalog::embedded();
        let judge = OrphanJudge {
            catalog: &catalog,
            deps: &deps,
            installed: &installed,
            age_days: 30,
            now: SystemTime::now(),
        };
        let mtime = SystemTime::now() - Duration::from_secs(40 * 86400);
        assert!(!judge.is_bundle_orphaned(
            "com.keep.app",
            Path::new("/tmp/Library/Caches/com.keep.app"),
            mtime
        ));
    }

    #[test]
    fn claude_vm_age_defaults_and_invalid() {
        assert_eq!(claude_vm_orphan_age_days_from_raw(None), 7);
        assert_eq!(claude_vm_orphan_age_days_from_raw(Some("")), 7);
        assert_eq!(claude_vm_orphan_age_days_from_raw(Some("nope")), 7);
        assert_eq!(claude_vm_orphan_age_days_from_raw(Some("14")), 14);
        assert_eq!(claude_vm_orphan_age_days_from_raw(Some("0")), 7);
    }

    #[test]
    fn is_claude_vm_bundle_path_only_under_claude_support() {
        let home = Path::new("/Users/t");
        assert!(is_claude_vm_bundle_path(
            Path::new("/Users/t/Library/Application Support/Claude/vm_bundles/x.bundle"),
            home,
        ));
        assert!(!is_claude_vm_bundle_path(
            Path::new("/Users/t/Library/Caches/com.foo.bar"),
            home,
        ));
        assert!(!is_claude_vm_bundle_path(
            Path::new("/Users/t/Library/Application Support/Other/x.bundle"),
            home,
        ));
        assert!(!is_claude_vm_bundle_path(
            Path::new("/Users/t/Library/Application Support/Claude/notes.txt"),
            home,
        ));
    }

    #[test]
    fn orphan_label_for_claude_vm() {
        let p = Path::new("/Users/t/Library/Application Support/Claude/vm_bundles/x.bundle");
        assert_eq!(orphan_label(p), "Orphaned Claude workspace VM");
    }

    fn claude_judge<'a>(
        deps: &'a FakeOrphanDeps,
        installed: &'a HashSet<String>,
        catalog: &'a ProtectionCatalog,
    ) -> OrphanJudge<'a> {
        OrphanJudge {
            catalog,
            deps,
            installed,
            age_days: 30,
            now: SystemTime::now(),
        }
    }

    #[test]
    fn claude_vm_judge_gates() {
        let catalog = ProtectionCatalog::embedded();
        let path = Path::new("/Users/t/Library/Application Support/Claude/vm_bundles/x.bundle");
        let old = SystemTime::now() - Duration::from_secs(10 * 86400);
        let young = SystemTime::now() - Duration::from_secs(1 * 86400);
        let id = CLAUDE_DESKTOP_BUNDLE_ID;

        let running = FakeOrphanDeps {
            claude_running: true,
            spotlight: true,
            mdfind: HashMap::from([(id.into(), Ok(false))]),
            ..Default::default()
        };
        let empty = HashSet::new();
        assert!(
            !claude_judge(&running, &empty, &catalog).is_claude_vm_bundle_orphaned(path, old, 7)
        );

        let installed_set = HashSet::from([id.to_string()]);
        let deps_inst = FakeOrphanDeps {
            spotlight: true,
            mdfind: HashMap::from([(id.into(), Ok(false))]),
            ..Default::default()
        };
        assert!(!claude_judge(&deps_inst, &installed_set, &catalog)
            .is_claude_vm_bundle_orphaned(path, old, 7));

        let deps_young = FakeOrphanDeps {
            spotlight: true,
            mdfind: HashMap::from([(id.into(), Ok(false))]),
            ..Default::default()
        };
        assert!(!claude_judge(&deps_young, &empty, &catalog)
            .is_claude_vm_bundle_orphaned(path, young, 7));

        let deps_spot = FakeOrphanDeps {
            spotlight: false,
            mdfind: HashMap::from([(id.into(), Ok(false))]),
            ..Default::default()
        };
        assert!(
            !claude_judge(&deps_spot, &empty, &catalog).is_claude_vm_bundle_orphaned(path, old, 7)
        );

        let deps_md = FakeOrphanDeps {
            spotlight: true,
            mdfind: HashMap::from([(id.into(), Ok(true))]),
            ..Default::default()
        };
        assert!(
            !claude_judge(&deps_md, &empty, &catalog).is_claude_vm_bundle_orphaned(path, old, 7)
        );

        let deps_err = FakeOrphanDeps {
            spotlight: true,
            mdfind: HashMap::from([(id.into(), Err(crate::orphan::OrphanProbeError::Unavailable))]),
            ..Default::default()
        };
        assert!(
            !claude_judge(&deps_err, &empty, &catalog).is_claude_vm_bundle_orphaned(path, old, 7)
        );

        let deps_ok = FakeOrphanDeps {
            spotlight: true,
            mdfind: HashMap::from([(id.into(), Ok(false))]),
            ..Default::default()
        };
        assert!(claude_judge(&deps_ok, &empty, &catalog).is_claude_vm_bundle_orphaned(path, old, 7));
    }
}
