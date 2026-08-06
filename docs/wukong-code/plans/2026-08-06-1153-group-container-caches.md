# Group Container Caches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `vole clean` plan→apply 落地与 Mole `clean_group_container_caches` 同形的 Group Containers Logs/Caches/tmp 叶节点清理（零保护层改动；1.7.0）。

**Architecture:** 新建 `vole-core::groupcaches` 模块做扫描/判定与 label；经 `custom` handler `group_container_caches` 接线；plan 走既有 `validate_path_for_deletion`（**无** stubs 式豁免）；apply 走普通 `mole_delete_verified`（**无** rule_id 早分支）。根不可读 → FDA degrade；规模超限 → 非整规则 degrade 的 truncated notice。

**Tech Stack:** Rust / macOS / vole-core / tempfile 单测 / TOML 规则。

## Global Constraints

- 版本意图：**1.7.0**（SemVer MINOR）；规则数 **514 → 515**；**不 bump** `schema_version`
- `rule_id`：**`group-container-caches`**（**无** `orphaned-` 前缀）；handler：`group_container_caches`；`category`：`app-caches`
- 规则文件：**`data/rules/app-caches.toml`**（**禁止**写入 `zzz-orphaned.toml`）
- **禁止**修改：`should_protect_path` / `is_container_cache_or_tmp` / `is_explicit_clean_cache_path` / `protection.toml` / apply carve-out / stubs 式 protect 豁免
- 扫描根写死：`$HOME/Library/Group Containers`；叶节点（mindepth 1 maxdepth 1）；含隐藏项（对齐 Mole `dotglob`）
- 规模上限：单候选子树 **200**、整规则 **2000**；触发 → 该子树不提候选 + `PlanNotice::GroupContainersTruncated`；**禁止**退化成「提目录本身」
- protected 判定（比 Mole 严）：原 id / 去 `group.` / 去前导 TeamID（`^[A-Z0-9]{10}\.`）/ 两者都去，任一命中 `should_protect_data` 即 protected
- Apple 前缀硬跳过：`com.apple.*` / `group.com.apple.*` / `systemgroup.com.apple.*`
- Safari 扩展探测 **fail-closed**（含 Containers 同 id 目录不可读 → 跳过）
- home 经 `VOLE_TEST_HOME` / 既有 `HOME` 注入覆盖（plan 已支持）

---

## File Structure

| 文件 | 职责 |
|---|---|
| **Create** `crates/vole-core/src/groupcaches/mod.rs` | 常量、`is_apple_group_container`、`strip_team_id_prefix`、`is_group_container_protected`、`group_container_cache_label` |
| **Create** `crates/vole-core/src/groupcaches/select.rs` | `select_group_container_caches`、`GroupCacheScanError`、`GroupCacheSelectResult{paths, truncated}`、Safari 探测、叶枚举、上限 |
| **Modify** `crates/vole-core/src/lib.rs` | `pub mod groupcaches;` |
| **Modify** `crates/vole-core/src/rules/custom_handlers.rs` | `CustomDegrade::GroupContainersInaccessible` + handler 分派 |
| **Modify** `crates/vole-core/src/ops/plan.rs` | `PlanNotice::{GroupContainersInaccessible, GroupContainersTruncated}` + degrade/truncated 映射 + label 分支 |
| **Modify** `crates/vole-core/src/ops/coverage.rs` | `GROUP_CONTAINERS_WARN`、coverage 文案、notice 追加 |
| **Modify** `crates/vole-core/src/ops/mod.rs` | 导出新 WARN 常量 |
| **Modify** `crates/vole-cli/src/clean.rs` | human plan 打印 FDA warn + truncated notice |
| **Modify** `data/rules/app-caches.toml` | 追加规则 |
| **Modify** `README.md`、`Cargo.toml`、`docs/releases/v1.7.0.md`、`docs/findings/...`、`Formula/vole.rb` | 发版 |

**不做：** `apply_plan.rs` 任何 `rule_id` 分支；`protection/` 任何修改。

---

### Task 1: `groupcaches` 模块骨架 + 保护判定 + label

**Files:**
- Create: `crates/vole-core/src/groupcaches/mod.rs`
- Create: `crates/vole-core/src/groupcaches/select.rs`（先放空 `select` stub，下一 task 填）
- Modify: `crates/vole-core/src/lib.rs`（加 `pub mod groupcaches;`，紧邻 `pub mod stubs;`）

**Interfaces:**
- Consumes: `crate::protection::{should_protect_data, ProtectionCatalog}`
- Produces:
  - `pub const GROUP_CONTAINER_CACHE_RULE_ID: &str = "group-container-caches";`
  - `pub const MAX_LEAVES_PER_CANDIDATE: usize = 200;`
  - `pub const MAX_LEAVES_TOTAL: usize = 2000;`
  - `pub fn is_apple_group_container(id: &str) -> bool`
  - `pub fn strip_team_id_prefix(id: &str) -> &str`
  - `pub fn is_group_container_protected(id: &str, catalog: &ProtectionCatalog) -> bool`
  - `pub fn group_container_cache_label(path: &Path, home: &Path) -> String`

