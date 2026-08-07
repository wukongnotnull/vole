//! Time Machine 失败中（`*.inProgress`）备份清理。
//!
//! 对齐 Mole `clean_time_machine_failed_backups`：fail-closed 门控、48h 安全窗、
//! apply 仅 `tmutil delete`（不走 sudo rm）。

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

/// `tm-failed-backups` 规则 id（1.28.0）。
pub const TM_FAILED_BACKUPS_RULE_ID: &str = "tm-failed-backups";

/// 安全窗（小时），对齐 Mole `MOLE_TM_BACKUP_SAFE_HOURS`。
pub const TM_BACKUP_SAFE_HOURS: u64 = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmRunningState {
    Running,
    Idle,
    Unknown,
}

/// Time Machine / 卷探测依赖（测试可注入）。
pub trait TmDeps: Send + Sync {
    fn tmutil_exists(&self) -> bool;
    fn auto_backup_configured(&self) -> bool;
    fn destination_configured(&self) -> bool;
    fn running_state(&self) -> TmRunningState;
    fn volumes_root(&self) -> PathBuf;
    fn fs_type(&self, vol: &Path) -> String;
    fn bundle_mount_point(&self, bundle: &Path) -> Option<PathBuf>;
    fn path_mtime(&self, path: &Path) -> Option<SystemTime>;
    fn dir_size_bytes(&self, path: &Path) -> u64;
    fn delete_backup(&self, path: &Path) -> Result<(), String>;
}

#[derive(Debug, Default, Clone)]
pub struct TmSelectResult {
    pub paths: Vec<PathBuf>,
    /// Running 或 Unknown 导致未扫描。
    pub skipped_busy: bool,
}

/// 门控步骤 1–5：全部通过才允许扫描卷。
pub fn gates_allow_scan(deps: &dyn TmDeps) -> bool {
    if !deps.tmutil_exists() {
        return false;
    }
    if !deps.auto_backup_configured() {
        return false;
    }
    if !deps.destination_configured() {
        return false;
    }
    if !deps.volumes_root().is_dir() {
        return false;
    }
    matches!(deps.running_state(), TmRunningState::Idle)
}

/// 目录名是否为失败中备份（Mole：`*.inProgress` / `*.inprogress`）。
pub fn is_tm_inprogress_dir_name(name: &str) -> bool {
    name.ends_with(".inProgress") || name.ends_with(".inprogress")
}

fn is_network_or_unknown_fs(fs: &str) -> bool {
    matches!(
        fs.to_ascii_lowercase().as_str(),
        "nfs" | "smbfs" | "afpfs" | "cifs" | "webdav" | "unknown"
    )
}

fn hours_old(mtime: SystemTime, now: SystemTime) -> Option<u64> {
    now.duration_since(mtime).ok().map(|d| d.as_secs() / 3600)
}

/// `{vol}/Backups.backupdb/**/*.inProgress` 深度≤3（相对 backupdb）。
/// Bundle 挂载点路径须由 `path_under_tm_bundle_mount` 另验，禁止仅凭深度放行。
pub fn path_allowed_for_tm_delete(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !is_tm_inprogress_dir_name(name) {
        return false;
    }
    let Some(rest) = path
        .to_str()
        .and_then(|s| s.split_once("/Backups.backupdb/").map(|(_, r)| r))
    else {
        return false;
    };
    let depth = rest.split('/').filter(|p| !p.is_empty()).count();
    (1..=3).contains(&depth)
}

/// 路径是否位于某 backupbundle/sparsebundle 的当前挂载点下（apply 复验用）。
pub fn path_under_tm_bundle_mount(deps: &dyn TmDeps, path: &Path) -> bool {
    if !path.is_absolute()
        || path.components().any(|c| matches!(c, Component::ParentDir))
        || !path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(is_tm_inprogress_dir_name)
    {
        return false;
    }
    let Some(vol) = volume_root_for_path(path, &deps.volumes_root()) else {
        return false;
    };
    let Ok(rd) = fs::read_dir(vol) else {
        return false;
    };
    for be in rd.flatten() {
        let bundle = be.path();
        let bn = be.file_name().to_string_lossy().into_owned();
        if !(bn.ends_with(".backupbundle") || bn.ends_with(".sparsebundle")) {
            continue;
        }
        let Some(mount) = deps.bundle_mount_point(&bundle) else {
            continue;
        };
        if path.starts_with(&mount) {
            let rest = path.strip_prefix(&mount).ok();
            let depth = rest
                .map(|r| {
                    r.components()
                        .filter(|c| matches!(c, Component::Normal(_)))
                        .count()
                })
                .unwrap_or(0);
            if (1..=3).contains(&depth) {
                return true;
            }
        }
    }
    false
}

