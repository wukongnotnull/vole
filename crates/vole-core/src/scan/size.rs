//! 文件占用与硬链接去重（对齐 mole `getActualFileSize` / `countableFileSize`）。

use std::collections::HashSet;
use std::fs::Metadata;
use std::os::darwin::fs::MetadataExt;
use std::path::Path;

pub fn actual_file_size(meta: &Metadata) -> u64 {
    let blocks = meta.st_blocks() * 512;
    let logical = meta.len();
    if blocks < logical {
        blocks
    } else {
        logical
    }
}

/// 硬链接去重：同一 `(dev, ino)` 只计一次体积。
pub fn countable_file_size(meta: &Metadata, seen: &mut HashSet<(u64, u64)>) -> u64 {
    let size = actual_file_size(meta);
    if meta.st_nlink() > 1 {
        let key = (meta.st_dev(), meta.st_ino());
        if seen.contains(&key) {
            return 0;
        }
        seen.insert(key);
    }
    size
}

pub fn last_access_time(meta: &Metadata) -> Option<std::time::SystemTime> {
    let secs = meta.st_atime();
    let nsec = meta.st_atime_nsec();
    if secs == 0 && nsec == 0 {
        None
    } else {
        Some(std::time::UNIX_EPOCH + std::time::Duration::new(secs as u64, nsec as u32))
    }
}

/// 大文件榜跳过的小文件扩展名（对齐 mole `skipExtensions` 子集）。
pub fn skip_large_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "go" | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "json"
            | "md"
            | "txt"
            | "yml"
            | "yaml"
            | "xml"
            | "html"
            | "css"
            | "py"
            | "rb"
            | "java"
            | "rs"
            | "swift"
            | "c"
            | "cpp"
            | "h"
    )
}
