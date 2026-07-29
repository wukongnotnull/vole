//! `validate_path_for_deletion`（对齐 mole `lib/core/file_ops.sh`）。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::critical::{
    is_coresymbolicationd_cache, is_critical_deletion_path, is_private_allowlisted,
    normalize_policy_path,
};
use super::endpoint::is_endpoint_security_cache_path;

/// 应用层保护判定（Phase 4a Task 3 完整实现）。
pub trait PathProtection {
    fn should_protect(&self, policy_path: &str) -> bool;
}

/// 尚未加载保护清单时的占位实现。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPathProtection;

impl PathProtection for NoPathProtection {
    fn should_protect(&self, _policy_path: &str) -> bool {
        false
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("empty path")]
    Empty,
    #[error("path must be absolute")]
    NotAbsolute,
    #[error("path traversal not allowed")]
    Traversal,
    #[error("contains control characters")]
    ControlChar,
    #[error("cannot read symlink")]
    UnreadableSymlink,
    #[error("symlink points to protected system path")]
    SymlinkToCritical,
    #[error("resolves into a critical system path")]
    AncestorResolvesToCritical,
    #[error("endpoint-security agent cache")]
    EndpointSecurityCache,
    #[error("critical system path")]
    CriticalSystemPath,
    #[error("protected path")]
    ProtectedPath,
}

/// 校验路径是否允许进入删除管线。
pub fn validate_path_for_deletion(
    path: &str,
    protection: &dyn PathProtection,
) -> Result<(), ValidationError> {
    if path.is_empty() {
        return Err(ValidationError::Empty);
    }
    if !path.starts_with('/') {
        return Err(ValidationError::NotAbsolute);
    }
    if has_traversal_component(path) {
        return Err(ValidationError::Traversal);
    }
    if path.bytes().any(|b| b.is_ascii_control()) {
        return Err(ValidationError::ControlChar);
    }

    let policy_path = normalize_policy_path(path);

    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            if let Some(resolved) = resolve_symlink_target(path).map_err(|_| ValidationError::UnreadableSymlink)? {
                let resolved = normalize_policy_path(&resolved);
                if is_critical_deletion_path(&resolved) {
                    return Err(ValidationError::SymlinkToCritical);
                }
            }
        }
    }

    if ancestor_symlink_redirects_to_critical(path, protection) {
        return Err(ValidationError::AncestorResolvesToCritical);
    }

    if is_coresymbolicationd_cache(&policy_path) {
        return Ok(());
    }

    if is_endpoint_security_cache_path(&policy_path) {
        return Err(ValidationError::EndpointSecurityCache);
    }

    if is_private_allowlisted(&policy_path) {
        return Ok(());
    }

    if is_critical_deletion_path(&policy_path) {
        return Err(ValidationError::CriticalSystemPath);
    }

    if protection.should_protect(&policy_path) {
        return Err(ValidationError::ProtectedPath);
    }

    Ok(())
}

fn has_traversal_component(path: &str) -> bool {
    path.split('/').any(|part| part == "..")
}

fn resolve_symlink_target(path: &str) -> io::Result<Option<String>> {
    let link_target = fs::read_link(path)?;
    let resolved = if link_target.is_absolute() {
        link_target
    } else {
        let parent = Path::new(path).parent().unwrap_or(Path::new("/"));
        parent.join(link_target)
    };
    Ok(Some(resolved.to_string_lossy().into_owned()))
}

fn ancestor_symlink_redirects_to_critical(path: &str, protection: &dyn PathProtection) -> bool {
    let policy_path = normalize_policy_path(path);
    let parent_dir = Path::new(&policy_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));

    if !parent_dir.is_dir() {
        return false;
    }

    if !ancestor_has_symlink(&parent_dir) {
        return false;
    }

    let resolved_parent = fs::canonicalize(&parent_dir).unwrap_or(parent_dir.clone());
    let leaf = Path::new(&policy_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let resolved_path = normalize_policy_path(&format!(
        "{}/{}",
        resolved_parent.to_string_lossy(),
        leaf
    ));

    if resolved_parent != parent_dir && is_critical_deletion_path(&resolved_path) {
        return true;
    }
    resolved_parent != parent_dir && protection.should_protect(&resolved_path)
}

fn ancestor_has_symlink(dir: &Path) -> bool {
    let mut probe = dir.to_path_buf();
    while probe.components().count() > 0 {
        if fs::symlink_metadata(&probe)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
        if probe == Path::new("/") {
            break;
        }
        probe = probe.parent().unwrap_or(Path::new("/")).to_path_buf();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct FakeProtection {
        prefix: String,
    }

    impl PathProtection for FakeProtection {
        fn should_protect(&self, policy_path: &str) -> bool {
            policy_path.contains(&self.prefix)
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vole-validate-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_empty_and_relative() {
        let p = NoPathProtection;
        assert_eq!(
            validate_path_for_deletion("", &p),
            Err(ValidationError::Empty)
        );
        assert_eq!(
            validate_path_for_deletion("relative/path", &p),
            Err(ValidationError::NotAbsolute)
        );
    }

    #[test]
    fn rejects_traversal_but_allows_firefox_files() {
        let p = NoPathProtection;
        assert_eq!(
            validate_path_for_deletion("/tmp/../etc", &p),
            Err(ValidationError::Traversal)
        );
        let dir = scratch("firefox");
        let ff = dir.join("2753419432nreetyfallipx..files");
        fs::create_dir_all(&ff).unwrap();
        assert!(validate_path_for_deletion(&ff.to_string_lossy(), &p).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_critical_system_paths() {
        let p = NoPathProtection;
        assert_eq!(
            validate_path_for_deletion("/System", &p),
            Err(ValidationError::CriticalSystemPath)
        );
        assert_eq!(
            validate_path_for_deletion("/Applications/Finder.app", &p),
            Err(ValidationError::CriticalSystemPath)
        );
    }

    #[test]
    fn allows_private_tmp_children() {
        let p = NoPathProtection;
        assert_eq!(
            validate_path_for_deletion("/private/tmp", &p),
            Err(ValidationError::CriticalSystemPath)
        );
        assert!(validate_path_for_deletion("/private/tmp/mole-old-artifact", &p).is_ok());
    }

    #[test]
    fn rejects_ancestor_symlink_into_protected_user_data() {
        let dir = scratch("ancestor-user");
        let protected_home = dir.join("home");
        fs::create_dir_all(protected_home.join("Library/Keychains")).unwrap();
        let fake_cache = dir.join("cache-root");
        symlink(protected_home.join("Library"), &fake_cache).unwrap();
        let target = fake_cache.join("Keychains/login.keychain-db");
        fs::write(&target, b"x").unwrap();
        let protection = FakeProtection {
            prefix: "Keychains".into(),
        };
        assert_eq!(
            validate_path_for_deletion(&target.to_string_lossy(), &protection),
            Err(ValidationError::AncestorResolvesToCritical)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn accepts_ordinary_cache_file() {
        let p = NoPathProtection;
        let dir = scratch("ordinary");
        let cache = dir.join("real/Caches/cache.db");
        fs::create_dir_all(cache.parent().unwrap()).unwrap();
        fs::write(&cache, b"x").unwrap();
        assert!(validate_path_for_deletion(&cache.to_string_lossy(), &p).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_endpoint_security_cache() {
        let p = NoPathProtection;
        assert_eq!(
            validate_path_for_deletion(
                "/private/var/folders/9d/abc/C/com.crowdstrike.falcon.App/com.apple.metalfe",
                &p
            ),
            Err(ValidationError::EndpointSecurityCache)
        );
    }
}
