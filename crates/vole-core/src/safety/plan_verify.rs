//! plan apply 阶段的 TOCTOU 身份校验（对齐设计 5.6 plan 威胁模型）。

use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

use rustix::fs::{AtFlags, Mode, OFlags};
use thiserror::Error;

use super::critical::normalize_policy_path;
use super::validate::{validate_path_for_deletion, PathProtection, ValidationError};

/// plan 条目记录的目标身份（`dev` / `ino` / `mtime`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanEntryIdentity {
    pub dev: u64,
    pub ino: u64,
    pub mtime: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanVerifyError {
    #[error("empty path")]
    Empty,
    #[error("path must be absolute")]
    NotAbsolute,
    #[error("path traversal not allowed")]
    Traversal,
    #[error("cannot open filesystem root: {0}")]
    RootOpen(String),
    #[error("path segment open failed (possible symlink): {0}")]
    SegmentOpen(String),
    #[error("stat failed: {0}")]
    StatFailed(String),
    #[error("cross-device path")]
    CrossDevice,
    #[error("inode mismatch, target was replaced")]
    InodeMismatch,
    #[error("mtime mismatch, target was modified")]
    MtimeMismatch,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanApplyError {
    #[error("policy: {0}")]
    Policy(#[from] ValidationError),
    #[error("identity: {0}")]
    Identity(#[from] PlanVerifyError),
}

/// 在 plan 生成阶段快照路径身份。
pub fn capture_plan_entry_identity(path: &Path) -> Result<PlanEntryIdentity, std::io::Error> {
    let meta = std::fs::symlink_metadata(path)?;
    Ok(PlanEntryIdentity {
        dev: meta.dev(),
        ino: meta.ino(),
        mtime: meta.mtime(),
    })
}

/// 逐段 `openat(O_NOFOLLOW)` 打开绝对路径并比对 `(dev, ino, mtime)`。
///
/// 不使用绝对路径字符串重新解析，避免在 apply 阶段重新打开 TOCTOU 窗口。
pub fn verify_plan_entry(path: &str, expect: &PlanEntryIdentity) -> Result<(), PlanVerifyError> {
    if path.is_empty() {
        return Err(PlanVerifyError::Empty);
    }
    if !path.starts_with('/') {
        return Err(PlanVerifyError::NotAbsolute);
    }

    let normalized = physical_traversal_path(path);
    if normalized.split('/').any(|part| part == "..") {
        return Err(PlanVerifyError::Traversal);
    }

    let components = path_components(&normalized);
    let Some(leaf) = components.last() else {
        return Err(PlanVerifyError::Empty);
    };
    let parents = &components[..components.len() - 1];

    let mut dir = rustix::fs::open("/", OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty())
        .map_err(|e| PlanVerifyError::RootOpen(e.to_string()))?;

    for comp in parents {
        let next = rustix::fs::openat(
            &dir,
            *comp,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|e| PlanVerifyError::SegmentOpen(format!("{comp:?}: {e}")))?;
        dir = next;
    }

    let st = rustix::fs::statat(&dir, *leaf, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|e| PlanVerifyError::StatFailed(e.to_string()))?;

    let dev = st.st_dev as u64;
    let ino = st.st_ino as u64;
    let mtime = st.st_mtime;

    if dev != expect.dev {
        return Err(PlanVerifyError::CrossDevice);
    }
    if ino != expect.ino {
        return Err(PlanVerifyError::InodeMismatch);
    }
    if mtime != expect.mtime {
        return Err(PlanVerifyError::MtimeMismatch);
    }
    Ok(())
}

/// apply 前完整闸口：策略校验 + TOCTOU 身份校验。
pub fn verify_plan_entry_for_apply(
    path: &str,
    expect: &PlanEntryIdentity,
    protection: &dyn PathProtection,
) -> Result<(), PlanApplyError> {
    validate_path_for_deletion(path, protection)?;
    verify_plan_entry(path, expect)?;
    Ok(())
}

fn path_components(path: &str) -> Vec<&str> {
    Path::new(path)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_str().unwrap_or("")),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// macOS 根级 `/var`、`/tmp` 是 symlink；`O_NOFOLLOW` 遍历需走 `/private` 物理路径。
fn physical_traversal_path(path: &str) -> String {
    let p = normalize_policy_path(path);
    if let Some(rest) = p.strip_prefix("/var/") {
        return format!("/private/var/{rest}");
    }
    if p == "/var" {
        return "/private/var".into();
    }
    if let Some(rest) = p.strip_prefix("/tmp/") {
        return format!("/private/tmp/{rest}");
    }
    if p == "/tmp" {
        return "/private/tmp".into();
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protection::AppProtection;
    use crate::safety::NoPathProtection;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::time::{Duration, SystemTime};

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vole-plan-verify-{tag}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn identity_of(path: &Path) -> PlanEntryIdentity {
        capture_plan_entry_identity(path).unwrap()
    }

    #[test]
    fn rejects_path_that_does_not_exist() {
        let root = scratch("nonexistent");
        fs::create_dir_all(root.join("Caches")).unwrap();
        let fake = PlanEntryIdentity {
            dev: 1,
            ino: 999_999,
            mtime: 0,
        };
        let path = root.join("Caches/never-existed");
        assert!(matches!(
            verify_plan_entry(&path.to_string_lossy(), &fake),
            Err(PlanVerifyError::StatFailed(_))
        ));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_symlink_swapped_leaf() {
        let root = scratch("leaf");
        let cache = root.join("Caches");
        fs::create_dir_all(&cache).unwrap();
        let target = cache.join("blob");
        fs::write(&target, b"real").unwrap();

        let expect = identity_of(&target);
        assert!(verify_plan_entry(&target.to_string_lossy(), &expect).is_ok());

        let sensitive = root.join("sensitive");
        fs::write(&sensitive, b"do not delete").unwrap();
        fs::remove_file(&target).unwrap();
        symlink(&sensitive, &target).unwrap();

        assert_eq!(
            verify_plan_entry(&target.to_string_lossy(), &expect),
            Err(PlanVerifyError::InodeMismatch)
        );
        assert!(sensitive.exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_symlink_swapped_intermediate() {
        let root = scratch("intermediate");
        let real_mid = root.join("Caches");
        fs::create_dir_all(real_mid.join("app")).unwrap();
        let target = real_mid.join("app/blob");
        fs::write(&target, b"real").unwrap();

        let expect = identity_of(&target);
        let path = target.to_string_lossy().into_owned();
        assert!(verify_plan_entry(&path, &expect).is_ok());

        let elsewhere = root.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(elsewhere.join("blob"), b"decoy").unwrap();
        fs::remove_dir_all(real_mid.join("app")).unwrap();
        symlink(&elsewhere, real_mid.join("app")).unwrap();

        assert!(matches!(
            verify_plan_entry(&path, &expect),
            Err(PlanVerifyError::SegmentOpen(_))
        ));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_mtime_mismatch() {
        let root = scratch("mtime");
        let file = root.join("blob");
        fs::write(&file, b"v1").unwrap();
        let expect = identity_of(&file);
        assert!(verify_plan_entry(&file.to_string_lossy(), &expect).is_ok());

        std::thread::sleep(Duration::from_secs(1));
        let past = SystemTime::UNIX_EPOCH + Duration::from_secs(expect.mtime as u64);
        fs::write(&file, b"v2").unwrap();
        let file_time = fs::metadata(&file).unwrap().modified().unwrap();
        assert!(file_time > past);

        assert_eq!(
            verify_plan_entry(&file.to_string_lossy(), &expect),
            Err(PlanVerifyError::MtimeMismatch)
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn apply_rejects_policy_before_identity() {
        let protection = AppProtection::new();
        let expect = PlanEntryIdentity {
            dev: 1,
            ino: 1,
            mtime: 0,
        };
        assert_eq!(
            verify_plan_entry_for_apply("/System/test", &expect, &protection),
            Err(PlanApplyError::Policy(ValidationError::CriticalSystemPath))
        );
    }

    #[test]
    fn apply_accepts_ordinary_cache_with_matching_identity() {
        let root = scratch("apply-ok");
        let cache = root.join("Caches/cache.db");
        fs::create_dir_all(cache.parent().unwrap()).unwrap();
        fs::write(&cache, b"x").unwrap();
        let expect = identity_of(&cache);
        let protection = AppProtection::new();

        verify_plan_entry_for_apply(&cache.to_string_lossy(), &expect, &protection).unwrap();

        let p = NoPathProtection;
        verify_plan_entry_for_apply(&cache.to_string_lossy(), &expect, &p).unwrap();
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_relative_and_traversal_paths() {
        let expect = PlanEntryIdentity {
            dev: 1,
            ino: 1,
            mtime: 0,
        };
        assert_eq!(
            verify_plan_entry("relative/path", &expect),
            Err(PlanVerifyError::NotAbsolute)
        );
        assert_eq!(
            verify_plan_entry("/tmp/../etc/hosts", &expect),
            Err(PlanVerifyError::Traversal)
        );
    }
}