- [ ] **Step 1: Write the failing tests**（写在 `mod.rs` 的 `#[cfg(test)]`）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protection::ProtectionCatalog;
    use std::path::Path;

    #[test]
    fn apple_prefix_variants() {
        assert!(is_apple_group_container("com.apple.notes"));
        assert!(is_apple_group_container("group.com.apple.notes"));
        assert!(is_apple_group_container("systemgroup.com.apple.notes"));
        assert!(!is_apple_group_container("group.com.example.app"));
        assert!(!is_apple_group_container("com.macpaw.CleanMyMac"));
    }

    #[test]
    fn strip_team_id_only_ten_alnum() {
        assert_eq!(
            strip_team_id_prefix("HUAQ24HBR6.dev.orbstack"),
            "dev.orbstack"
        );
        assert_eq!(
            strip_team_id_prefix("S8EX82NJP6.com.tencent.xinWeChat"),
            "com.tencent.xinWeChat"
        );
        assert_eq!(strip_team_id_prefix("group.com.example"), "group.com.example");
        assert_eq!(strip_team_id_prefix("abcdefghij.com.x"), "com.x"); // 10 alnum
        assert_eq!(strip_team_id_prefix("short.com.x"), "short.com.x"); // <10
        assert_eq!(strip_team_id_prefix("ABCDEFGHIJ.com.x"), "com.x");
        // 含小写不算 TeamID（Apple TeamID 为大写字母+数字）
        assert_eq!(strip_team_id_prefix("abcdefghIJ.com.x"), "com.x"); // still 10 alnum A-Z0-9? 'a'..'j' are not A-Z0-9
        // 修正：小写不匹配 ^[A-Z0-9]{10}
        assert_eq!(strip_team_id_prefix("abcdefghij.com.x"), "abcdefghij.com.x");
    }

    #[test]
    fn protected_via_raw_id() {
        let c = ProtectionCatalog::embedded();
        // com.macpaw.* 在 data_protected_bundles
        assert!(is_group_container_protected("com.macpaw.CleanMyMac", &c));
    }

    #[test]
    fn protected_via_group_strip() {
        let c = ProtectionCatalog::embedded();
        // 若 catalog 仅登记无 group. 的 id，去前缀后须命中
        // CleanMyMac 通常登记为 com.macpaw.* —— 带 group. 前缀时依赖 strip
        assert!(is_group_container_protected(
            "group.com.macpaw.CleanMyMac",
            &c
        ));
    }

    #[test]
    fn protected_via_teamid_strip_for_tencent() {
        let c = ProtectionCatalog::embedded();
        // data_protected 含 com.tencent.* 时，TeamID 前缀也应判 protected
        // 若当前 catalog 不含腾讯，本测用「自造」路径：至少保证去 TeamID 后再次查询
        // 用 CleanMyMac + 假 TeamID 验证归一化链路
        assert!(is_group_container_protected(
            "S8EX82NJP6.com.macpaw.CleanMyMac",
            &c
        ));
    }

    #[test]
    fn non_protected_example_app() {
        let c = ProtectionCatalog::embedded();
        assert!(!is_group_container_protected("group.com.example.app", &c));
        assert!(!is_group_container_protected("com.example.app", &c));
    }

    #[test]
    fn label_uses_relative_under_group_containers() {
        let home = Path::new("/Users/t");
        let p = Path::new(
            "/Users/t/Library/Group Containers/group.com.example.app/Library/Caches/foo",
        );
        assert_eq!(
            group_container_cache_label(p, home),
            "Group container cache: group.com.example.app/Library/Caches/foo"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vole-core --lib groupcaches::tests -- --nocapture`
Expected: FAIL（`groupcaches` 模块 / 函数不存在）

- [ ] **Step 3: Minimal implementation**

`crates/vole-core/src/groupcaches/mod.rs`:

```rust
//! Group Containers Logs/Caches/tmp 叶清理（Mole `clean_group_container_caches` 同形）。
//! 本期零保护层改动；见 design 2026-08-06-1133。

mod select;

pub use select::{
    select_group_container_caches, GroupCacheScanError, GroupCacheSelectResult,
};

use std::path::Path;

use crate::protection::{should_protect_data, ProtectionCatalog};

pub const GROUP_CONTAINER_CACHE_RULE_ID: &str = "group-container-caches";
pub const MAX_LEAVES_PER_CANDIDATE: usize = 200;
pub const MAX_LEAVES_TOTAL: usize = 2000;

pub fn is_apple_group_container(id: &str) -> bool {
    id.starts_with("com.apple.")
        || id.starts_with("group.com.apple.")
        || id.starts_with("systemgroup.com.apple.")
}

/// 剥前导 TeamID（恰好 10 位 `[A-Z0-9]` + `.`）。不匹配则原样返回。
pub fn strip_team_id_prefix(id: &str) -> &str {
    let bytes = id.as_bytes();
    if bytes.len() > 11
        && bytes[10] == b'.'
        && bytes[..10]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        &id[11..]
    } else {
        id
    }
}

pub fn is_group_container_protected(id: &str, catalog: &ProtectionCatalog) -> bool {
    let no_group = id.strip_prefix("group.").unwrap_or(id);
    let no_team = strip_team_id_prefix(id);
    let no_team_no_group = no_team.strip_prefix("group.").unwrap_or(no_team);
    should_protect_data(id, catalog)
        || should_protect_data(no_group, catalog)
        || should_protect_data(no_team, catalog)
        || should_protect_data(no_team_no_group, catalog)
}

pub fn group_container_cache_label(path: &Path, home: &Path) -> String {
    let root = home.join("Library/Group Containers");
    let rel = path
        .strip_prefix(&root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });
    format!("Group container cache: {rel}")
}