fn volume_root_for_path(path: &Path, volumes_root: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(volumes_root).ok()?;
    let mut comps = rel.components();
    let Component::Normal(vol_name) = comps.next()? else {
        return None;
    };
    Some(volumes_root.join(vol_name))
}

pub fn path_allowed_for_tm_apply(deps: &dyn TmDeps, path: &Path) -> bool {
    path_allowed_for_tm_delete(path) || path_under_tm_bundle_mount(deps, path)
}

fn walk_inprogress(root: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    let Ok(rd) = fs::read_dir(root) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if !meta.is_dir() {
            continue;
        }
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if is_tm_inprogress_dir_name(&name) {
            out.push(path);
            continue;
        }
        if depth < max_depth {
            walk_inprogress(&path, depth + 1, max_depth, out);
        }
    }
}

fn candidate_ok(deps: &dyn TmDeps, path: &Path, now: SystemTime) -> bool {
    if !(path_allowed_for_tm_delete(path) || path_under_tm_bundle_mount(deps, path)) {
        return false;
    }
    let Some(mtime) = deps.path_mtime(path) else {
        return false;
    };
    let Some(hours) = hours_old(mtime, now) else {
        return false;
    };
    if hours < TM_BACKUP_SAFE_HOURS {
        return false;
    }
    deps.dir_size_bytes(path) > 0
}

/// 选入陈旧 inProgress；busy 时 paths 空且 skipped_busy=true。
pub fn select_tm_failed_backups(deps: &dyn TmDeps, now: SystemTime) -> TmSelectResult {
    if !deps.tmutil_exists()
        || !deps.auto_backup_configured()
        || !deps.destination_configured()
        || !deps.volumes_root().is_dir()
    {
        return TmSelectResult::default();
    }
    match deps.running_state() {
        TmRunningState::Idle => {}
        TmRunningState::Running | TmRunningState::Unknown => {
            return TmSelectResult {
                paths: Vec::new(),
                skipped_busy: true,
            };
        }
    }

    let volumes_root = deps.volumes_root();
    let Ok(vols) = fs::read_dir(&volumes_root) else {
        return TmSelectResult::default();
    };

    let mut out = Vec::new();
    for ent in vols.flatten() {
        let vol = ent.path();
        let Ok(meta) = fs::symlink_metadata(&vol) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name == "MacintoshHD" {
            continue;
        }
        let backupdb = vol.join("Backups.backupdb");
        let mobile = vol.join(".MobileBackups");
        let has_bundle = fs::read_dir(&vol).ok().is_some_and(|rd| {
            rd.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.ends_with(".backupbundle") || n.ends_with(".sparsebundle")
            })
        });
        if !backupdb.is_dir() && !mobile.is_dir() && !has_bundle {
            continue;
        }
        if is_network_or_unknown_fs(&deps.fs_type(&vol)) {
            continue;
        }
        if backupdb.is_dir() {
            let mut found = Vec::new();
            walk_inprogress(&backupdb, 1, 3, &mut found);
            for p in found {
                if candidate_ok(deps, &p, now) {
                    out.push(p);
                }
            }
        }
        if let Ok(rd) = fs::read_dir(&vol) {
            for be in rd.flatten() {
                let bundle = be.path();
                let bn = be.file_name().to_string_lossy().into_owned();
                if !(bn.ends_with(".backupbundle") || bn.ends_with(".sparsebundle")) {
                    continue;
                }
                let Ok(bm) = fs::symlink_metadata(&bundle) else {
                    continue;
                };
                if !bm.is_dir() {
                    continue;
                }
                let Some(mount) = deps.bundle_mount_point(&bundle) else {
                    continue;
                };
                let mut found = Vec::new();
                walk_inprogress(&mount, 1, 3, &mut found);
                for p in found {
                    if candidate_ok(deps, &p, now) {
                        out.push(p);
                    }
                }
            }
        }
    }

    TmSelectResult {
        paths: out,
        skipped_busy: false,
    }
}

/// 生产计划候选。
pub fn tm_failed_backups_plan_candidates() -> Vec<PathBuf> {
    select_tm_failed_backups(&LiveTmDeps, SystemTime::now()).paths
}

/// 生产路径。
pub struct LiveTmDeps;

/// 仅 `cfg(test)` / debug 构建可读的测试捷径；release 永远 false。
fn test_tm_force_idle() -> bool {
    #[cfg(any(test, debug_assertions))]
    {
        std::env::var_os("VOLE_TEST_TM_FORCE_IDLE").is_some()
    }
    #[cfg(not(any(test, debug_assertions)))]
    {
        false
    }
}

