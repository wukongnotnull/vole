//! plan 的 TOCTOU 校验原型。Phase 4 会重写，这里只为量出工作量。

use std::path::Path;

use rustix::fs::{Mode, OFlags};

/// plan 里记录的目标身份。
pub struct Identity {
    pub dev: u64,
    pub ino: u64,
}

/// 逐段打开路径，每一段都禁止跟随 symlink，最后比对身份。
///
/// 关键点：不用绝对路径字符串重新解析，那样等于把 TOCTOU 窗口又打开一次。
pub fn verify(root: &Path, relative: &Path, expect: &Identity) -> Result<(), String> {
    let mut dir = rustix::fs::open(root, OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty())
        .map_err(|e| format!("打不开 root: {e}"))?;

    let mut components: Vec<_> = relative.components().collect();
    let leaf = components.pop().ok_or("空相对路径")?;

    for comp in components {
        let next = rustix::fs::openat(
            &dir,
            comp.as_os_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|e| format!("路径段 {comp:?} 打开失败（可能是 symlink）: {e}"))?;
        dir = next;
    }

    let st = rustix::fs::statat(
        &dir,
        leaf.as_os_str(),
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|e| format!("statat 失败: {e}"))?;

    if st.st_dev as u64 != expect.dev {
        return Err("跨设备，拒绝".into());
    }
    if st.st_ino as u64 != expect.ino {
        return Err("inode 不匹配，目标已被替换".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个用例一个独立根目录，避免相互干扰。
    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vole-toctou-{}-{}", tag, std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn identity_of(p: &std::path::Path) -> Identity {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(p).unwrap();
        Identity {
            dev: m.dev(),
            ino: m.ino(),
        }
    }

    /// 攻击一：plan 里塞入一条当前文件系统上不存在的路径。
    /// verify 必须报错，调用方据此报错退出而非静默跳过。
    #[test]
    fn rejects_path_that_does_not_exist() {
        let root = scratch("nonexistent");
        std::fs::create_dir_all(root.join("Caches")).unwrap();

        let fake = Identity {
            dev: 1,
            ino: 999_999,
        };
        let err = verify(&root, std::path::Path::new("Caches/never-existed"), &fake).unwrap_err();
        assert!(err.contains("statat 失败"), "实际错误: {err}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// 攻击二：plan 生成后把末段换成指向敏感目录的 symlink。
    /// inode 不再匹配，必须拒绝。
    #[test]
    fn rejects_symlink_swapped_leaf() {
        let root = scratch("leaf");
        let cache = root.join("Caches");
        std::fs::create_dir_all(&cache).unwrap();
        let target = cache.join("blob");
        std::fs::write(&target, b"real").unwrap();

        // plan 生成时记录真实身份。
        let expect = identity_of(&target);
        assert!(verify(&root, std::path::Path::new("Caches/blob"), &expect).is_ok());

        // 攻击者把它换成 symlink。
        let sensitive = root.join("sensitive");
        std::fs::write(&sensitive, b"do not delete").unwrap();
        std::fs::remove_file(&target).unwrap();
        std::os::unix::fs::symlink(&sensitive, &target).unwrap();

        let err = verify(&root, std::path::Path::new("Caches/blob"), &expect).unwrap_err();
        assert!(err.contains("inode 不匹配"), "实际错误: {err}");
        assert!(sensitive.exists(), "敏感文件必须仍在");

        std::fs::remove_dir_all(&root).ok();
    }

    /// 攻击三：把路径的中间段换成 symlink。
    /// 这是最常见的绕过点——只检查末段的实现会在这里失守。
    #[test]
    fn rejects_symlink_swapped_intermediate() {
        let root = scratch("intermediate");
        let real_mid = root.join("Caches");
        std::fs::create_dir_all(real_mid.join("app")).unwrap();
        let target = real_mid.join("app/blob");
        std::fs::write(&target, b"real").unwrap();

        let expect = identity_of(&target);
        assert!(verify(&root, std::path::Path::new("Caches/app/blob"), &expect).is_ok());

        // 攻击者把中间段 Caches/app 换成 symlink，指向别处。
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("blob"), b"decoy").unwrap();
        std::fs::remove_dir_all(real_mid.join("app")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, real_mid.join("app")).unwrap();

        let err = verify(&root, std::path::Path::new("Caches/app/blob"), &expect).unwrap_err();
        assert!(
            err.contains("可能是 symlink"),
            "必须在打开中间段时就拒绝，实际错误: {err}"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
