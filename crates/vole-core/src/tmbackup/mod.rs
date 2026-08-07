//! Time Machine 失败中（`*.inProgress`）备份清理。
//!
//! 对齐 Mole `clean_time_machine_failed_backups`：fail-closed 门控、48h 安全窗、
//! apply 仅 `tmutil delete`（不走 sudo rm）。

use std::path::{Component, Path, PathBuf};
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

/// 粗形状：绝对路径、无 `..`、叶名为 inProgress。完整 backupdb 深度在 select/allowlist 细化。
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
    let s = path.to_string_lossy();
    s.contains("/Backups.backupdb/") || path_looks_like_bundle_inprogress(path)
}

fn path_looks_like_bundle_inprogress(path: &Path) -> bool {
    // Bundle 挂载点路径多变；至少要求绝对 inProgress 叶且深度合理（≥2 组件）
    path.components().count() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeTmDeps {
        tmutil: bool,
        auto: bool,
        dest: bool,
        running: TmRunningState,
        volumes: PathBuf,
        fs: String,
        deleted: Mutex<Vec<PathBuf>>,
    }

    fn happy(volumes: PathBuf) -> FakeTmDeps {
        FakeTmDeps {
            tmutil: true,
            auto: true,
            dest: true,
            running: TmRunningState::Idle,
            volumes,
            fs: "apfs".into(),
            deleted: Mutex::new(Vec::new()),
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
        fn bundle_mount_point(&self, _bundle: &Path) -> Option<PathBuf> {
            None
        }
        fn path_mtime(&self, path: &Path) -> Option<SystemTime> {
            std::fs::symlink_metadata(path).ok()?.modified().ok()
        }
        fn dir_size_bytes(&self, path: &Path) -> u64 {
            // 测试：有任意子文件则 >0
            std::fs::read_dir(path)
                .ok()
                .map(|rd| rd.count() as u64)
                .unwrap_or(0)
                .max(1)
        }
        fn delete_backup(&self, path: &Path) -> Result<(), String> {
            self.deleted.lock().unwrap().push(path.to_path_buf());
            let _ = std::fs::remove_dir_all(path);
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
}
