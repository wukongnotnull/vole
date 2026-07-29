//! 废纸篓移动（对齐 mole `_mole_move_to_trash`）。

use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

use vole_sys::Trash;

use super::config::{test_no_auth, test_trash_dir};

#[derive(Debug)]
pub enum TrashMoveError {
    BlockedTestMode,
    Io(io::Error),
}

pub fn move_to_trash(
    path: &Path,
    backend: &dyn Trash,
    timeout: Duration,
) -> Result<(), TrashMoveError> {
    if let Some(dir) = test_trash_dir() {
        return move_to_test_trash(path, &dir).map_err(TrashMoveError::Io);
    }

    if test_no_auth() {
        return Err(TrashMoveError::BlockedTestMode);
    }

    backend
        .trash_path(path, timeout)
        .map_err(TrashMoveError::Io)
}

fn move_to_test_trash(path: &Path, trash_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(trash_dir)?;
    let pid = std::process::id();
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "item".into());
    let dest = trash_dir.join(format!("{name}.{pid}.{stamp}"));
    fs::rename(path, dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env;
    use std::fs;
    use vole_sys::macos::MacTrash;

    #[test]
    fn test_trash_dir_moves_without_finder() {
        let _guard = test_env::lock();
        let root = std::env::temp_dir().join(format!("vole-trash-{}", std::process::id()));
        let victim = root.join("victim");
        let trash = root.join("Trash");
        fs::create_dir_all(&victim).unwrap();
        fs::write(victim.join("data.txt"), b"x").unwrap();
        std::env::set_var("MOLE_TEST_TRASH_DIR", &trash);

        move_to_trash(&victim, &MacTrash, Duration::from_secs(1)).unwrap();
        assert!(!victim.exists());
        assert!(!fs::read_dir(&trash).unwrap().next().is_none());

        std::env::remove_var("MOLE_TEST_TRASH_DIR");
        fs::remove_dir_all(&root).ok();
    }
}
