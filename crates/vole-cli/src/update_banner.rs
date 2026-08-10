//! Local update-message cache for the bare-vole home menu banner (no network).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `$VOLE_CACHE_DIR/update_message` if set, else `$HOME/.cache/vole/update_message`.
pub fn update_message_cache_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("VOLE_CACHE_DIR") {
        return PathBuf::from(dir).join("update_message");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".cache").join("vole").join("update_message")
}

pub fn read_update_message_cache() -> Option<String> {
    read_update_message_cache_at(&update_message_cache_path())
}

pub fn write_update_message_cache(msg: Option<&str>) -> io::Result<()> {
    write_update_message_cache_at(&update_message_cache_path(), msg)
}

pub fn read_update_message_cache_at(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn write_update_message_cache_at(path: &Path, msg: Option<&str>) -> io::Result<()> {
    match msg.map(str::trim).filter(|s| !s.is_empty()) {
        None => {
            if path.exists() {
                fs::remove_file(path)?;
            }
            Ok(())
        }
        Some(text) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, format!("{text}\n"))
        }
    }
}

/// Persist or clear the home-menu banner from a `vole update --check` outcome.
pub fn sync_cache_from_check(current: &str, latest: Option<&str>) -> io::Result<()> {
    match latest {
        Some(latest) if !latest.is_empty() && latest != current => {
            write_update_message_cache(Some(&format!(
                "Update {latest} available, run vole update"
            )))
        }
        _ => write_update_message_cache(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_write_update_message_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update_message");
        assert!(read_update_message_cache_at(&path).is_none());
        write_update_message_cache_at(&path, Some("Update 9.9.9 available, run vole update"))
            .unwrap();
        let msg = read_update_message_cache_at(&path).unwrap();
        assert!(msg.contains("9.9.9"));
        write_update_message_cache_at(&path, None).unwrap();
        assert!(read_update_message_cache_at(&path).is_none());
    }
}
