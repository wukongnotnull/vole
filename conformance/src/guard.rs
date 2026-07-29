//! 一致性测试的越界护栏。
//!
//! 被测程序（mole 与 vole）都会真实删除文件，而 mole 的规则路径并不完全
//! 受 $HOME 约束。护栏对若干哨兵目录做改动前后的快照对比，任何根目录之外
//! 的变化都中止整个测试运行——不是警告。

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, PartialEq, Eq)]
pub enum ViolationKind {
    Modified,
    Removed,
    Created,
}

#[derive(Debug)]
pub struct GuardViolation {
    pub path: PathBuf,
    pub kind: ViolationKind,
}

pub struct Guard {
    root: PathBuf,
    /// 哨兵路径 → 初始 mtime。缺失表示当时不存在。
    snapshot: HashMap<PathBuf, Option<SystemTime>>,
}

impl Guard {
    /// `root` 是允许改动的唯一区域。`sentinels` 是要监视的根外目录，
    /// 递归一层收集条目——全盘快照太慢，哨兵覆盖 mole 实际会碰的位置即可。
    pub fn new(root: &Path, sentinels: &[PathBuf]) -> io::Result<Self> {
        let mut snapshot = HashMap::new();
        for dir in sentinels {
            collect(dir, &mut snapshot)?;
        }
        Ok(Guard {
            root: root.to_path_buf(),
            snapshot,
        })
    }

    pub fn assert_no_outside_changes(&self) -> Result<(), GuardViolation> {
        for (path, before) in &self.snapshot {
            if path.starts_with(&self.root) {
                continue;
            }
            let now = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            match (before, now) {
                (Some(_), None) => {
                    return Err(GuardViolation {
                        path: path.clone(),
                        kind: ViolationKind::Removed,
                    })
                }
                (None, Some(_)) => {
                    return Err(GuardViolation {
                        path: path.clone(),
                        kind: ViolationKind::Created,
                    })
                }
                (Some(a), Some(b)) if a != &b => {
                    return Err(GuardViolation {
                        path: path.clone(),
                        kind: ViolationKind::Modified,
                    })
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn collect(dir: &Path, out: &mut HashMap<PathBuf, Option<SystemTime>>) -> io::Result<()> {
    if !dir.exists() {
        out.insert(dir.to_path_buf(), None);
        return Ok(());
    }
    out.insert(
        dir.to_path_buf(),
        std::fs::metadata(dir).and_then(|m| m.modified()).ok(),
    );
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        out.insert(
            path.clone(),
            std::fs::metadata(&path).and_then(|m| m.modified()).ok(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 护栏必须能发现根目录之外的改动，否则它形同虚设。
    #[test]
    fn detects_modification_outside_root() {
        let tmp = std::env::temp_dir().join(format!("vole-guard-{}", std::process::id()));
        let root = tmp.join("root");
        let outside = tmp.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let sentinel = outside.join("sentinel");
        std::fs::write(&sentinel, b"before").unwrap();

        let guard = Guard::new(&root, std::slice::from_ref(&outside)).unwrap();

        // 模拟被测程序越界写入。
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&sentinel, b"after").unwrap();

        let violation = guard.assert_no_outside_changes().unwrap_err();
        assert_eq!(violation.path, sentinel);
        assert_eq!(violation.kind, ViolationKind::Modified);

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// 根目录内的改动是正常的，不得误报。
    #[test]
    fn allows_modification_inside_root() {
        let tmp = std::env::temp_dir().join(format!("vole-guard-ok-{}", std::process::id()));
        let root = tmp.join("root");
        let outside = tmp.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("sentinel"), b"x").unwrap();

        let guard = Guard::new(&root, std::slice::from_ref(&outside)).unwrap();
        std::fs::write(root.join("scratch"), b"y").unwrap();

        assert!(guard.assert_no_outside_changes().is_ok());

        std::fs::remove_dir_all(&tmp).ok();
    }
}
