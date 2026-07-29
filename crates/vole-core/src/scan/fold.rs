//! 折叠目录名与跳过目录（对齐 mole `foldDirs` / `defaultSkipDirs`）。

use std::path::Path;

pub fn should_fold_name(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".svn"
            | ".hg"
            | "node_modules"
            | ".npm"
            | "__pycache__"
            | ".venv"
            | "venv"
            | "target"
            | ".gradle"
            | ".m2"
            | ".cargo"
            | "build"
            | "dist"
            | ".cache"
            | "Pods"
            | "DerivedData"
            | ".next"
            | ".yarn"
            | ".pnpm-store"
            | "vendor"
            | "Caches"
            | ".Trash"
    )
}

/// 任意扫描根下跳过的目录（对齐 mole `defaultSkipDirs`）。
pub fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        "nfs"
            | "PHD"
            | "Permissions"
            | "OrbStack"
            | "Colima"
            | "Parallels"
            | "VMware Fusion"
            | "VirtualBox VMs"
            | "Rancher Desktop"
            | ".lima"
            | ".colima"
            | ".orbstack"
    )
}

pub fn should_skip_root_child(root: &Path, name: &str) -> bool {
    if should_skip_dir(name) {
        return true;
    }
    if root == Path::new("/") {
        matches!(
            name,
            "dev"
                | "tmp"
                | "private"
                | "cores"
                | "net"
                | "home"
                | "System"
                | "sbin"
                | "bin"
                | "etc"
                | "var"
                | "Volumes"
                | "Network"
                | ".vol"
        )
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_orbstack_anywhere() {
        assert!(should_skip_dir("OrbStack"));
        assert!(should_skip_root_child(Path::new("/Users/me"), "OrbStack"));
    }
}
