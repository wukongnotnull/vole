//! `sudo -n` 与测试 Backend。

use super::{path_allowed_for_privilege, PrivilegeBackend, PrivilegeError};
use crate::delete::test_no_auth;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// 永不提权（测试默认 / probe 失败）。
pub struct NoPrivilege;

impl PrivilegeBackend for NoPrivilege {
    fn probe_noninteractive(&self) -> bool {
        false
    }

    fn remove_permanent(&self, _path: &Path) -> Result<(), PrivilegeError> {
        Err(PrivilegeError::Unavailable)
    }

    fn launchctl_unload(&self, _plist: &Path) -> Result<(), PrivilegeError> {
        Err(PrivilegeError::Unavailable)
    }
}

/// 生产：非交互 `sudo -n`。
pub struct SudoNoninteractive;

impl PrivilegeBackend for SudoNoninteractive {
    fn probe_noninteractive(&self) -> bool {
        if test_no_auth() {
            return false;
        }
        Command::new("sudo")
            .args(["-n", "true"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn remove_permanent(&self, path: &Path) -> Result<(), PrivilegeError> {
        if test_no_auth() {
            return Err(PrivilegeError::Unavailable);
        }
        if !path_allowed_for_privilege(path) {
            return Err(PrivilegeError::Refused);
        }
        let status = Command::new("sudo")
            .args(["-n", "/bin/rm", "-rf", "--"])
            .arg(path)
            .status()
            .map_err(|e| PrivilegeError::CommandFailed(e.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(PrivilegeError::CommandFailed(format!("rm exit {status}")))
        }
    }

    fn launchctl_unload(&self, plist: &Path) -> Result<(), PrivilegeError> {
        if test_no_auth() {
            return Err(PrivilegeError::Unavailable);
        }
        if !path_allowed_for_privilege(plist) {
            return Err(PrivilegeError::Refused);
        }
        let status = Command::new("sudo")
            .args(["-n", "/bin/launchctl", "unload"])
            .arg(plist)
            .status()
            .map_err(|e| PrivilegeError::CommandFailed(e.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(PrivilegeError::CommandFailed(format!(
                "launchctl unload exit {status}"
            )))
        }
    }
}

/// 测试用：记录 remove 调用，不执行真 sudo。
pub struct RecordingPrivilege {
    pub probe: bool,
    pub removed: Mutex<Vec<PathBuf>>,
    pub unloaded: Mutex<Vec<PathBuf>>,
}

impl RecordingPrivilege {
    pub fn allowing() -> Self {
        Self {
            probe: true,
            removed: Mutex::new(Vec::new()),
            unloaded: Mutex::new(Vec::new()),
        }
    }
}

impl PrivilegeBackend for RecordingPrivilege {
    fn probe_noninteractive(&self) -> bool {
        self.probe
    }

    fn remove_permanent(&self, path: &Path) -> Result<(), PrivilegeError> {
        if !self.probe {
            return Err(PrivilegeError::Unavailable);
        }
        if !path_allowed_for_privilege(path) {
            return Err(PrivilegeError::Refused);
        }
        self.removed.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    fn launchctl_unload(&self, plist: &Path) -> Result<(), PrivilegeError> {
        if !self.probe {
            return Err(PrivilegeError::Unavailable);
        }
        if !path_allowed_for_privilege(plist) {
            return Err(PrivilegeError::Refused);
        }
        self.unloaded.lock().unwrap().push(plist.to_path_buf());
        Ok(())
    }
}
