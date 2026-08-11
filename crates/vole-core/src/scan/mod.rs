//! 目录扫描（对齐 mole `scan` / `walkDir`）。

mod du;
mod fold;
mod size;

use std::collections::{BinaryHeap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use jwalk::WalkDir;

use crate::cancel::CancelToken;

pub use du::du_directory_size;
pub use fold::{should_fold_name, should_skip_dir, should_skip_root_child};
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
pub fn scan_directory(root: &Path, cancel: &CancelToken) -> io::Result<ScanResult> {
    scan_directory_with_progress(root, cancel, |_| {})
}

/// 同 [`scan_directory`]，每完成一个根子项调用 `on_child`（用于 TUI live 进度）。
pub fn scan_directory_with_progress<F>(
    root: &Path,
    cancel: &CancelToken,
    mut on_child: F,
) -> io::Result<ScanResult>
where
    F: FnMut(&DirEntry),
{
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("not a directory: {}", root.display()),
        ));
    }

    let seen = Arc::new(Mutex::new(HashSet::new()));
    let files_scanned = Arc::new(AtomicU64::new(0));
    let large_heap = Arc::new(Mutex::new(LargeFileHeap::new(MAX_LARGE_FILES)));

    let mut child_entries: Vec<(DirEntry, u64)> = Vec::new();
    let mut total_size = 0u64;

    for entry in fs::read_dir(root)? {
        cancel.check_scan()?;
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if should_skip_root_child(root, &name) {
            continue;
        }
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.file_type().is_symlink() {
            let size = {
                let mut guard = seen.lock().unwrap();
                countable_file_size(&meta, &mut guard)
            };
            files_scanned.fetch_add(1, Ordering::Relaxed);
            total_size += size;
            let is_dir = fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false);
            let display_name = format!("{name} →");
            let dir_entry = DirEntry {
                name: display_name,
                path,
                size,
                is_dir,
                last_access: last_access_time(&meta),
            };
            on_child(&dir_entry);
            child_entries.push((dir_entry, size));
            continue;
        }

        if meta.is_dir() {
            let (size, files) = if should_fold_name(&name) {
                (du_directory_size(&path)?, 0)
            } else {
                walk_subtree(&path, &seen, &large_heap, cancel)?
            };
            files_scanned.fetch_add(files, Ordering::Relaxed);
            let last_access = fs::metadata(&path).ok().and_then(|m| last_access_time(&m));
            let dir_entry = DirEntry {
                name,
                path,
                size,
                is_dir: true,
                last_access,
            };
            on_child(&dir_entry);
            child_entries.push((dir_entry, size));
            total_size += size;
        } else {
            let mut local_seen = seen.lock().unwrap();
            let size = countable_file_size(&meta, &mut local_seen);
            drop(local_seen);
            files_scanned.fetch_add(1, Ordering::Relaxed);
            total_size += size;
            maybe_push_large(&large_heap, &path, size);
            let last_access = last_access_time(&meta);
            let dir_entry = DirEntry {
                name,
                path,
                size,
                is_dir: false,
                last_access,
            };
            on_child(&dir_entry);
            child_entries.push((dir_entry, size));
        }
    }

    cancel.check_scan()?;

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
) -> io::Result<(u64, u64)> {
    if should_fold_name(root.file_name().and_then(|n| n.to_str()).unwrap_or("")) {
        return Ok((du_directory_size(root)?, 0));
    }

    let mut local_size = 0u64;
    let mut local_files = 0u64;
    let fold_extra = Arc::new(AtomicU64::new(0));

    let fold_extra_cb = fold_extra.clone();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .skip_hidden(false)
        .process_read_dir(move |_depth, _path, _parent, children| {
            children.iter_mut().for_each(|each_result| {
                if let Ok(entry) = each_result {
                    if entry.file_type.is_dir() {
                        let name = entry.file_name.to_string_lossy();
                        if should_fold_name(name.as_ref()) {
                            entry.read_children_path = None;
                            let path = entry.path();
                            if let Ok(sz) = du_directory_size(&path) {
                                fold_extra_cb.fetch_add(sz, Ordering::Relaxed);
                            }
                        }
                    }
                }
            });
        });

    for entry in walker {
        cancel.check_scan()?;
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

    local_size += fold_extra.load(Ordering::Relaxed);
    cancel.check_scan()?;
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

trait ScanCancel {
    fn check_scan(&self) -> io::Result<()>;
}

impl ScanCancel for CancelToken {
    fn check_scan(&self) -> io::Result<()> {
        if self.is_cancelled() {
            Err(io::Error::new(io::ErrorKind::Interrupted, "scan cancelled"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use crate::cancel::CancelToken;

    #[test]
    fn progress_emits_each_root_child_before_done() {
        let dir = std::env::temp_dir().join(format!("vole-scan-prog-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut a = File::create(dir.join("a.txt")).unwrap();
        a.write_all(b"aa").unwrap();
        let mut b = File::create(dir.join("b.txt")).unwrap();
        b.write_all(&vec![0u8; 64]).unwrap();

        let cancel = CancelToken::new();
        let mut seen = Vec::new();
        let result = scan_directory_with_progress(&dir, &cancel, |e| {
            seen.push(e.name.clone());
        })
        .unwrap();
        assert_eq!(seen.len(), 2, "expected one progress event per root child");
        assert_eq!(result.entries.len(), 2);
        assert!(result.entries.iter().any(|e| e.name == "a.txt"));
        assert!(result.entries.iter().any(|e| e.name == "b.txt"));
        assert_eq!(result.entries[0].name, "b.txt");
        let plain = scan_directory(&dir, &cancel).unwrap();
        assert_eq!(plain.total_size, result.total_size);
        assert_eq!(plain.total_files, result.total_files);
        let _ = fs::remove_dir_all(&dir);
    }

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
    fn fold_dir_uses_du_size() {
        let dir = std::env::temp_dir().join(format!("vole-fold-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let nm = dir.join("node_modules");
        fs::create_dir(&nm).unwrap();
        let mut f = File::create(nm.join("pkg.js")).unwrap();
        f.write_all(&vec![0u8; 4096]).unwrap();

        let cancel = CancelToken::new();
        let result = scan_directory(&dir, &cancel).unwrap();
        assert!(
            result.total_size >= 4096,
            "folded node_modules should count via du, got {}",
            result.total_size
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_orbstack_child() {
        let dir = std::env::temp_dir().join(format!("vole-skip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let orb = dir.join("OrbStack");
        fs::create_dir(&orb).unwrap();
        let mut f = File::create(orb.join("disk.img")).unwrap();
        f.write_all(&vec![0u8; 4096]).unwrap();

        let cancel = CancelToken::new();
        let result = scan_directory(&dir, &cancel).unwrap();
        assert_eq!(result.total_size, 0);
        assert!(result.entries.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_returns_interrupted() {
        let dir = std::env::temp_dir().join(format!("vole-cancel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let cancel = CancelToken::new();
        cancel.cancel();
        let err = scan_directory(&dir, &cancel).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_not_followed() {
        let base = std::env::temp_dir().join(format!("vole-symlink-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let dir = base.join("scan");
        let outside = base.join("outside");
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let mut f = File::create(outside.join("heavy.bin")).unwrap();
        f.write_all(&vec![0u8; 64 * 1024]).unwrap();
        symlink(&outside, dir.join("link")).unwrap();

        let cancel = CancelToken::new();
        let result = scan_directory(&dir, &cancel).unwrap();
        assert!(
            result.total_size < 64 * 1024,
            "symlink target must not be walked, total={}",
            result.total_size
        );
        assert_eq!(result.total_files, 1);
        let _ = fs::remove_dir_all(&base);
    }
}
