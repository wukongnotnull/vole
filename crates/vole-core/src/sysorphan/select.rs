//! 三树扫描与 orphan 判定（spec §5）。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::orphan::OrphanDeps;
use crate::protection::is_reverse_dns_bundle_id;

use super::plist::read_launchd_program;
use super::probe::{is_package_managed_binary, probe_binary_presence, BinaryPresence};
use super::protect::is_known_protected;

const PHT_SKIP_EXTENSIONS: &[&str] = &[
    "json",
    "cfg",
    "conf",
    "me2me_enabled",
    "log",
    "dat",
    "db",
    "xml",
    "yml",
    "yaml",
    "ini",
    "txt",
    "pid",
    "sock",
    "lock",
];

/// `/Library` 扫描根（可经 `VOLE_TEST_SYSTEM_LIBRARY` 覆盖）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemServiceRoots {
    pub launch_daemons: PathBuf,
    pub launch_agents: PathBuf,
    pub privileged_helpers: PathBuf,
}

impl SystemServiceRoots {
    pub fn live() -> Self {
        Self {
            launch_daemons: PathBuf::from("/Library/LaunchDaemons"),
            launch_agents: PathBuf::from("/Library/LaunchAgents"),
            privileged_helpers: PathBuf::from("/Library/PrivilegedHelperTools"),
        }
    }

