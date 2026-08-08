//! PAM Touch ID for sudo（对照 Mole `bin/touchid.sh`）。

use crate::delete::test_no_auth;
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Mole 同款 tid 行（字节级一致）。
pub const PAM_TID_LINE: &str = "auth       sufficient     pam_tid.so";

const SUDO_LOCAL_HEADER: &str = "# sudo_local: local customizations for sudo";

#[derive(Debug, Error)]
pub enum TouchIdError {
    #[error("touchid io: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Message(String),
}

pub trait PamInstall {
    fn install_file(&self, src: &Path, dst: &Path) -> io::Result<()>;
    fn copy_file(&self, src: &Path, dst: &Path) -> io::Result<()>;
}

/// 测试 / 注入路径：直接写文件系统，不调用 sudo。
pub struct FakePamInstall;

impl PamInstall for FakePamInstall {
    fn install_file(&self, src: &Path, dst: &Path) -> io::Result<()> {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
        Ok(())
    }

    fn copy_file(&self, src: &Path, dst: &Path) -> io::Result<()> {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
        Ok(())
    }
}

/// 生产：仅 `sudo -n`；`VOLE_TEST_NO_AUTH` 下拒绝写入。
pub struct LivePamInstall;

fn live_pam_blocked() -> bool {
    if test_no_auth() {
        return true;
    }
    // 单测构建默认禁止真 sudo（防并行测试清掉 env 后挂起）。
    #[cfg(test)]
    {
        if std::env::var_os("VOLE_TEST_ALLOW_LIVE_PAM").is_none() {
            return true;
        }
    }
    false
}

impl PamInstall for LivePamInstall {
    fn install_file(&self, src: &Path, dst: &Path) -> io::Result<()> {
        if live_pam_blocked() {
            return Err(io::Error::other("VOLE_TEST_NO_AUTH: refusing pam install"));
        }
        let status = std::process::Command::new("sudo")
            .args([
                "-n",
                "install",
                "-m",
                "444",
                "-o",
                "root",
                "-g",
                "wheel",
                src.to_str().ok_or_else(|| io::Error::other("src utf8"))?,
                dst.to_str().ok_or_else(|| io::Error::other("dst utf8"))?,
            ])
            .status()?;
        if status.success() {
            let _ = fs::remove_file(src);
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "sudo -n install failed: {status}"
            )))
        }
    }

    fn copy_file(&self, src: &Path, dst: &Path) -> io::Result<()> {
        if live_pam_blocked() {
            return Err(io::Error::other("VOLE_TEST_NO_AUTH: refusing pam copy"));
        }
        let status = std::process::Command::new("sudo")
            .args([
                "-n",
                "cp",
                src.to_str().ok_or_else(|| io::Error::other("src utf8"))?,
                dst.to_str().ok_or_else(|| io::Error::other("dst utf8"))?,
            ])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!("sudo -n cp failed: {status}")))
        }
    }
}

/// 生产安装器。测试注入路径时由 CLI 改用 [`FakePamInstall`]。
pub fn pam_install_for_runtime() -> Box<dyn PamInstall> {
    Box::new(LivePamInstall)
}

/// 是否使用了测试注入的 PAM 路径（可安全 Fake 写入）。
pub fn pam_paths_injected() -> bool {
    std::env::var_os("VOLE_PAM_SUDO_FILE").is_some()
        || std::env::var_os("VOLE_PAM_SUDO_LOCAL_FILE").is_some()
}

#[derive(Debug, Clone)]
pub struct TouchIdPaths {
    pub sudo: PathBuf,
    pub sudo_local: PathBuf,
}

pub fn resolve_touchid_paths() -> TouchIdPaths {
    let sudo = std::env::var_os("VOLE_PAM_SUDO_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/pam.d/sudo"));
    let sudo_local = std::env::var_os("VOLE_PAM_SUDO_LOCAL_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            sudo.parent()
                .unwrap_or_else(|| Path::new("/etc/pam.d"))
                .join("sudo_local")
        });
    TouchIdPaths { sudo, sudo_local }
}

