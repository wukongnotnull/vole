//! 删除前体积测量（对齐 mole `get_path_size_kb`）。

use std::fs;
use std::path::Path;

use crate::scan::du_directory_size;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSizeKb {
    Known(u64),
    Unknown,
}

pub fn measure_path_size_kb(path: &str) -> PathSizeKb {
    let path = Path::new(path);
    if !path.exists() && path.symlink_metadata().is_err() {
        return PathSizeKb::Unknown;
    }

    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return PathSizeKb::Unknown,
    };

    if meta.is_file() || meta.file_type().is_symlink() {
        return PathSizeKb::Known(meta.len().div_ceil(1024));
    }

    if meta.is_dir() {
        return match du_directory_size(path) {
            Ok(bytes) => PathSizeKb::Known(bytes.div_ceil(1024)),
            Err(_) => PathSizeKb::Unknown,
        };
    }

    PathSizeKb::Unknown
}

pub fn size_kb_field(size: PathSizeKb) -> String {
    match size {
        PathSizeKb::Known(kb) => kb.to_string(),
        PathSizeKb::Unknown => "unknown".into(),
    }
}

/// 删除前实测字节数（文件用 metadata，目录用 du）；失败返回 `None`。
pub fn measure_path_size_bytes(path: &str) -> Option<u64> {
    let path = Path::new(path);
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.is_file() || meta.file_type().is_symlink() {
        return Some(meta.len());
    }
    if meta.is_dir() {
        return du_directory_size(path).ok();
    }
    None
}
