//! Group Containers 扫描判定。

use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum GroupCacheScanError {
    /// `~/Library/Group Containers` 存在但不可列。
    GroupContainersInaccessible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupCacheSelectResult {
    pub paths: Vec<PathBuf>,
    /// 任一候选子树 / 整规则触达规模上限。
    pub truncated: bool,
}

pub fn select_group_container_caches(
    _home: &Path,
) -> Result<GroupCacheSelectResult, GroupCacheScanError> {
    Ok(GroupCacheSelectResult {
        paths: Vec::new(),
        truncated: false,
    })
}
