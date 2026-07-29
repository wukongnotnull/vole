//! 折叠目录名（对齐 mole `foldDirs` 子集）。

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

pub fn should_skip_root_child(root: &Path, name: &str) -> bool {
    if root == Path::new("/") {
        matches!(
            name,
            "dev" | "tmp" | "private" | "cores" | "net" | "home" | "System" | "sbin" | "bin"
                | "etc" | "var" | "Volumes" | "Network" | ".vol"
        )
    } else {
        false
    }
}
