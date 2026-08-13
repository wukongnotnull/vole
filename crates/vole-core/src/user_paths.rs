//! Vole on-disk locations under `$HOME`, with Mole-path read fallback.

use std::path::PathBuf;

fn join_home(rel: &str) -> PathBuf {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(rel))
        .unwrap_or_else(|| PathBuf::from(rel))
}

/// Prefer `primary` when it exists; otherwise `fallback` if that exists; else `primary`.
pub fn prefer_existing(primary: PathBuf, fallback: PathBuf) -> PathBuf {
    if primary.exists() {
        primary
    } else if fallback.exists() {
        fallback
    } else {
        primary
    }
}

pub fn vole_config_dir() -> PathBuf {
    join_home(".config/vole")
}

pub fn mole_config_dir() -> PathBuf {
    join_home(".config/mole")
}

pub fn vole_logs_dir() -> PathBuf {
    join_home("Library/Logs/vole")
}

pub fn mole_logs_dir() -> PathBuf {
    join_home("Library/Logs/mole")
}

pub fn whitelist_write_path() -> PathBuf {
    vole_config_dir().join("whitelist")
}

pub fn whitelist_read_path() -> PathBuf {
    prefer_existing(whitelist_write_path(), mole_config_dir().join("whitelist"))
}

pub fn optimize_whitelist_write_path() -> PathBuf {
    vole_config_dir().join("whitelist_optimize")
}

pub fn optimize_whitelist_read_path() -> PathBuf {
    prefer_existing(
        optimize_whitelist_write_path(),
        mole_config_dir().join("whitelist_optimize"),
    )
}

pub fn status_prefs_write_path() -> PathBuf {
    vole_config_dir().join("status_prefs")
}

pub fn status_prefs_read_path() -> PathBuf {
    prefer_existing(
        status_prefs_write_path(),
        mole_config_dir().join("status_prefs"),
    )
}

fn env_path(keys: &[&str]) -> Option<PathBuf> {
    for key in keys {
        if let Some(p) = std::env::var_os(key) {
            return Some(PathBuf::from(p));
        }
    }
    None
}

pub fn operations_log_write_path() -> PathBuf {
    env_path(&[
        "VOLE_OPERATIONS_LOG",
        "MOLE_OPERATIONS_LOG",
        "OPERATIONS_LOG_FILE",
    ])
    .unwrap_or_else(|| vole_logs_dir().join("operations.log"))
}

pub fn deletions_log_write_path() -> PathBuf {
    env_path(&["VOLE_DELETE_LOG", "MOLE_DELETE_LOG"])
        .unwrap_or_else(|| vole_logs_dir().join("deletions.log"))
}

pub fn operations_log_env_overridden() -> bool {
    env_path(&[
        "VOLE_OPERATIONS_LOG",
        "MOLE_OPERATIONS_LOG",
        "OPERATIONS_LOG_FILE",
    ])
    .is_some()
}

pub fn deletions_log_env_overridden() -> bool {
    env_path(&["VOLE_DELETE_LOG", "MOLE_DELETE_LOG"]).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;

    #[test]
    fn whitelist_write_path_is_under_vole_config() {
        let _guard = test_env::lock();
        std::env::set_var("HOME", "/Users/demo");
        let path = whitelist_write_path();
        assert!(
            path.ends_with(".config/vole/whitelist"),
            "{}",
            path.display()
        );
        std::env::remove_var("HOME");
    }
}
