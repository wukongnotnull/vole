//! Fail-closed 二进制存在性探测与 package-managed 路径判定。

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// 无 sudo 下对 Program 路径的存在性结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryPresence {
    /// 每级祖先均可进入，且终点明确 `ENOENT`。
    Missing,
    /// 文件存在，或权限不足无法确认（视为可能仍存在）。
    PresentOrUnknowable,
}

/// Spec §4.3：仅「祖先可进入 + 终点 ENOENT」才算缺失。
pub fn probe_binary_presence(path: &Path) -> BinaryPresence {
    if path.as_os_str().is_empty() {
        return BinaryPresence::PresentOrUnknowable;
    }

    let mut cumulative = PathBuf::new();
    let components: Vec<_> = path.components().collect();
    if components.is_empty() {
        return BinaryPresence::PresentOrUnknowable;
    }

    for (idx, component) in components.iter().enumerate() {
        match component {
            Component::RootDir => {
                cumulative.push("/");
                continue;
            }
            Component::CurDir | Component::ParentDir => {
                return BinaryPresence::PresentOrUnknowable;
            }
            Component::Prefix(_) => {
                return BinaryPresence::PresentOrUnknowable;
            }
            Component::Normal(_) => {
                cumulative.push(component.as_os_str());
            }
        }

        let is_final = idx + 1 == components.len();
        if is_final {
            return match std::fs::symlink_metadata(&cumulative) {
                Ok(_) => BinaryPresence::PresentOrUnknowable,
                Err(err) if err.kind() == ErrorKind::NotFound => BinaryPresence::Missing,
                Err(_) => BinaryPresence::PresentOrUnknowable,
            };
        }

        match std::fs::metadata(&cumulative) {
            Ok(meta) if meta.is_dir() => {
                if std::fs::read_dir(&cumulative).is_err() {
                    return BinaryPresence::PresentOrUnknowable;
                }
            }
            Ok(_) => {
                // 祖先是文件却还有后续组件 —— 终点不可能存在。
                return BinaryPresence::Missing;
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return BinaryPresence::Missing;
            }
            Err(_) => return BinaryPresence::PresentOrUnknowable,
        }
    }

    BinaryPresence::PresentOrUnknowable
}

/// Mole `_is_package_managed_binary` 同形。
pub fn is_package_managed_binary(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.starts_with("/usr/local/bin/")
        || s.starts_with("/usr/local/sbin/")
        || s.starts_with("/opt/homebrew/bin/")
        || s.starts_with("/opt/homebrew/sbin/")
        || is_homebrew_opt_bin(&s)
        || s.starts_with("/usr/bin/")
        || s.starts_with("/usr/sbin/")
        || s.starts_with("/usr/libexec/")
        || s.starts_with("/bin/")
        || s.starts_with("/sbin/")
}

fn is_homebrew_opt_bin(s: &str) -> bool {
    // /opt/homebrew/opt/<pkg>/bin/* or .../sbin/*
    let Some(rest) = s.strip_prefix("/opt/homebrew/opt/") else {
        return false;
    };
    let mut parts = rest.split('/');
    let Some(_pkg) = parts.next() else {
        return false;
    };
    matches!(parts.next(), Some("bin") | Some("sbin")) && parts.next().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("vole-sysorphan-probe-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn probe_missing_when_ancestors_enterable_and_enoent() {
        let root = scratch("missing");
        let target = root.join("bin").join("gone");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        assert_eq!(probe_binary_presence(&target), BinaryPresence::Missing);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn probe_present_when_file_exists() {
        let root = scratch("exists");
        let target = root.join("helper");
        fs::write(&target, b"x").unwrap();
        assert_eq!(
            probe_binary_presence(&target),
            BinaryPresence::PresentOrUnknowable
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn probe_unknowable_when_ancestor_not_enterable() {
        let root = scratch("denied");
        let secret = root.join("secret");
        fs::create_dir_all(&secret).unwrap();
        let target = secret.join("helper");
        // File may or may not exist under denied dir; we block the dir.
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o000)).unwrap();
        let result = probe_binary_presence(&target);
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o700)).unwrap();
        let _ = fs::remove_dir_all(&root);
        assert_eq!(result, BinaryPresence::PresentOrUnknowable);
    }

    #[test]
    fn package_managed_prefixes() {
        assert!(is_package_managed_binary(Path::new("/opt/homebrew/bin/x")));
        assert!(is_package_managed_binary(Path::new("/usr/libexec/foo")));
        assert!(is_package_managed_binary(Path::new(
            "/opt/homebrew/opt/pkg/bin/x"
        )));
        assert!(is_package_managed_binary(Path::new("/usr/local/sbin/y")));
        assert!(is_package_managed_binary(Path::new("/bin/ls")));
        assert!(is_package_managed_binary(Path::new("/sbin/launchd")));
        assert!(!is_package_managed_binary(Path::new(
            "/Library/PrivilegedHelperTools/x"
        )));
        assert!(!is_package_managed_binary(Path::new(
            "/Applications/A.app/Contents/MacOS/A"
        )));
    }
}
