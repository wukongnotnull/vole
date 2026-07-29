//! 删除行为配置（对齐 mole 环境变量）。

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMode {
    Permanent,
    Trash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteModeParseError {
    Invalid(String),
}

pub fn delete_mode_from_env() -> Result<DeleteMode, DeleteModeParseError> {
    match std::env::var("MOLE_DELETE_MODE") {
        Err(_) => Ok(DeleteMode::Permanent),
        Ok(v) if v == "permanent" => Ok(DeleteMode::Permanent),
        Ok(v) if v == "trash" => Ok(DeleteMode::Trash),
        Ok(v) => Err(DeleteModeParseError::Invalid(v)),
    }
}

pub fn dry_run_enabled() -> bool {
    std::env::var_os("MOLE_DRY_RUN").is_some_and(|v| v == "1")
        || std::env::var_os("VOLE_DRY_RUN").is_some_and(|v| v == "1")
}

pub fn test_no_auth() -> bool {
    std::env::var_os("MOLE_TEST_NO_AUTH").is_some_and(|v| v == "1")
        || std::env::var_os("VOLE_TEST_NO_AUTH").is_some_and(|v| v == "1")
}

pub fn test_trash_dir() -> Option<PathBuf> {
    std::env::var("MOLE_TEST_TRASH_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("VOLE_TEST_TRASH_DIR").ok().map(PathBuf::from))
}

pub fn deletion_log_path() -> PathBuf {
    std::env::var_os("MOLE_DELETE_LOG")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join("Library/Logs/mole/deletions.log"))
        })
        .unwrap_or_else(|| PathBuf::from("Library/Logs/mole/deletions.log"))
}
