//! Mole `_remove_verified_container_stub` 同形 carve-out。
//!
//! SAFE: 刻意绕开 `mole_delete_verified` / `verify_plan_entry_for_apply`，
//! **不要**「统一」回共享删除管线：`should_protect_path` 对 `~/Library/Containers`
//! 一刀切保护（含 `com.macpaw.*` data_protected），走共享管线会让本规则永远空转。
//! 窄度由构造保证：调用方只喂硬编码 allowlist 命中的候选，本函数再重验
//! 「非 symlink 目录 + 唯一子项是 metadata plist」后 unlink + rmdir（非 `rm -r`），
//! check→remove 间长出任何内容的容器都会因 rmdir 非空失败而原样保留。

use std::path::Path;

use super::select::is_verified_stub_dir;
use super::CONTAINER_STUB_METADATA;

#[derive(Debug, PartialEq, Eq)]
pub enum StubRemoveError {
    /// 重验失败：不是（或已不再是）纯 stub，未做任何删除。
    NotAStub,
    /// metadata unlink 失败，目录保留。
    MetadataUnlink,
    /// rmdir 失败（如 TOCTOU 塞入新内容），目录保留。
    RmdirFailed,
}

/// 重验 stub 形状后 `unlink` metadata + `rmdir` 目录；任何失败都不递归、不升级。
pub fn remove_verified_container_stub(dir: &Path) -> Result<(), StubRemoveError> {
    if !is_verified_stub_dir(dir) {
        return Err(StubRemoveError::NotAStub);
    }
    let metadata = dir.join(CONTAINER_STUB_METADATA);
    std::fs::remove_file(&metadata).map_err(|_| StubRemoveError::MetadataUnlink)?;
    std::fs::remove_dir(dir).map_err(|_| StubRemoveError::RmdirFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_stub(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vole-stub-remove-{tag}-{}/com.macpaw.CleanMyMac4",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(dir.parent().unwrap());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CONTAINER_STUB_METADATA), b"plist").unwrap();
        dir
    }

    #[test]
    fn removes_pure_stub() {
        let dir = temp_stub("ok");
        assert_eq!(remove_verified_container_stub(&dir), Ok(()));
        assert!(!dir.exists());
        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn extra_content_rejected_and_preserved() {
        let dir = temp_stub("extra");
        fs::create_dir_all(dir.join("Data")).unwrap();
        assert_eq!(
            remove_verified_container_stub(&dir),
            Err(StubRemoveError::NotAStub)
        );
        assert!(dir.join(CONTAINER_STUB_METADATA).exists());
        assert!(dir.join("Data").exists());
        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn symlink_rejected() {
        let dir = temp_stub("symlink-target");
        let link = dir.parent().unwrap().join("com.macpaw.CleanMyMacX");
        std::os::unix::fs::symlink(&dir, &link).unwrap();
        assert_eq!(
            remove_verified_container_stub(&link),
            Err(StubRemoveError::NotAStub)
        );
        assert!(dir.join(CONTAINER_STUB_METADATA).exists());
        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn missing_dir_rejected() {
        let gone = std::env::temp_dir().join(format!("vole-stub-remove-gone-{}", line!()));
        let _ = fs::remove_dir_all(&gone);
        assert_eq!(
            remove_verified_container_stub(&gone),
            Err(StubRemoveError::NotAStub)
        );
    }
}
