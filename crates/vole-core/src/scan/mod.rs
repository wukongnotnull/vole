//! 目录扫描（对齐 mole `scan` / `walkDir`）。

mod fold;
mod size;

use std::collections::{BinaryHeap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use jwalk::WalkDir;

use crate::cancel::CancelToken;

pub use fold::{should_fold_name, should_skip_root_child};
pub use size::{actual_file_size, countable_file_size, last_access_time, skip_large_file};

const MAX_ENTRIES: usize = 30;
const MAX_LARGE_FILES: usize = 20;
const LARGE_FILE_MIN: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
    pub last_access: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    pub entries: Vec<DirEntry>,
    pub large_files: Vec<FileEntry>,
    pub total_size: u64,
    pub total_files: u64,
}

/// 扫描单层目录：子项按体积降序，大文件榜跨整棵树。
pub fn scan_directory(root: &Path, cancel: &CancelToken) -> std::io::Result<ScanResult> {
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("not a directory: {}", root.display()),
        ));
    }

    let seen = Arc::new(Mutex::new(HashSet::new()));
    let files_scanned = Arc::new(AtomicU64::new(0));
    let large_heap = Arc::new(Mutex::new(LargeFileHeap::new(MAX_LARGE_FILES)));

    let mut child_entries: Vec<(DirEntry, u64)> = Vec::new();
    let mut total_size = 0u64;

    for entry in fs::read_dir(root)? {
        if cancel.is_cancelled() {
            break;
        }
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if should_skip_root_child(root, &name) {
            continue;
        }
        let path = entry.path();
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.is_dir() {
            let (size, files) = walk_subtree(&path, &seen, &large_heap, cancel)?;
            files_scanned.fetch_add(files, Ordering::Relaxed);
            let last_access = fs::metadata(&path).ok().and_then(|m| last_access_time(&m));
            child_entries.push((
                DirEntry {
                    name,
                    path,
                    size,
                    is_dir: true,
                    last_access,
                },
                size,
            ));
            total_size += size;
        } else {
            let mut local_seen = seen.lock().unwrap();
            let size = countable_file_size(&meta, &mut local_seen);
            files_scanned.fetch_add(1, Ordering::Relaxed);
            total_size += size;
            maybe_push_large(&large_heap, &path, size);
            let last_access = last_access_time(&meta);
            child_entries.push((
                DirEntry {
                    name,
                    path,
                    size,
                    is_dir: false,
                    last_access,
                },
                size,
            ));
        }
    }

    child_entries.sort_by_key(|b| std::cmp::Reverse(b.1));
    let entries: Vec<DirEntry> = child_entries
        .into_iter()
        .take(MAX_ENTRIES)
        .map(|(e, _)| e)
        .collect();

    let large_files = {
        let heap = &mut *large_heap.lock().unwrap();
        heap.take_sorted()
    };
    let total_files = files_scanned.load(Ordering::Relaxed);

    Ok(ScanResult {
        entries,
        large_files,
        total_size,
        total_files,
    })
}

fn walk_subtree(
    root: &Path,
    seen: &Arc<Mutex<HashSet<(u64, u64)>>>,
    large_heap: &Arc<Mutex<LargeFileHeap>>,
    cancel: &CancelToken,
) -> std::io::Result<(u64, u64)> {
    if should_fold_name(root.file_name().and_then(|n| n.to_str()).unwrap_or("")) {
        return Ok((0, 0));
    }

    let mut local_size = 0u64;
    let mut local_files = 0u64;

    let walker = WalkDir::new(root)
        .follow_links(false)
        .skip_hidden(false)
        .process_read_dir(|_depth, _path, _parent, children| {
            children.iter_mut().for_each(|each_result| {
                if let Ok(entry) = each_result {
                    if entry.file_type.is_dir() {
                        let name = entry.file_name.to_string_lossy();
                        if should_fold_name(name.as_ref()) {
                            entry.read_children_path = None;
                        }
                    }
                }
            });
        });

    for entry in walker {
        if cancel.is_cancelled() {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if entry.file_type().is_dir() {
            continue;
        }
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mut guard = seen.lock().unwrap();
        let size = countable_file_size(&meta, &mut guard);
        local_size += size;
        local_files += 1;
        maybe_push_large(large_heap, &path, size);
    }

    Ok((local_size, local_files))
}

fn maybe_push_large(heap: &Arc<Mutex<LargeFileHeap>>, path: &Path, size: u64) {
    if size < LARGE_FILE_MIN || skip_large_file(path) {
        return;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let entry = FileEntry {
        name,
        path: path.to_path_buf(),
        size,
    };
    heap.lock().unwrap().push(entry);
}

struct LargeFileHeap {
    max: usize,
    heap: BinaryHeap<HeapFile>,
}

struct HeapFile {
    size: u64,
    entry: FileEntry,
}

impl PartialEq for HeapFile {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
    }
}

impl Eq for HeapFile {}

impl PartialOrd for HeapFile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapFile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.size.cmp(&other.size)
    }
}

impl LargeFileHeap {
    fn new(max: usize) -> Self {
        Self {
            max,
            heap: BinaryHeap::new(),
        }
    }

    fn push(&mut self, entry: FileEntry) {
        if self.heap.len() < self.max {
            self.heap.push(HeapFile {
                size: entry.size,
                entry,
            });
        } else if let Some(min) = self.heap.peek() {
            if entry.size > min.size {
                self.heap.pop();
                self.heap.push(HeapFile {
                    size: entry.size,
                    entry,
                });
            }
        }
    }

    fn take_sorted(&mut self) -> Vec<FileEntry> {
        let mut entries: Vec<FileEntry> = self.heap.drain().map(|h| h.entry).collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.size));
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    use crate::cancel::CancelToken;

    #[test]
    fn scan_small_tree() {
        let dir = std::env::temp_dir().join(format!("vole-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let sub = dir.join("sub");
        fs::create_dir(&sub).unwrap();
        let mut f = File::create(sub.join("big.bin")).unwrap();
        f.write_all(&vec![0u8; 1024]).unwrap();
        File::create(dir.join("small.txt")).unwrap();

        let cancel = CancelToken::new();
        let result = scan_directory(&dir, &cancel).unwrap();
        assert!(result.total_size >= 1024);
        assert!(result.total_files >= 2);
        assert!(!result.entries.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fold_skips_node_modules() {
        let dir = std::env::temp_dir().join(format!("vole-fold-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let nm = dir.join("node_modules");
        fs::create_dir(&nm).unwrap();
        let mut f = File::create(nm.join("pkg.js")).unwrap();
        f.write_all(&vec![0u8; 4096]).unwrap();

        let cancel = CancelToken::new();
        let result = scan_directory(&dir, &cancel).unwrap();
        assert_eq!(result.total_size, 0);
        let _ = fs::remove_dir_all(&dir);
    }
}