    /// `VOLE_TEST_SYSTEM_LIBRARY` 指向 fake `/Library` 根目录。
    pub fn from_env() -> Self {
        if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
            let base = PathBuf::from(base);
            return Self {
                launch_daemons: base.join("LaunchDaemons"),
                launch_agents: base.join("LaunchAgents"),
                privileged_helpers: base.join("PrivilegedHelperTools"),
            };
        }
        Self::live()
    }

    pub fn under(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();
        Self {
            launch_daemons: base.join("LaunchDaemons"),
            launch_agents: base.join("LaunchAgents"),
            privileged_helpers: base.join("PrivilegedHelperTools"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SysOrphanScanError {
    AllRootsInaccessible,
}

/// 扫描可读子集；三树皆不可列/零可读能力时返回 `AllRootsInaccessible`。
pub fn select_system_service_orphans(
    roots: &SystemServiceRoots,
    home: &Path,
    deps: &dyn OrphanDeps,
) -> Result<Vec<PathBuf>, SysOrphanScanError> {
    let installed = match deps.scan_installed_bundle_ids(home) {
        Ok(set) => set,
        // 安装扫描失败 → fail-closed：不出候选。
        Err(_) => return Ok(Vec::new()),
    };

    let mut out = Vec::new();
    let mut any_readable = false;

    match scan_plist_tree(&roots.launch_daemons, home, deps, &installed, &mut out) {
        TreeAccess::Readable => any_readable = true,
        TreeAccess::Inaccessible => {}
    }
    match scan_plist_tree(&roots.launch_agents, home, deps, &installed, &mut out) {
        TreeAccess::Readable => any_readable = true,
        TreeAccess::Inaccessible => {}
    }
    match scan_pht_tree(&roots.privileged_helpers, home, deps, &installed, &mut out) {
        TreeAccess::Readable => any_readable = true,
        TreeAccess::Inaccessible => {}
    }

    if !any_readable {
        return Err(SysOrphanScanError::AllRootsInaccessible);
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// apply 政策重验：路径须仍能被选为 orphan（与 plan 扫描规则同形）。失败则不得删。
pub fn recheck_system_service_entry(path: &Path, home: &Path, deps: &dyn OrphanDeps) -> bool {
    if !crate::privilege::path_allowed_for_privilege(path) {
        return false;
    }
    let roots = SystemServiceRoots::from_env();
    let Ok(orphans) = select_system_service_orphans(&roots, home, deps) else {
        return false;
    };
    orphans.iter().any(|p| p == path)
}

enum TreeAccess {
    Readable,
    Inaccessible,
}

fn scan_plist_tree(
    dir: &Path,
    home: &Path,
    deps: &dyn OrphanDeps,
    installed: &HashSet<String>,
    out: &mut Vec<PathBuf>,
) -> TreeAccess {
    let Ok(entries) = fs::read_dir(dir) else {
        return TreeAccess::Inaccessible;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".plist") {
            continue;
        }
        if name.starts_with("com.apple.") {
            continue;
        }
        let bundle_id = name.trim_end_matches(".plist");
        if is_plist_orphaned(&path, bundle_id, home, deps, installed) {
            out.push(path);
        }
    }
    TreeAccess::Readable
}

fn scan_pht_tree(
    dir: &Path,
    home: &Path,
    deps: &dyn OrphanDeps,
    installed: &HashSet<String>,
    out: &mut Vec<PathBuf>,
) -> TreeAccess {
    let Ok(entries) = fs::read_dir(dir) else {
        return TreeAccess::Inaccessible;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if pht_extension_skipped(name) {
            continue;
        }
        let bundle_id = name.trim_end_matches(".plist");
        if bundle_id.starts_with("com.apple.") {
            continue;
        }
        if is_known_protected(name, home, deps) || is_known_protected(bundle_id, home, deps) {
            continue;
        }
        if !(bundle_id.starts_with("com.")
            || bundle_id.starts_with("org.")
            || bundle_id.starts_with("net.")
            || bundle_id.starts_with("io."))
        {
            continue;
        }
        if bundle_has_installed_app(bundle_id, installed, deps) {
            continue;
        }
        out.push(path);
    }
    TreeAccess::Readable
}

fn pht_extension_skipped(filename: &str) -> bool {
    let Some((_, ext)) = filename.rsplit_once('.') else {
        return false; // extensionless helpers OK
    };
    PHT_SKIP_EXTENSIONS.contains(&ext)
}

fn is_plist_orphaned(
    plist: &Path,
    bundle_id: &str,
    home: &Path,
    deps: &dyn OrphanDeps,
    installed: &HashSet<String>,
) -> bool {
    let Some(program) = read_launchd_program(plist) else {
        return false;
    };

    match probe_binary_presence(&program) {
        BinaryPresence::PresentOrUnknowable => {
            if is_under_privileged_helpers(&program) {
                let helper_id = privileged_helper_bundle_id_from_binary(&program);
                !bundle_has_installed_app(&helper_id, installed, deps)
            } else {
                false
            }
        }
        BinaryPresence::Missing => {
            if is_package_managed_binary(&program) {
                return false;
            }
            if is_known_protected(bundle_id, home, deps) {
                return false;
            }
            true
        }
    }
}

fn is_under_privileged_helpers(path: &Path) -> bool {
    path.to_string_lossy()
        .contains("/Library/PrivilegedHelperTools/")
        || path
            .components()
            .any(|c| c.as_os_str() == "PrivilegedHelperTools")
}

/// Mole `_privileged_helper_bundle_id_from_binary` 可读子集。
pub fn privileged_helper_bundle_id_from_binary(binary: &Path) -> String {
    let s = binary.to_string_lossy();
    if let Some(idx) = s.find(".bundle/Contents/MacOS/") {
        let before = &s[..idx];
        if let Some(start) = before.rfind('/') {
            let dir_name = &before[start + 1..];
            return dir_name.trim_end_matches(".bundle").to_string();
        }
    }
    binary
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .trim_end_matches(".plist")
        .to_string()
}

fn bundle_has_installed_app(
    bundle_id: &str,
    installed: &HashSet<String>,
    deps: &dyn OrphanDeps,
) -> bool {
    if !is_reverse_dns_bundle_id(bundle_id) {
        return false;
    }
    if installed
        .iter()
        .any(|id| id.eq_ignore_ascii_case(bundle_id))
    {
        return true;
    }
    if !deps.spotlight_available() {
        return true;
    }
    deps.mdfind_bundle(bundle_id).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orphan::FakeOrphanDeps;
    use std::collections::HashMap;
    use std::os::unix::fs::PermissionsExt;

    fn scratch(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "vole-sysorphan-select-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_plist(path: &Path, program: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(
            path,
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Program</key>
  <string>{program}</string>
</dict>
</plist>
"#
            ),
        )
        .unwrap();
    }

    fn roots_under(base: &Path) -> SystemServiceRoots {
        for d in ["LaunchDaemons", "LaunchAgents", "PrivilegedHelperTools"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }
        SystemServiceRoots::under(base)
    }

    fn deps_empty() -> FakeOrphanDeps {
        FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::new(),
            mdfind: HashMap::new(),
            ..Default::default()
        }
    }

    #[test]
    fn selects_plist_with_missing_non_package_program() {
        let base = scratch("orphan-plist");
        let roots = roots_under(&base);
        let missing = base.join("nowhere/bin/gone");
        write_plist(
            &roots.launch_daemons.join("com.example.gone.plist"),
            &missing.to_string_lossy(),
        );
        let got = select_system_service_orphans(&roots, &base, &deps_empty()).unwrap();
        assert_eq!(got.len(), 1);
        assert!(got[0].ends_with("com.example.gone.plist"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn skips_com_apple_and_unreadable_plist() {
        let base = scratch("skip-apple");
        let roots = roots_under(&base);
        let missing = base.join("nowhere/bin/gone");
        write_plist(
            &roots.launch_daemons.join("com.apple.something.plist"),
            &missing.to_string_lossy(),
        );
        let denied = roots.launch_daemons.join("com.example.denied.plist");
        write_plist(&denied, &missing.to_string_lossy());
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000)).unwrap();
        let got = select_system_service_orphans(&roots, &base, &deps_empty()).unwrap();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(got.is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn skips_package_managed_missing_binary() {
        let base = scratch("pkg");
        let roots = roots_under(&base);
        write_plist(
            &roots.launch_agents.join("com.example.brew.plist"),
            "/opt/homebrew/bin/missing-tool",
        );
        let got = select_system_service_orphans(&roots, &base, &deps_empty()).unwrap();
        assert!(got.is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn pht_binary_present_parent_missing_is_orphan() {
        let base = scratch("pht1082");
        let roots = roots_under(&base);
        let helper = roots.privileged_helpers.join("com.example.Helper");
        fs::write(&helper, b"x").unwrap();
        write_plist(
            &roots.launch_daemons.join("com.example.Helper.plist"),
            &helper.to_string_lossy(),
        );
        // Program path contains PrivilegedHelperTools → #1082 branch.
        let got = select_system_service_orphans(&roots, &base, &deps_empty()).unwrap();
        assert!(
            got.iter().any(|p| p.ends_with("com.example.Helper.plist")),
            "got={got:?}"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn pht_binary_present_parent_installed_not_orphan() {
        let base = scratch("pht-ok");
        let roots = roots_under(&base);
        let helper = roots.privileged_helpers.join("com.example.Helper");
        fs::write(&helper, b"x").unwrap();
        write_plist(
            &roots.launch_daemons.join("com.example.Helper.plist"),
            &helper.to_string_lossy(),
        );
        let deps = FakeOrphanDeps {
            spotlight: true,
            installed: HashSet::from(["com.example.Helper".into()]),
            ..Default::default()
        };
        let got = select_system_service_orphans(&roots, &base, &deps).unwrap();
        assert!(!got.iter().any(|p| p.ends_with("com.example.Helper.plist")));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn homebrew_mxcl_always_protected() {
        let base = scratch("brew");
        let roots = roots_under(&base);
        let missing = base.join("nowhere/bin/gone");
        write_plist(
            &roots.launch_daemons.join("homebrew.mxcl.nginx.plist"),
            &missing.to_string_lossy(),
        );
        let got = select_system_service_orphans(&roots, &base, &deps_empty()).unwrap();
        assert!(got.is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn pht_skips_app_dir_json_and_non_reverse_dns() {
        let base = scratch("pht-skip");
        let roots = roots_under(&base);
        fs::create_dir_all(roots.privileged_helpers.join("Foo.app")).unwrap();
        fs::write(roots.privileged_helpers.join("notes.json"), b"{}").unwrap();
        fs::write(roots.privileged_helpers.join("not-dns-helper"), b"x").unwrap();
        fs::write(
            roots.privileged_helpers.join("com.example.orphanhelper"),
            b"x",
        )
        .unwrap();
        let got = select_system_service_orphans(&roots, &base, &deps_empty()).unwrap();
        assert_eq!(got.len(), 1);
        assert!(got[0].ends_with("com.example.orphanhelper"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn all_roots_inaccessible_errors() {
        let base = scratch("denied-all");
        let roots = roots_under(&base);
        for d in [
            &roots.launch_daemons,
            &roots.launch_agents,
            &roots.privileged_helpers,
        ] {
            fs::set_permissions(d, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let err = select_system_service_orphans(&roots, &base, &deps_empty()).unwrap_err();
        for d in [
            &roots.launch_daemons,
            &roots.launch_agents,
            &roots.privileged_helpers,
        ] {
            fs::set_permissions(d, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert_eq!(err, SysOrphanScanError::AllRootsInaccessible);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn one_inaccessible_root_still_scans_others() {
        let base = scratch("partial");
        let roots = roots_under(&base);
        let missing = base.join("nowhere/bin/gone");
        write_plist(
            &roots.launch_agents.join("com.example.partial.plist"),
            &missing.to_string_lossy(),
        );
        fs::set_permissions(&roots.launch_daemons, fs::Permissions::from_mode(0o000)).unwrap();
        let got = select_system_service_orphans(&roots, &base, &deps_empty()).unwrap();
        fs::set_permissions(&roots.launch_daemons, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(got.len(), 1);
        let _ = fs::remove_dir_all(&base);
    }
}