pub fn backup_path(sudo: &Path) -> PathBuf {
    PathBuf::from(format!("{}.vole-backup", sudo.display()))
}

pub fn is_touchid_configured(paths: &TouchIdPaths) -> bool {
    file_contains_tid(&paths.sudo_local) || file_contains_tid(&paths.sudo)
}

fn file_contains_tid(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|s| s.contains("pam_tid.so"))
        .unwrap_or(false)
}

fn sudo_mentions_sudo_local(paths: &TouchIdPaths) -> bool {
    fs::read_to_string(&paths.sudo)
        .map(|s| s.contains("sudo_local"))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchIdAction {
    Enable,
    Disable,
    None,
}

#[derive(Debug, Clone, Serialize)]
pub struct TouchIdPlan {
    pub configured: bool,
    pub uses_sudo_local: bool,
    pub action: TouchIdAction,
    pub targets: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

pub fn plan_touchid(paths: &TouchIdPaths, intent: Option<TouchIdAction>) -> TouchIdPlan {
    let configured = is_touchid_configured(paths);
    let uses_sudo_local = sudo_mentions_sudo_local(paths);
    let action = match intent {
        Some(a) => a,
        None if configured => TouchIdAction::Disable,
        None => TouchIdAction::Enable,
    };
    let mut targets = Vec::new();
    if uses_sudo_local {
        targets.push(paths.sudo_local.clone());
        if file_contains_tid(&paths.sudo) {
            targets.push(paths.sudo.clone());
        }
    } else {
        targets.push(paths.sudo.clone());
    }
    TouchIdPlan {
        configured,
        uses_sudo_local,
        action,
        targets,
        note: Some(
            "tty multi-key menu polish and bioutil hardware probe are coverage long-tail".into(),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TouchIdOutcome {
    AlreadyEnabled,
    AlreadyDisabled,
    Enabled,
    Disabled,
    DryRun,
    SkippedNoAuth,
    Failed(String),
}

/// CLI/runtime：非 dry-run 且 `VOLE_TEST_NO_AUTH` 时禁止写入（防真授权挂起）。
pub fn touchid_auth_blocked() -> bool {
    test_no_auth()
}

pub fn enable_touchid(
    paths: &TouchIdPaths,
    installer: &dyn PamInstall,
    dry_run: bool,
) -> Result<TouchIdOutcome, TouchIdError> {
    if dry_run {
        return Ok(TouchIdOutcome::DryRun);
    }
    if sudo_mentions_sudo_local(paths) {
        return enable_via_sudo_local(paths, installer);
    }
    enable_via_legacy(paths, installer)
}

pub fn disable_touchid(
    paths: &TouchIdPaths,
    installer: &dyn PamInstall,
    dry_run: bool,
) -> Result<TouchIdOutcome, TouchIdError> {
    if dry_run {
        if !is_touchid_configured(paths) {
            return Ok(TouchIdOutcome::AlreadyDisabled);
        }
        return Ok(TouchIdOutcome::DryRun);
    }
    if !is_touchid_configured(paths) {
        return Ok(TouchIdOutcome::AlreadyDisabled);
    }
    if file_contains_tid(&paths.sudo_local) {
        return disable_via_sudo_local(paths, installer);
    }
    if file_contains_tid(&paths.sudo) {
        return disable_via_legacy(paths, installer);
    }
    Err(TouchIdError::Message(
        "Could not find Touch ID configuration to disable".into(),
    ))
}

fn enable_via_sudo_local(
    paths: &TouchIdPaths,
    installer: &dyn PamInstall,
) -> Result<TouchIdOutcome, TouchIdError> {
    let legacy = file_contains_tid(&paths.sudo);
    if file_contains_tid(&paths.sudo_local) {
        if legacy {
            remove_tid_from_file(&paths.sudo, installer)?;
        }
        return Ok(TouchIdOutcome::AlreadyEnabled);
    }

    if paths.sudo_local.exists() {
        let mut body = fs::read_to_string(&paths.sudo_local)?;
        if !body.contains("pam_tid.so") {
            if !body.ends_with('\n') && !body.is_empty() {
                body.push('\n');
            }
            body.push_str(PAM_TID_LINE);
            body.push('\n');
            install_string(&paths.sudo_local, &body, installer)?;
        }
    } else {
        let body = format!("{SUDO_LOCAL_HEADER}\n{PAM_TID_LINE}\n");
        install_string(&paths.sudo_local, &body, installer)?;
    }

    if legacy {
        remove_tid_from_file(&paths.sudo, installer)?;
    }
    Ok(TouchIdOutcome::Enabled)
}

fn disable_via_sudo_local(
    paths: &TouchIdPaths,
    installer: &dyn PamInstall,
) -> Result<TouchIdOutcome, TouchIdError> {
    remove_tid_from_file(&paths.sudo_local, installer)?;
    if file_contains_tid(&paths.sudo) {
        remove_tid_from_file(&paths.sudo, installer)?;
    }
    Ok(TouchIdOutcome::Disabled)
}

fn enable_via_legacy(
    paths: &TouchIdPaths,
    installer: &dyn PamInstall,
) -> Result<TouchIdOutcome, TouchIdError> {
    if is_touchid_configured(paths) {
        return Ok(TouchIdOutcome::AlreadyEnabled);
    }
    if !paths.sudo.exists() {
        return Err(TouchIdError::Message(format!(
            "PAM sudo file missing: {}",
            paths.sudo.display()
        )));
    }

    let bak = backup_path(&paths.sudo);
    if !bak.exists() {
        installer.copy_file(&paths.sudo, &bak).map_err(|e| {
            TouchIdError::Message(format!("Failed to create backup {}: {e}", bak.display()))
        })?;
    }

    let original = fs::read_to_string(&paths.sudo)?;
    let new_body = insert_tid_after_comments(&original);
    if new_body == original {
        return Err(TouchIdError::Message(
            "Failed to modify configuration".into(),
        ));
    }

    match install_string(&paths.sudo, &new_body, installer) {
        Ok(()) => Ok(TouchIdOutcome::Enabled),
        Err(e) => {
            // 安全回滚：尝试用备份恢复
            if bak.exists() {
                let _ = installer.copy_file(&bak, &paths.sudo);
            }
            Err(TouchIdError::Message(format!(
                "Failed to enable Touch ID ({e}); backup at {}",
                bak.display()
            )))
        }
    }
}

fn disable_via_legacy(
    paths: &TouchIdPaths,
    installer: &dyn PamInstall,
) -> Result<TouchIdOutcome, TouchIdError> {
    let bak = backup_path(&paths.sudo);
    if !bak.exists() {
        installer.copy_file(&paths.sudo, &bak).map_err(|e| {
            TouchIdError::Message(format!("Failed to create backup {}: {e}", bak.display()))
        })?;
    }
    remove_tid_from_file(&paths.sudo, installer)?;
    Ok(TouchIdOutcome::Disabled)
}

fn insert_tid_after_comments(content: &str) -> String {
    let mut out = String::new();
    let mut inserted = false;
    for line in content.lines() {
        if line.starts_with('#') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if !inserted {
            out.push_str(PAM_TID_LINE);
            out.push('\n');
            inserted = true;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !inserted {
        out.push_str(PAM_TID_LINE);
        out.push('\n');
    }
    out
}

fn remove_tid_from_file(path: &Path, installer: &dyn PamInstall) -> Result<(), TouchIdError> {
    let body = fs::read_to_string(path)?;
    let filtered: String = body
        .lines()
        .filter(|l| !l.contains("pam_tid.so"))
        .fold(String::new(), |mut acc, l| {
            acc.push_str(l);
            acc.push('\n');
            acc
        });
    install_string(path, &filtered, installer)
}

fn install_string(dst: &Path, body: &str, installer: &dyn PamInstall) -> Result<(), TouchIdError> {
    let parent = dst
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let tmp = parent.join(format!(
        ".vole-touchid-{}.tmp",
        std::process::id()
    ));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }
    match installer.install_file(&tmp, dst) {
        Ok(()) => {
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(TouchIdError::Io(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn status_reads_sudo_local_first() {
        let dir = tempfile::tempdir().unwrap();
        let sudo = dir.path().join("sudo");
        let local = dir.path().join("sudo_local");
        fs::write(&sudo, "# sudo_local included\n").unwrap();
        fs::write(&local, format!("{PAM_TID_LINE}\n")).unwrap();
        let paths = TouchIdPaths {
            sudo,
            sudo_local: local,
        };
        assert!(is_touchid_configured(&paths));
    }

    #[test]
    fn plan_toggles_direction() {
        let dir = tempfile::tempdir().unwrap();
        let sudo = dir.path().join("sudo");
        let local = dir.path().join("sudo_local");
        fs::write(&sudo, "sudo_local\n").unwrap();
        let paths = TouchIdPaths {
            sudo,
            sudo_local: local,
        };
        let p = plan_touchid(&paths, None);
        assert_eq!(p.action, TouchIdAction::Enable);
        assert!(!p.configured);
    }

    #[test]
    fn enable_writes_sudo_local_when_sudo_mentions_it() {
        let dir = tempfile::tempdir().unwrap();
        let sudo = dir.path().join("sudo");
        let local = dir.path().join("sudo_local");
        fs::write(&sudo, "#\n# sudo: via sudo_local\n").unwrap();
        let paths = TouchIdPaths {
            sudo: sudo.clone(),
            sudo_local: local.clone(),
        };
        let out = enable_touchid(&paths, &FakePamInstall, false).unwrap();
        assert!(matches!(out, TouchIdOutcome::Enabled));
        let body = fs::read_to_string(&local).unwrap();
        assert!(body.contains("pam_tid.so"));
        assert!(is_touchid_configured(&paths));
    }

    #[test]
    fn disable_removes_tid_from_sudo_local() {
        let dir = tempfile::tempdir().unwrap();
        let sudo = dir.path().join("sudo");
        let local = dir.path().join("sudo_local");
        fs::write(&sudo, "sudo_local\n").unwrap();
        fs::write(&local, format!("# hdr\n{PAM_TID_LINE}\n")).unwrap();
        let paths = TouchIdPaths {
            sudo,
            sudo_local: local.clone(),
        };
        let out = disable_touchid(&paths, &FakePamInstall, false).unwrap();
        assert!(matches!(out, TouchIdOutcome::Disabled));
        assert!(!fs::read_to_string(&local).unwrap().contains("pam_tid.so"));
    }

    #[test]
    fn dry_run_enable_does_not_create_sudo_local() {
        let dir = tempfile::tempdir().unwrap();
        let sudo = dir.path().join("sudo");
        let local = dir.path().join("sudo_local");
        fs::write(&sudo, "sudo_local\n").unwrap();
        let paths = TouchIdPaths {
            sudo,
            sudo_local: local.clone(),
        };
        let out = enable_touchid(&paths, &FakePamInstall, true).unwrap();
        assert!(matches!(out, TouchIdOutcome::DryRun));
        assert!(!local.exists());
    }

    #[test]
    fn legacy_enable_creates_backup_and_inserts_tid() {
        let dir = tempfile::tempdir().unwrap();
        let sudo = dir.path().join("sudo");
        let local = dir.path().join("sudo_local");
        fs::write(&sudo, "# comment\nauth required pam_opendirectory.so\n").unwrap();
        let paths = TouchIdPaths {
            sudo: sudo.clone(),
            sudo_local: local,
        };
        enable_touchid(&paths, &FakePamInstall, false).unwrap();
        let bak = backup_path(&sudo);
        assert!(bak.exists(), "missing backup {}", bak.display());
        let body = fs::read_to_string(&sudo).unwrap();
        assert!(body.contains("pam_tid.so"));
        let non_comment = body.lines().find(|l| !l.starts_with('#')).unwrap();
        assert!(non_comment.contains("pam_tid.so"));
    }

    #[test]
    fn migrate_legacy_tid_into_sudo_local() {
        let dir = tempfile::tempdir().unwrap();
        let sudo = dir.path().join("sudo");
        let local = dir.path().join("sudo_local");
        fs::write(
            &sudo,
            format!("# via sudo_local\n{PAM_TID_LINE}\nauth required pam_opendirectory.so\n"),
        )
        .unwrap();
        let paths = TouchIdPaths {
            sudo: sudo.clone(),
            sudo_local: local.clone(),
        };
        let out = enable_touchid(&paths, &FakePamInstall, false).unwrap();
        assert!(matches!(
            out,
            TouchIdOutcome::Enabled | TouchIdOutcome::AlreadyEnabled
        ));
        assert!(fs::read_to_string(&local).unwrap().contains("pam_tid.so"));
        assert!(!fs::read_to_string(&sudo).unwrap().contains("pam_tid.so"));
    }

    struct FailOncePam {
        fails: AtomicUsize,
        inner: FakePamInstall,
    }

    impl PamInstall for FailOncePam {
        fn install_file(&self, src: &Path, dst: &Path) -> io::Result<()> {
            if self.fails.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(io::Error::other("simulated install failure"));
            }
            self.inner.install_file(src, dst)
        }

        fn copy_file(&self, src: &Path, dst: &Path) -> io::Result<()> {
            self.inner.copy_file(src, dst)
        }
    }

    #[test]
    fn legacy_enable_rolls_back_on_install_failure() {
        let dir = tempfile::tempdir().unwrap();
        let sudo = dir.path().join("sudo");
        let local = dir.path().join("sudo_local");
        let original = "# comment\nauth required pam_opendirectory.so\n";
        fs::write(&sudo, original).unwrap();
        let paths = TouchIdPaths {
            sudo: sudo.clone(),
            sudo_local: local,
        };
        let failing = FailOncePam {
            fails: AtomicUsize::new(0),
            inner: FakePamInstall,
        };
        let err = enable_touchid(&paths, &failing, false).unwrap_err();
        assert!(err.to_string().contains("backup") || err.to_string().contains("Failed"));
        // 回滚后内容应回到备份（与 original 一致）
        let after = fs::read_to_string(&sudo).unwrap();
        assert_eq!(after, original);
        assert!(!after.contains("pam_tid.so"));
    }

    #[test]
    fn live_install_refuses_under_test_no_auth() {
        let _g = env_lock();
        let prev = std::env::var_os("VOLE_TEST_NO_AUTH");
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::write(&src, "x").unwrap();
        let err = LivePamInstall.install_file(&src, &dst).unwrap_err();
        assert!(
            err.to_string().contains("VOLE_TEST_NO_AUTH"),
            "err={err}"
        );
        assert!(!dst.exists());
        match prev {
            Some(v) => std::env::set_var("VOLE_TEST_NO_AUTH", v),
            None => std::env::remove_var("VOLE_TEST_NO_AUTH"),
        }
    }

    #[test]
    fn touchid_auth_blocked_reads_env() {
        let _g = env_lock();
        let prev = std::env::var_os("VOLE_TEST_NO_AUTH");
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        assert!(touchid_auth_blocked());
        match prev {
            Some(v) => std::env::set_var("VOLE_TEST_NO_AUTH", v),
            None => std::env::remove_var("VOLE_TEST_NO_AUTH"),
        }
    }
}
