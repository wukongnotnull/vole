//! 安装来源判定（M9 update / M10 remove 共享）。

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOrigin {
    Homebrew,
    Manual,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallLayout {
    pub origin: InstallOrigin,
    pub binary_path: PathBuf,
    pub prefix_bin: PathBuf,
    pub config_dir: PathBuf,
}

pub fn default_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("VOLE_CONFIG_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs_config_fallback()
}

fn dirs_config_fallback() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config/vole");
    }
    PathBuf::from(".config/vole")
}

/// 根据调用中的二进制路径判定安装形态（不跟 PATH 上的其它副本）。
pub fn detect_install_layout(binary_path: &Path, config_dir: Option<&Path>) -> InstallLayout {
    let config = config_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(default_config_dir);

    if binary_path.as_os_str().is_empty() {
        return InstallLayout {
            origin: InstallOrigin::Unknown,
            binary_path: binary_path.to_path_buf(),
            prefix_bin: PathBuf::new(),
            config_dir: config,
        };
    }

    let canonical =
        std::fs::canonicalize(binary_path).unwrap_or_else(|_| binary_path.to_path_buf());
    let prefix_bin = canonical
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();

    let origin = if is_homebrew_vole_path(binary_path, &canonical) {
        InstallOrigin::Homebrew
    } else {
        InstallOrigin::Manual
    };

    InstallLayout {
        origin,
        binary_path: canonical,
        prefix_bin,
        config_dir: config,
    }
}

fn is_homebrew_vole_path(original: &Path, canonical: &Path) -> bool {
    let canon_s = canonical.to_string_lossy();
    if canon_s.contains("/Cellar/vole/") {
        return true;
    }

    // Symlink target may already be canonicalized above; also probe readlink text.
    if original
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        if let Ok(target) = std::fs::read_link(original) {
            let t = target.to_string_lossy();
            if t.contains("Cellar/vole") {
                return true;
            }
        }
    }

    let orig_s = original.to_string_lossy();
    for prefix in ["/opt/homebrew", "/usr/local"] {
        let bin = format!("{prefix}/bin/vole");
        if orig_s == bin || canon_s == bin {
            let cellar = PathBuf::from(format!("{prefix}/Cellar/vole"));
            if cellar.is_dir() {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_homebrew_cellar_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let cellar = dir.path().join("Cellar/vole/2.3.0/bin");
        fs::create_dir_all(&cellar).unwrap();
        let real = cellar.join("vole");
        fs::write(&real, b"x").unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let link = bin.join("vole");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let layout = detect_install_layout(&link, Some(dir.path()));
        assert_eq!(layout.origin, InstallOrigin::Homebrew);
        assert_eq!(layout.prefix_bin, fs::canonicalize(&cellar).unwrap());
    }

    #[test]
    fn detects_manual_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("vole");
        fs::write(&exe, b"x").unwrap();
        let layout = detect_install_layout(&exe, Some(dir.path()));
        assert_eq!(layout.origin, InstallOrigin::Manual);
        assert_eq!(layout.prefix_bin, fs::canonicalize(&bin).unwrap());
    }
}
