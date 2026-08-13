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
    crate::user_paths::deletions_log_write_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;

    #[test]
    fn deletion_log_write_path_defaults_to_vole() {
        let _guard = test_env::lock();
        std::env::remove_var("VOLE_DELETE_LOG");
        std::env::remove_var("MOLE_DELETE_LOG");
        std::env::set_var("HOME", "/Users/demo");
        let path = deletion_log_path();
        assert!(
            path.ends_with("Library/Logs/vole/deletions.log"),
            "{}",
            path.display()
        );
        std::env::remove_var("HOME");
    }
}