// 把 Step 1 的 tests 模块粘贴到此处
```

`crates/vole-core/src/groupcaches/select.rs`（骨架，下一 task 填满）:

```rust
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
```

`lib.rs` 在 `pub mod stubs;` 旁加 `pub mod groupcaches;`。

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vole-core --lib groupcaches::tests -- --nocapture`
Expected: PASS（若 `protected_via_group_strip` / tencent 因 catalog 缺条目失败，改用 `com.macpaw.CleanMyMac` + TeamID/`group.` 变体；不要改 catalog）

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/groupcaches crates/vole-core/src/lib.rs
git commit -m "feat(groupcaches): add protect helpers and label (zero protection change)"
```

---

### Task 2: `select_group_container_caches` 扫描流水线

**Files:**
- Modify: `crates/vole-core/src/groupcaches/select.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Consumes: Task 1 的 `is_apple_group_container` / `is_group_container_protected` / 上限常量；`ProtectionCatalog::embedded()`
- Produces: 完整 `select_group_container_caches(home) -> Result<GroupCacheSelectResult, GroupCacheScanError>`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn temp_home(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "vole-gcc-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Library/Group Containers")).unwrap();
        root
    }

    fn mk_leaves(home: &Path, container: &str, sub: &str, names: &[&str]) {
        let dir = home
            .join("Library/Group Containers")
            .join(container)
            .join(sub);
        fs::create_dir_all(&dir).unwrap();
        for n in names {
            fs::write(dir.join(n), b"x").unwrap();
        }
    }

    #[test]
    fn non_protected_selects_logs_caches_tmp_leaves() {
        let home = temp_home("np");
        mk_leaves(
            &home,
            "group.com.example.app",
            "Logs",
            &["a.log", ".DS_Store"],
        );
        mk_leaves(
            &home,
            "group.com.example.app",
            "Library/Caches",
            &["c1"],
        );
        mk_leaves(&home, "group.com.example.app", "tmp", &["t1"]);
        // 非候选子树不应入选
        mk_leaves(
            &home,
            "group.com.example.app",
            "Library/Application Support",
            &["keep"],
        );

        let got = select_group_container_caches(&home).unwrap();
        assert!(!got.truncated);
        let rels: Vec<_> = got
            .paths
            .iter()
            .map(|p| {
                p.strip_prefix(home.join("Library/Group Containers"))
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect();
        assert!(rels.iter().any(|r| r.ends_with("Logs/a.log")));
        assert!(rels.iter().any(|r| r.ends_with("Logs/.DS_Store")));
        assert!(rels.iter().any(|r| r.ends_with("Library/Caches/c1")));
        assert!(rels.iter().any(|r| r.ends_with("tmp/t1")));
        assert!(!rels.iter().any(|r| r.contains("Application Support")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn protected_macpaw_only_logs_candidates_from_handler() {
        // handler 仍提 Logs；Caches/tmp 不提（plan 层另测 ProtectedPath skip）
        let home = temp_home("prot");
        mk_leaves(&home, "com.macpaw.CleanMyMac", "Logs", &["x.log"]);
        mk_leaves(
            &home,
            "com.macpaw.CleanMyMac",
            "Library/Caches",
            &["y"],
        );
        let got = select_group_container_caches(&home).unwrap();
        assert!(got
            .paths
            .iter()
            .any(|p| p.ends_with("Logs/x.log")));
        assert!(!got
            .paths
            .iter()
            .any(|p| p.to_string_lossy().contains("Caches")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn teamid_protected_vendor_skips_caches() {
        let home = temp_home("teamid");
        // TeamID + com.macpaw → 收严后 protected → 不提 Caches
        mk_leaves(
            &home,
            "S8EX82NJP6.com.macpaw.CleanMyMac",
            "Logs",
            &["x.log"],
        );
        mk_leaves(
            &home,
            "S8EX82NJP6.com.macpaw.CleanMyMac",
            "Caches",
            &["y"],
        );
        let got = select_group_container_caches(&home).unwrap();
        assert!(got.paths.iter().any(|p| p.ends_with("Logs/x.log")));
        assert!(!got
            .paths
            .iter()
            .any(|p| p.to_string_lossy().contains("Caches")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn apple_and_notes_skipped() {
        let home = temp_home("apple");
        mk_leaves(&home, "group.com.apple.notes", "Logs", &["n.log"]);
        mk_leaves(&home, "com.apple.foo", "Caches", &["c"]);
        let got = select_group_container_caches(&home).unwrap();
        assert!(got.paths.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn safari_extension_container_skipped_fail_closed() {
        let home = temp_home("safari");
        mk_leaves(
            &home,
            "group.com.example.ext",
            "Library/Caches",
            &["c"],
        );
        // 对应 Containers 下有 Safari 字样条目
        let cdir = home
            .join("Library/Containers/group.com.example.ext");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(cdir.join("SomethingSafariWebExtension"), b"x").unwrap();

        let got = select_group_container_caches(&home).unwrap();
        assert!(got.paths.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn safari_probe_unreadable_containers_fail_closed() {
        let home = temp_home("safari-deny");
        mk_leaves(
            &home,
            "group.com.example.ext",
            "Library/Caches",
            &["c"],
        );
        let cdir = home
            .join("Library/Containers/group.com.example.ext");
        fs::create_dir_all(&cdir).unwrap();
        fs::set_permissions(&cdir, fs::Permissions::from_mode(0o000)).unwrap();

        let got = select_group_container_caches(&home).unwrap();
        fs::set_permissions(&cdir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(got.paths.is_empty(), "unreadable Containers ⇒ skip container");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn symlink_container_and_leaf_skipped() {
        let home = temp_home("sym");
        let real = home.join("real-container");
        fs::create_dir_all(real.join("Caches")).unwrap();
        fs::write(real.join("Caches/c"), b"x").unwrap();
        std::os::unix::fs::symlink(
            &real,
            home.join("Library/Group Containers/group.com.example.app"),
        )
        .unwrap();

        let home2 = temp_home("sym-leaf");
        let caches = home2
            .join("Library/Group Containers/group.com.example.app/Caches");
        fs::create_dir_all(&caches).unwrap();
        let target = home2.join("outside");
        fs::write(&target, b"x").unwrap();
        std::os::unix::fs::symlink(&target, caches.join("c")).unwrap();

        assert!(select_group_container_caches(&home)
            .unwrap()
            .paths
            .is_empty());
        assert!(select_group_container_caches(&home2)
            .unwrap()
            .paths
            .is_empty());
        let _ = fs::remove_dir_all(&home);
        let _ = fs::remove_dir_all(&home2);
    }

    #[test]
    fn missing_root_ok_empty_unreadable_root_errors() {
        let bare = std::env::temp_dir().join(format!(
            "vole-gcc-noroot-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&bare);
        fs::create_dir_all(&bare).unwrap();
        assert!(select_group_container_caches(&bare)
            .unwrap()
            .paths
            .is_empty());
        let _ = fs::remove_dir_all(&bare);

        let home = temp_home("denied");
        let root = home.join("Library/Group Containers");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
        let got = select_group_container_caches(&home);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(got, Err(GroupCacheScanError::GroupContainersInaccessible));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn per_candidate_cap_skips_whole_tree_and_sets_truncated() {
        let home = temp_home("cap");
        let logs = home
            .join("Library/Group Containers/group.com.example.app/Logs");
        fs::create_dir_all(&logs).unwrap();
        for i in 0..(super::super::MAX_LEAVES_PER_CANDIDATE + 1) {
            fs::write(logs.join(format!("f{i}")), b"x").unwrap();
        }
        // 另一个未超限的候选仍可入选
        mk_leaves(
            &home,
            "group.com.example.app",
            "tmp",
            &["only"],
        );

        let got = select_group_container_caches(&home).unwrap();
        assert!(got.truncated);
        assert!(
            !got.paths
                .iter()
                .any(|p| p.to_string_lossy().contains("/Logs/")),
            "over-cap tree must contribute zero leaves"
        );
        assert!(got.paths.iter().any(|p| p.ends_with("tmp/only")));
        // 禁止把 Logs 目录本身当候选
        assert!(!got.paths.iter().any(|p| p.ends_with("Logs")));
        let _ = fs::remove_dir_all(&home);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vole-core --lib groupcaches::select::tests -- --nocapture`
Expected: FAIL（骨架返回空）

- [ ] **Step 3: Minimal implementation**

替换 `select.rs` 为：

```rust
//! Group Containers 扫描判定（Mole `clean_group_container_caches` 同形）。

use std::fs;
use std::path::{Path, PathBuf};

use crate::protection::ProtectionCatalog;

use super::{
    is_apple_group_container, is_group_container_protected, MAX_LEAVES_PER_CANDIDATE,
    MAX_LEAVES_TOTAL,
};

#[derive(Debug, PartialEq, Eq)]
pub enum GroupCacheScanError {
    GroupContainersInaccessible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupCacheSelectResult {
    pub paths: Vec<PathBuf>,
    pub truncated: bool,
}

pub fn select_group_container_caches(
    home: &Path,
) -> Result<GroupCacheSelectResult, GroupCacheScanError> {
    let root = home.join("Library/Group Containers");
    if !root.exists() {
        return Ok(GroupCacheSelectResult {
            paths: Vec::new(),
            truncated: false,
        });
    }
    let entries = fs::read_dir(&root).map_err(|_| GroupCacheScanError::GroupContainersInaccessible)?;
    let catalog = ProtectionCatalog::embedded();

    let mut out = Vec::new();
    let mut truncated = false;

    for entry in entries.flatten() {
        let dir = entry.path();
        let Ok(meta) = fs::symlink_metadata(&dir) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        // 不可读容器跳过（避免重复 TCC），不 degrade
        if fs::read_dir(&dir).is_err() {
            continue;
        }
        let Some(id) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_apple_group_container(id) {
            continue;
        }
        if looks_like_safari_web_extension(home, id) {
            continue;
        }
        let protected = is_group_container_protected(id, &catalog);

        let mut candidates: Vec<PathBuf> = vec![dir.join("Logs"), dir.join("Library/Logs")];
        if !protected {
            candidates.extend([
                dir.join("tmp"),
                dir.join("Library/tmp"),
                dir.join("Caches"),
                dir.join("Library/Caches"),
            ]);
        }

        for cand in candidates {
            match collect_leaves(&cand, &mut out, &mut truncated) {
                CollectOutcome::Continue => {}
                CollectOutcome::StopTotal => {
                    out.sort();
                    return Ok(GroupCacheSelectResult {
                        paths: out,
                        truncated: true,
                    });
                }
            }
        }
    }

    out.sort();
    Ok(GroupCacheSelectResult {
        paths: out,
        truncated,
    })
}

enum CollectOutcome {
    Continue,
    StopTotal,
}

fn collect_leaves(
    candidate: &Path,
    out: &mut Vec<PathBuf>,
    truncated: &mut bool,
) -> CollectOutcome {
    let Ok(meta) = fs::symlink_metadata(candidate) else {
        return CollectOutcome::Continue;
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return CollectOutcome::Continue;
    }
    let Ok(rd) = fs::read_dir(candidate) else {
        return CollectOutcome::Continue;
    };

    let mut leaves = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(m) = fs::symlink_metadata(&path) else {
            continue;
        };
        if m.file_type().is_symlink() {
            continue;
        }
        leaves.push(path);
        if leaves.len() > MAX_LEAVES_PER_CANDIDATE {
            *truncated = true;
            // 整树不提任何叶子
            return CollectOutcome::Continue;
        }
    }
    if leaves.len() > MAX_LEAVES_PER_CANDIDATE {
        *truncated = true;
        return CollectOutcome::Continue;
    }

    for path in leaves {
        if out.len() >= MAX_LEAVES_TOTAL {
            *truncated = true;
            return CollectOutcome::StopTotal;
        }
        out.push(path);
    }
    CollectOutcome::Continue
}

/// Safari Web Extension：若 `~/Library/Containers/<id>` 存在，列其顶层；
/// 任一名字（不分大小写）含 `safari` → 跳过；目录不可读 → 跳过（fail-closed）。
fn looks_like_safari_web_extension(home: &Path, container_id: &str) -> bool {
    let containers = home.join("Library/Containers").join(container_id);
    if !containers.exists() {
        return false;
    }
    let Ok(rd) = fs::read_dir(&containers) else {
        return true; // fail-closed
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let Some(s) = name.to_str() else {
            continue;
        };
        if s.to_ascii_lowercase().contains("safari") {
            return true;
        }
    }
    false
}

// 粘贴 Step 1 的 tests
```

注意：单测里 `super::super::MAX_LEAVES_PER_CANDIDATE` 若模块路径不便，改为 `crate::groupcaches::MAX_LEAVES_PER_CANDIDATE`。

目录级白名单：spec §5.7 说「白名单命中整树跳过」——**plan 层已对叶子做白名单**，handler 不做第二份白名单（YAGNI；与 container stubs 一致）。若后续要目录级，另开 task；本期测试不覆盖目录白名单。

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vole-core --lib groupcaches:: -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/groupcaches/select.rs
git commit -m "feat(groupcaches): select leaf caches with caps and safari fail-closed"
```

---

### Task 3: Handler + PlanNotice 接线 + TOML 规则

**Files:**
- Modify: `crates/vole-core/src/rules/custom_handlers.rs`
- Modify: `crates/vole-core/src/ops/plan.rs`（`PlanNotice` 两变体 + degrade/truncated 处理 + label）
- Modify: `data/rules/app-caches.toml`（文件末尾追加）

**Interfaces:**
- Consumes: `select_group_container_caches` / `GroupCacheScanError` / `GROUP_CONTAINER_CACHE_RULE_ID` / `group_container_cache_label`
- Produces: handler `group_container_caches`；`CustomDegrade::GroupContainersInaccessible`；`PlanNotice::{GroupContainersInaccessible, GroupContainersTruncated}`

- [ ] **Step 1: Write failing handler + plan tests**

在 `custom_handlers.rs` tests 末尾追加：

```rust
#[test]
fn group_container_caches_handler_selects_and_degrades() {
    use crate::orphan::FakeOrphanDeps;
    use std::os::unix::fs::PermissionsExt;

    let rule = Rule {
        id: "group-container-caches".into(),
        category: None,
        label: "t".into(),
        platform: vec![],
        paths: vec![],
        impact: None,
        disabled: false,
        last_verified: None,
        strategy: crate::rules::schema::StrategyConfig {
            kind: crate::rules::schema::StrategyKind::Custom,
            keep: None,
            env_override: None,
            days: None,
            names: None,
            handler: Some("group_container_caches".into()),
        },
        guards: Default::default(),
    };
    let deps = FakeOrphanDeps::default();

    let home = tempfile::tempdir().unwrap();
    let leaf = home
        .path()
        .join("Library/Group Containers/group.com.example.app/Logs");
    fs::create_dir_all(&leaf).unwrap();
    fs::write(leaf.join("a.log"), b"x").unwrap();
    let got = select_custom("group_container_caches", &[], home.path(), &rule, &deps);
    assert!(got.degrade.is_none());
    assert_eq!(got.paths.len(), 1);
    assert!(got.paths[0].ends_with("Logs/a.log"));

    let denied = tempfile::tempdir().unwrap();
    let root = denied.path().join("Library/Group Containers");
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
    let got = select_custom("group_container_caches", &[], denied.path(), &rule, &deps);
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(got.paths.is_empty());
    assert_eq!(
        got.degrade,
        Some(CustomDegrade::GroupContainersInaccessible)
    );
}
```

**重要：`CustomSelectResult` 目前只有 `paths` + `degrade`，没有 `truncated`。** truncated 不能塞进 degrade。接线方式：

1. handler 返回 paths；若 select 结果 `truncated == true`，handler 通过**新字段**告知 plan：
2. **方案（采纳）**：扩展 `CustomSelectResult`：

```rust
pub struct CustomSelectResult {
    pub paths: Vec<PathBuf>,
    pub degrade: Option<CustomDegrade>,
    /// 非 degrade：规模截断 notice
    pub truncated: bool,
}
```

同步改 `CustomSelectResult::ok` 设 `truncated: false`；所有现有构造点补字段（编译器会标全）。plan 在 custom 分支：

```rust
if result.truncated {
    if !notices.contains(&PlanNotice::GroupContainersTruncated) {
        notices.push(PlanNotice::GroupContainersTruncated);
    }
}
```

在 `plan.rs` tests 追加（对齐现有 stub 测样）：

```rust
fn group_container_cache_rule() -> Rule {
    Rule {
        id: "group-container-caches".into(),
        category: Some("app-caches".into()),
        label: "Group container caches".into(),
        platform: vec!["macos".into()],
        paths: vec!["~/Library/Group Containers".into()],
        impact: None,
        disabled: false,
        last_verified: None,
        strategy: StrategyConfig {
            kind: crate::rules::StrategyKind::Custom,
            keep: None,
            env_override: None,
            days: None,
            names: None,
            handler: Some("group_container_caches".into()),
        },
        guards: Default::default(),
    }
}

#[test]
fn plan_group_container_caches_selects_leaf_with_label() {
    let _guard = test_env::lock();
    let home = scratch("gcc-plan");
    let logs = home.join("Library/Group Containers/group.com.example.app/Logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(logs.join("a.log"), b"x").unwrap();
    std::env::set_var("HOME", &home);

    let orch = Orchestrator::new(crate::cancel::CancelToken::new(), None);
    let plan = orch
        .build_plan(&[group_container_cache_rule()], &AppProtection::new(), &[])
        .unwrap();

    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.entries[0].rule_id, "group-container-caches");
    assert_eq!(
        plan.entries[0].label,
        "Group container cache: group.com.example.app/Logs/a.log"
    );
    assert!(plan.notices.is_empty());
    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn plan_group_container_caches_degrades_when_root_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    let _guard = test_env::lock();
    let home = scratch("gcc-degrade");
    let root = home.join("Library/Group Containers");
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
    std::env::set_var("HOME", &home);

    let (tx, rx) = unbounded();
    let orch = Orchestrator::new(crate::cancel::CancelToken::new(), Some(tx));
    let plan = orch
        .build_plan(&[group_container_cache_rule()], &AppProtection::new(), &[])
        .unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(plan.entries.is_empty());
    assert!(plan
        .notices
        .contains(&PlanNotice::GroupContainersInaccessible));
    let mut saw = false;
    while let Ok(ev) = rx.try_recv() {
        if let StreamEvent::Skipped {
            rule_id,
            reason: SkipReason::TccDenied,
        } = ev
        {
            if rule_id == "group-container-caches" {
                saw = true;
            }
        }
    }
    assert!(saw);
    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn plan_protected_macpaw_logs_skipped_by_protection_gate() {
    // handler 会提 Logs 叶；plan 层 validate_path_for_deletion 因步骤 3 拒绝
    let _guard = test_env::lock();
    let home = scratch("gcc-prot-skip");
    let logs = home.join("Library/Group Containers/com.macpaw.CleanMyMac/Logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(logs.join("x.log"), b"x").unwrap();
    std::env::set_var("HOME", &home);

    let (tx, rx) = unbounded();
    let orch = Orchestrator::new(crate::cancel::CancelToken::new(), Some(tx));
    let plan = orch
        .build_plan(&[group_container_cache_rule()], &AppProtection::new(), &[])
        .unwrap();

    assert!(
        plan.entries.is_empty(),
        "protected id Logs must not enter plan: {:?}",
        plan.entries
    );
    let mut saw_skip = false;
    while let Ok(ev) = rx.try_recv() {
        if let StreamEvent::Skipped { rule_id, .. } = ev {
            if rule_id == "group-container-caches" {
                saw_skip = true;
            }
        }
    }
    assert!(saw_skip);
    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p vole-core --lib group_container_caches_handler -- --nocapture`
Expected: FAIL（handler / degrade 变体不存在）

- [ ] **Step 3: Wire implementation**

1. `CustomDegrade` 加变体 `GroupContainersInaccessible`
2. `CustomSelectResult` 加 `truncated: bool`；`ok()` 设 false；所有字面量构造补齐
3. `select_custom` match 加：

```rust
"group_container_caches" => group_container_caches(),
```

```rust
fn group_container_caches() -> CustomSelectResult {
    match crate::groupcaches::select_group_container_caches(
        // home 由 select_custom 传入 —— 改签名：
    ) { ... }
}
```

实际：

```rust
"group_container_caches" => group_container_caches(home),

fn group_container_caches(home: &Path) -> CustomSelectResult {
    match crate::groupcaches::select_group_container_caches(home) {
        Ok(r) => CustomSelectResult {
            paths: r.paths,
            degrade: None,
            truncated: r.truncated,
        },
        Err(crate::groupcaches::GroupCacheScanError::GroupContainersInaccessible) => {
            CustomSelectResult {
                paths: Vec::new(),
                degrade: Some(CustomDegrade::GroupContainersInaccessible),
                truncated: false,
            }
        }
    }
}
```

4. `PlanNotice`：

```rust
pub enum PlanNotice {
    OrphanLibraryInaccessible,
    SystemServicesInaccessible,
    ContainersInaccessible,
    GroupContainersInaccessible,
    GroupContainersTruncated,
}
```

5. plan custom 分支，在既有 degrade 处理后追加：

```rust
if let Some(CustomDegrade::GroupContainersInaccessible) = result.degrade {
    self.emit(StreamEvent::Skipped {
        rule_id: rule.id.clone(),
        reason: SkipReason::TccDenied,
    });
    if !notices.contains(&PlanNotice::GroupContainersInaccessible) {
        notices.push(PlanNotice::GroupContainersInaccessible);
    }
}
if result.truncated
    && !notices.contains(&PlanNotice::GroupContainersTruncated)
{
    notices.push(PlanNotice::GroupContainersTruncated);
}
```

6. label 分支追加：

```rust
} else if rule.id == crate::groupcaches::GROUP_CONTAINER_CACHE_RULE_ID {
    crate::groupcaches::group_container_cache_label(&path, &home)
```

7. `app-caches.toml` 末尾追加：

```toml

[[rule]]
id = "group-container-caches"
category = "app-caches"
label = "Group container caches"
platform = ["macos"]
paths = ["~/Library/Group Containers"]
impact = "Group Containers 内 Logs（全部）与非 data_protected 容器的 Caches/tmp 叶节点；跳过 Apple / Safari 扩展；受保护容器 Logs 仍可能被保护层跳过；apply 走普通废纸篓删除"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "custom"
handler = "group_container_caches"
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p vole-core --lib group_container_caches_handler plan_group_container_caches -- --nocapture
cargo test -p vole-core --lib rules::load -- --nocapture
```

Expected: PASS；加载后 enabled 规则含 `group-container-caches`。若有「最后一条启用规则」断言，**不应**被本规则碰破（本规则不在 `zzz-orphaned.toml`）。

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/rules/custom_handlers.rs \
  crates/vole-core/src/ops/plan.rs \
  data/rules/app-caches.toml
git commit -m "feat: wire group-container-caches handler and plan notices"
```

---

### Task 4: Coverage + CLI warn + README

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs`
- Modify: `crates/vole-core/src/ops/mod.rs`（导出 `GROUP_CONTAINERS_WARN`）
- Modify: `crates/vole-cli/src/clean.rs`
- Modify: `README.md`（规则数 514→515；Mole 对比句）

- [ ] **Step 1: Update failing coverage assertions first**

把 `coverage_note_mentions_mole_and_count` 里：

```rust
assert!(unported.contains("Group Containers 泛清理"));
```

改为：

```rust
assert!(
    !unported.contains("Group Containers 泛清理"),
    "group container caches partial coverage is shipped"
);
assert!(note.contains("Group Containers logs/caches"));
assert!(unported.contains("真 sudo 删除"));
assert!(unported.contains("受保护容器的组容器缓存") || unported.contains("受保护容器"));
```

并在 `coverage_with_orphan_notices` 测试追加对 `GroupContainersInaccessible` / `GROUP_CONTAINERS_WARN` 的断言。

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p vole-core --lib ops::coverage -- --nocapture`
Expected: FAIL（文案仍写「泛清理」未移植）

- [ ] **Step 3: Implement**

`coverage.rs`：

```rust
pub const GROUP_CONTAINERS_WARN: &str = "注意：group-container-caches 已跳过（无法读取 ~/Library/Group Containers）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。";

pub const GROUP_CONTAINERS_TRUNCATED_WARN: &str = "注意：group-container-caches 部分候选子树因条目过多已跳过（单树 >200 或整规则 >2000）。可用 Mole 清理或缩小范围后重试。";
```

`coverage_note` 文案：

```text
… container stubs（CleanMyMac allowlist）、Group Containers logs/caches（Mole 同形，受保护容器与 bundle 命名文件除外）已落地。
仍未移植：真 sudo 删除、受保护容器的组容器缓存、其它 sudo/系统路径（如 Rosetta `/Library`、claude pending-uploads）。
```

`coverage_with_orphan_notices` 追加：

```rust
if notices.contains(&PlanNotice::GroupContainersInaccessible) {
    out = format!("{out}\n{GROUP_CONTAINERS_WARN}");
}
if notices.contains(&PlanNotice::GroupContainersTruncated) {
    out = format!("{out}\n{GROUP_CONTAINERS_TRUNCATED_WARN}");
}
```

`ops/mod.rs` 导出两个新 WARN。

`clean.rs` `print_human_plan`：

```rust
if plan.notices.contains(&PlanNotice::GroupContainersInaccessible) {
    eprintln!("{GROUP_CONTAINERS_WARN}");
}
if plan.notices.contains(&PlanNotice::GroupContainersTruncated) {
    eprintln!("{GROUP_CONTAINERS_TRUNCATED_WARN}");
}
```

并补 import。

**说明：** stubs 的 `ContainersInaccessible` 在 human plan 里目前**没**打印 `CONTAINER_STUBS_WARN`（只走 coverage_with）；本期 Group Containers 按 spec 响亮提示 → **human plan 也打印**（与 orphan / system-services 同形）。

README：
- `514 条` → `515 条`
- 对比句：去掉「Group Containers 泛清理」作全家桶要点；可改为「受保护组容器缓存完整对齐 / 真 sudo …」

- [ ] **Step 4: Run tests**

```bash
cargo test -p vole-core --lib ops::coverage -- --nocapture
cargo test -p vole-core --lib -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/coverage.rs crates/vole-core/src/ops/mod.rs \
  crates/vole-cli/src/clean.rs README.md
git commit -m "feat: coverage and CLI warn for group-container-caches"
```

---

### Task 5: Apply 冒烟 + 保护层零回归 + 发版 1.7.0

**Files:**
- Modify: `Cargo.toml`（workspace version `1.7.0`）
- Create: `docs/releases/v1.7.0.md`
- Create: `docs/findings/2026-08-group-container-caches.md`（含闸口矩阵引用 design §6）
- Modify: `Formula/vole.rb`（version + sha256，发 tag/release 资产后更新）

- [ ] **Step 1: Apply 冒烟测（可放 `plan.rs` 旁或 `apply_plan` 测）**

用既有 apply 测试基础设施：对非 protected fixture 叶节点走 `apply_plan`，断言废纸篓删除成功；**确认** `apply_plan.rs` **无** `GROUP_CONTAINER_CACHE_RULE_ID` 分支。

最小测法：若已有 `apply_plan` 单测 fixture，追加一条 `group-container-caches` rule_id 的叶路径；否则用集成级：

```bash
# 手工冒烟（实现者本地）
VOLE_TEST_HOME=/tmp/vole-gcc-smoke ...
```

自动化：在 `crates/vole-core/src/ops/apply_plan.rs` 既有测试模块仿一条 orphan/cache 删除测（读邻近测例，复用 scratch + trash mock）。**不要**为冒烟引入 carve-out。

同时跑：

```bash
cargo test -p vole-core --lib protection:: -- --nocapture
cargo test -p vole-core --lib safety -- --nocapture
```

Expected: 保护层零变化，全绿。

- [ ] **Step 2: bump + docs**

`Cargo.toml`：`version = "1.7.0"`（workspace）。

`docs/releases/v1.7.0.md` 要点：
- Group Containers logs/caches 叶清理（Mole 同形）
- 零保护层改动；受保护 id / bundle 命名日志文件仍 skip
- TeamID 归一化比 Mole 严
- 规模上限 + FDA warn
- 规则 515；后续 1.8.0 才考虑保护层放行

`docs/findings/2026-08-group-container-caches.md`：贴 design §6 探针矩阵 +「为什么不做保护层扩展」。

- [ ] **Step 3: Full verify**

```bash
cargo fmt --all -- --check
cargo test -p vole-core --lib
cargo test -p vole-cli --lib
cargo clippy -p vole-core -p vole-cli -- -D warnings
./scripts/check-license.sh
./scripts/check-dep-direction.sh
./scripts/check-protocol-doc.sh
```

Expected: 全绿。

- [ ] **Step 4: Commit + Formula 占位说明**

```bash
git add Cargo.toml docs/releases/v1.7.0.md docs/findings/2026-08-group-container-caches.md
git commit -m "chore: release 1.7.0 group-container-caches"
```

Formula sha256：等 git tag / GitHub release 资产生成后更新（与 v1.6.0 流程同形）；**本 task 可先改 version 字符串，sha 留 release 脚本步**。

- [ ] **Step 5: PR gate**

- 开 PR（目标 `main`）
- **必跑 security-review**（扫描面 / fail-closed / 上限 / 确认保护层零 diff）
- CI 全绿后再合

---

## Self-Review

1. **Spec coverage**
   - §4 产品行为 / rule 落位 → Task 3 TOML + Task 4 README
   - §5 流水线（Apple / Safari fail-closed / TeamID / 候选子树 / 上限 / 隐藏文件）→ Task 1–2
   - §6 零保护层 + 矩阵 → Task 5 findings + 禁止清单在 Global Constraints
   - §7 普通 apply → Task 5（明确无 carve-out）
   - §8 FDA degrade + truncated notice → Task 3–4
   - §9 coverage → Task 4
   - §10–12 非目标/测安/验收 → 测试清单 + Task 5 PR gate

2. **Placeholder scan**：无 TBD；`CustomSelectResult.truncated` 扩展已写明。

3. **Type consistency**：`GroupCacheSelectResult` / `GroupCacheScanError` / `GROUP_CONTAINER_CACHE_RULE_ID` / notice 变体命名跨 task 一致。
