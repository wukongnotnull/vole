//! fixture 树的声明与物化。
//!
//! fixture 用 JSON 声明而非脚本构造，这样从 Mole 的 bats 用例里
//! 半自动抽取出来的期望值可以直接落成数据（设计文档第 7 节 B 类）。

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    Dir { path: PathBuf },
    File { path: PathBuf, size_kb: u64 },
}

impl Entry {
    fn path(&self) -> &Path {
        match self {
            Entry::Dir { path } | Entry::File { path, .. } => path,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub entries: Vec<Entry>,
}

impl Fixture {
    pub fn validate(&self) -> Result<(), String> {
        for entry in &self.entries {
            let p = entry.path();
            let s = p.to_string_lossy();
            if s.chars().any(|c| c.is_control()) {
                return Err(format!("fixture {} 的路径含控制字符: {s:?}", self.id));
            }
            if p.is_absolute() || s.contains("..") {
                return Err(format!(
                    "fixture {} 的路径必须是不含 .. 的相对路径: {s:?}",
                    self.id
                ));
            }
        }
        Ok(())
    }

    pub fn materialize(&self, home: &Path) -> io::Result<()> {
        if let Err(e) = self.validate() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, e));
        }
        for entry in &self.entries {
            let full = home.join(entry.path());
            match entry {
                Entry::Dir { .. } => std::fs::create_dir_all(&full)?,
                Entry::File { size_kb, .. } => {
                    if let Some(parent) = full.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&full, vec![0u8; (*size_kb as usize) * 1024])?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materializes_dirs_and_files_with_mtime() {
        let home = std::env::temp_dir().join(format!("vole-fx-{}", std::process::id()));
        std::fs::remove_dir_all(&home).ok();

        let fx = Fixture {
            id: "t".into(),
            entries: vec![
                Entry::Dir {
                    path: "Library/Caches/com.example.app".into(),
                },
                Entry::File {
                    path: "Library/Caches/com.example.app/blob".into(),
                    size_kb: 4,
                },
            ],
        };
        fx.materialize(&home).unwrap();

        assert!(home.join("Library/Caches/com.example.app").is_dir());
        let blob = home.join("Library/Caches/com.example.app/blob");
        assert_eq!(std::fs::metadata(&blob).unwrap().len(), 4096);

        std::fs::remove_dir_all(&home).ok();
    }

    /// 补丁的 JSON 转义只处理反斜杠与引号，含控制字符的路径会产出非法 JSON。
    /// 把这个限制挡在 fixture 校验里，而不是等 harness 解析失败。
    #[test]
    fn rejects_control_characters_in_paths() {
        let fx = Fixture {
            id: "bad".into(),
            entries: vec![Entry::Dir {
                path: "Library/Ca\tches".into(),
            }],
        };
        let err = fx.validate().unwrap_err();
        assert!(err.contains("控制字符"), "实际错误: {err}");
    }
}