fn test_volumes_root_override() -> Option<PathBuf> {
    #[cfg(any(test, debug_assertions))]
    {
        std::env::var_os("VOLE_TEST_VOLUMES").map(PathBuf::from)
    }
    #[cfg(not(any(test, debug_assertions)))]
    {
        None
    }
}

impl TmDeps for LiveTmDeps {
    fn tmutil_exists(&self) -> bool {
        if test_tm_force_idle() {
            return true;
        }
        Command::new("tmutil")
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn auto_backup_configured(&self) -> bool {
        if test_tm_force_idle() {
            return true;
        }
        let output = Command::new("defaults")
            .args([
                "read",
                "/Library/Preferences/com.apple.TimeMachine",
                "AutoBackup",
            ])
            .output();
        let Ok(output) = output else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        let s = String::from_utf8_lossy(&output.stdout);
        let t = s.trim();
        t == "0" || t == "1"
    }

    fn destination_configured(&self) -> bool {
        if test_tm_force_idle() {
            return true;
        }
        let output = Command::new("tmutil").arg("destinationinfo").output();
        let Ok(output) = output else {
            return false;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let err = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{text}{err}");
        if combined.contains("No destinations configured") {
            return false;
        }
        output.status.success()
    }

    fn running_state(&self) -> TmRunningState {
        if test_tm_force_idle() {
            return TmRunningState::Idle;
        }
        let output = Command::new("tmutil").arg("status").output();
        let Ok(output) = output else {
            return TmRunningState::Unknown;
        };
        if !output.status.success() {
            return TmRunningState::Unknown;
        }
        let st = String::from_utf8_lossy(&output.stdout);
        if !st.contains("Running") {
            return TmRunningState::Unknown;
        }
        for line in st.lines() {
            let t = line.trim();
            if t.contains("Running") && t.contains('=') {
                if t.contains("= 1") || t.contains("=1") {
                    return TmRunningState::Running;
                }
                if t.contains("= 0") || t.contains("=0") {
                    return TmRunningState::Idle;
                }
            }
        }
        TmRunningState::Unknown
    }

    fn volumes_root(&self) -> PathBuf {
        if let Some(p) = test_volumes_root_override() {
            return p;
        }
        PathBuf::from("/Volumes")
    }

    fn fs_type(&self, vol: &Path) -> String {
        if test_tm_force_idle() {
            return "apfs".into();
        }
        let output = Command::new("df").arg("-T").arg(vol).output();
        let Ok(output) = output else {
            return "unknown".into();
        };
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .nth(1)
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("unknown")
            .to_string()
    }

    fn bundle_mount_point(&self, bundle: &Path) -> Option<PathBuf> {
        let name = bundle.file_name()?.to_str()?;
        let output = Command::new("hdiutil").arg("info").output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut saw = false;
        for line in text.lines() {
            if line.contains("image-path") && line.contains(name) {
                saw = true;
                continue;
            }
            if saw && line.contains("/Volumes/") {
                for tok in line.split_whitespace() {
                    if tok.starts_with("/Volumes/") {
                        return Some(PathBuf::from(tok));
                    }
                }
            }
            if saw && line.starts_with('=') {
                break;
            }
        }
        None
    }

    fn path_mtime(&self, path: &Path) -> Option<SystemTime> {
        fs::symlink_metadata(path).ok()?.modified().ok()
    }

    fn dir_size_bytes(&self, path: &Path) -> u64 {
        let Ok(rd) = fs::read_dir(path) else {
            return 0;
        };
        if rd.flatten().next().is_some() {
            1
        } else {
            0
        }
    }

    fn delete_backup(&self, path: &Path) -> Result<(), String> {
        let status = Command::new("tmutil")
            .arg("delete")
            .arg(path)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("tmutil delete exit {status}"))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

    pub struct FakeTmDeps {
        pub tmutil: bool,
        pub auto: bool,
        pub dest: bool,
        pub running: TmRunningState,
        pub volumes: PathBuf,
        pub fs: String,
        pub deleted: Mutex<Vec<PathBuf>>,
        pub mounts: Vec<(PathBuf, PathBuf)>,
        pub size_override: Option<u64>,
    }

    pub fn happy(volumes: PathBuf) -> FakeTmDeps {
        FakeTmDeps {
            tmutil: true,
            auto: true,
            dest: true,
            running: TmRunningState::Idle,
            volumes,
            fs: "apfs".into(),
            deleted: Mutex::new(Vec::new()),
            mounts: Vec::new(),
            size_override: Some(1024),
        }
    }

    impl TmDeps for FakeTmDeps {
        fn tmutil_exists(&self) -> bool {
            self.tmutil
        }
        fn auto_backup_configured(&self) -> bool {
            self.auto
        }
        fn destination_configured(&self) -> bool {
            self.dest
        }
        fn running_state(&self) -> TmRunningState {
            self.running
        }
        fn volumes_root(&self) -> PathBuf {
            self.volumes.clone()
        }
        fn fs_type(&self, _vol: &Path) -> String {
            self.fs.clone()
        }
        fn bundle_mount_point(&self, bundle: &Path) -> Option<PathBuf> {
            self.mounts
                .iter()
                .find(|(b, _)| b == bundle)
                .map(|(_, m)| m.clone())
        }
        fn path_mtime(&self, path: &Path) -> Option<SystemTime> {
            fs::symlink_metadata(path).ok()?.modified().ok()
        }
        fn dir_size_bytes(&self, _path: &Path) -> u64 {
            self.size_override.unwrap_or(0)
        }
        fn delete_backup(&self, path: &Path) -> Result<(), String> {
            self.deleted.lock().unwrap().push(path.to_path_buf());
            let _ = fs::remove_dir_all(path);
            Ok(())
        }
    }

    #[test]
    fn gates_block_when_running() {
        let dir = tempfile::tempdir().unwrap();
        let mut deps = happy(dir.path().to_path_buf());
        deps.running = TmRunningState::Running;
        assert!(!gates_allow_scan(&deps));
    }

    #[test]
    fn gates_block_when_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let mut deps = happy(dir.path().to_path_buf());
        deps.running = TmRunningState::Unknown;
        assert!(!gates_allow_scan(&deps));
    }

    #[test]
    fn gates_allow_when_idle_and_configured() {
        let dir = tempfile::tempdir().unwrap();
        assert!(gates_allow_scan(&happy(dir.path().to_path_buf())));
    }

    #[test]
    fn inprogress_name_matches() {
        assert!(is_tm_inprogress_dir_name("2024-01-01-120000.inProgress"));
        assert!(is_tm_inprogress_dir_name("x.inprogress"));
        assert!(!is_tm_inprogress_dir_name("2024-01-01-120000"));
    }

    #[test]
    fn allowlist_rejects_non_backupdb_shape() {
        assert!(!path_allowed_for_tm_delete(Path::new(
            "/tmp/foo/bar/x.inProgress"
        )));
        assert!(!path_allowed_for_tm_delete(Path::new(
            "/Volumes/V/not-backupdb/Host/x.inProgress"
        )));
        assert!(path_allowed_for_tm_delete(Path::new(
            "/Volumes/V/Backups.backupdb/Host/x.inProgress"
        )));
    }

    #[test]
    fn select_finds_stale_backupdb_inprogress() {
        let root = tempfile::tempdir().unwrap();
        let vol = root.path().join("BackupVol");
        let ip = vol.join("Backups.backupdb/Host/2024.inProgress");
        fs::create_dir_all(&ip).unwrap();
        fs::write(ip.join("x"), b"x").unwrap();
        let ancient = SystemTime::now() - Duration::from_secs(49 * 3600);
        filetime::set_file_mtime(&ip, filetime::FileTime::from_system_time(ancient)).unwrap();
        let deps = happy(root.path().to_path_buf());
        let res = select_tm_failed_backups(&deps, SystemTime::now());
        assert_eq!(res.paths, vec![ip]);
        assert!(!res.skipped_busy);
    }

    #[test]
    fn select_skips_younger_than_48h() {
        let root = tempfile::tempdir().unwrap();
        let vol = root.path().join("BackupVol");
        let ip = vol.join("Backups.backupdb/Host/young.inProgress");
        fs::create_dir_all(&ip).unwrap();
        fs::write(ip.join("x"), b"x").unwrap();
        let deps = happy(root.path().to_path_buf());
        let res = select_tm_failed_backups(&deps, SystemTime::now());
        assert!(res.paths.is_empty());
    }

    #[test]
    fn select_skips_network_fs() {
        let root = tempfile::tempdir().unwrap();
        let vol = root.path().join("BackupVol");
        let ip = vol.join("Backups.backupdb/Host/old.inProgress");
        fs::create_dir_all(&ip).unwrap();
        let ancient = SystemTime::now() - Duration::from_secs(49 * 3600);
        filetime::set_file_mtime(&ip, filetime::FileTime::from_system_time(ancient)).unwrap();
        let mut deps = happy(root.path().to_path_buf());
        deps.fs = "nfs".into();
        let res = select_tm_failed_backups(&deps, SystemTime::now());
        assert!(res.paths.is_empty());
    }

    #[test]
    fn select_skipped_busy_when_running() {
        let root = tempfile::tempdir().unwrap();
        let mut deps = happy(root.path().to_path_buf());
        deps.running = TmRunningState::Running;
        let res = select_tm_failed_backups(&deps, SystemTime::now());
        assert!(res.skipped_busy);
        assert!(res.paths.is_empty());
    }
}
