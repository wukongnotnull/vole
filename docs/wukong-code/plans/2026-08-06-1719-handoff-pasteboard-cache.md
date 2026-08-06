# Handoff Pasteboard Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `vole clean` plan→apply 落地 Mole 同形的 Handoff pasteboard 叶清理（mtime>60min；零保护层改动；1.8.0）。

**Architecture:** 新建 `vole-core::handoff` 模块做扫描/重验/label；经 custom handler `handoff_pasteboard_cache` 接线；plan 走既有 `validate_path_for_deletion`（无豁免）；apply 仅在普通删除前做政策重验（根形状 + mtime），不旁路 protect。

**Tech Stack:** Rust / macOS / vole-core / tempfile + filetime 单测 / TOML 规则。

## Global Constraints

- 版本意图：**1.8.0**；规则数 **515 → 516**；**不 bump** `schema_version`
- `rule_id`：`handoff-pasteboard-cache`；handler：`handoff_pasteboard_cache`；`category`：`app-caches`
- 规则文件：**`data/rules/app-caches.toml`**（禁止写入 `zzz-orphaned.toml`）
- 根写死：`$HOME/Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard`
- mtime 阈值写死：**60 分钟**；整规则叶上限：**2000**
- **禁止**修改：`should_protect_path` / `protection.toml` / `is_explicit_clean_cache_path` / `skip_protection` / stubs 式 carve-out / plan 层 `validate_path_for_deletion` 豁免
- apply 政策重验失败 → `PathVanished` skip（对齐 orphan recheck）

---

## File Structure

| 文件 | 职责 |
|---|---|
| **Create** `crates/vole-core/src/handoff/mod.rs` | 常量、根路径拼接、`is_handoff_pasteboard_leaf_path`、`handoff_pasteboard_label` |
| **Create** `crates/vole-core/src/handoff/select.rs` | `select_handoff_pasteboard`、`recheck_handoff_pasteboard_entry`、degrade/truncated |
| **Modify** `crates/vole-core/src/lib.rs` | `pub mod handoff;` |
| **Modify** `crates/vole-core/src/rules/custom_handlers.rs` | degrade 变体 + handler 分派 |
| **Modify** `crates/vole-core/src/ops/plan.rs` | PlanNotice 两变体 + degrade/truncated/label |
| **Modify** `crates/vole-core/src/ops/apply_plan.rs` | 普通删除前政策重验（**不**旁路 protect） |
| **Modify** `crates/vole-core/src/ops/coverage.rs` + `ops/mod.rs` + `vole-cli/src/clean.rs` | WARN + coverage 文案 |
| **Modify** `data/rules/app-caches.toml` | 追加规则 |
| **Modify** `README.md`、`Cargo.toml`、releases/findings/Formula | 发版 |

---

### Task 1: `handoff` 模块骨架 + 形状/label + select/recheck

**Files:**
- Create: `crates/vole-core/src/handoff/mod.rs`
- Create: `crates/vole-core/src/handoff/select.rs`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub const HANDOFF_PASTEBOARD_RULE_ID: &str = "handoff-pasteboard-cache";`
  - `pub const HANDOFF_MTIME_MINUTES: u64 = 60;`
  - `pub const MAX_HANDOFF_LEAVES: usize = 2000;`
  - `pub fn handoff_pasteboard_root(home: &Path) -> PathBuf`
  - `pub fn is_handoff_pasteboard_leaf_path(path: &Path, home: &Path) -> bool`
  - `pub fn handoff_pasteboard_label(path: &Path) -> String`
  - `pub enum HandoffScanError { RootInaccessible }`
  - `pub struct HandoffSelectResult { paths: Vec<PathBuf>, truncated: bool }`
  - `pub fn select_handoff_pasteboard(home: &Path, now: SystemTime) -> Result<HandoffSelectResult, HandoffScanError>`
  - `pub fn recheck_handoff_pasteboard_entry(path: &Path, home: &Path, now: SystemTime) -> bool`

- [ ] **Step 1: Write failing tests**（`mod.rs` + `select.rs` 的 `#[cfg(test)]`）

```rust
// mod.rs tests
#[test]
fn leaf_path_gate_accepts_single_component() {
    let home = Path::new("/Users/t");
    assert!(is_handoff_pasteboard_leaf_path(
        &handoff_pasteboard_root(home).join("item1"),
        home
    ));
    assert!(!is_handoff_pasteboard_leaf_path(
        &handoff_pasteboard_root(home).join("a").join("b"),
        home
    ));
    assert!(!is_handoff_pasteboard_leaf_path(
        &home.join("Library/Group Containers/group.com.apple.coreservices.useractivityd/other"),
        home
    ));
    assert!(!is_handoff_pasteboard_leaf_path(home, home));
}

#[test]
fn label_uses_basename() {
    assert_eq!(
        handoff_pasteboard_label(Path::new("/Users/t/.../shared-pasteboard/abc")),
        "Handoff pasteboard: abc"
    );
}
```

```rust
// select.rs tests — 用 tempfile + filetime
#[test]
fn selects_only_older_than_60_minutes() { /* >60 入选；<60 否 */ }

#[test]
fn skips_symlink_leaf_and_symlink_root() { /* ... */ }

#[test]
fn missing_root_empty_unreadable_errors() { /* ... */ }

#[test]
fn cap_sets_truncated() { /* >2000 → truncated，paths.len()==2000 */ }

#[test]
fn recheck_rejects_fresh_mtime_and_outside_root() { /* ... */ }
```

- [ ] **Step 2: Run — expect FAIL**

`cargo test -p vole-core --lib handoff:: -- --nocapture`

- [ ] **Step 3: Implement**

`mod.rs`：

```rust
mod select;
pub use select::{
    recheck_handoff_pasteboard_entry, select_handoff_pasteboard, HandoffScanError,
    HandoffSelectResult,
};

pub const HANDOFF_PASTEBOARD_RULE_ID: &str = "handoff-pasteboard-cache";
pub const HANDOFF_MTIME_MINUTES: u64 = 60;
pub const MAX_HANDOFF_LEAVES: usize = 2000;

pub fn handoff_pasteboard_root(home: &Path) -> PathBuf {
    home.join("Library/Group Containers/group.com.apple.coreservices.useractivityd/shared-pasteboard")
}

pub fn is_handoff_pasteboard_leaf_path(path: &Path, home: &Path) -> bool {
    let root = handoff_pasteboard_root(home);
    let Ok(rel) = path.strip_prefix(&root) else { return false; };
    matches!(
        (rel.components().next(), rel.components().nth(1)),
        (Some(std::path::Component::Normal(_)), None)
    )
}

pub fn handoff_pasteboard_label(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
    format!("Handoff pasteboard: {name}")
}
```

`select.rs` 核心：

```rust
pub fn select_handoff_pasteboard(home: &Path, now: SystemTime) -> Result<HandoffSelectResult, HandoffScanError> {
    let root = handoff_pasteboard_root(home);
    if !root.exists() { return Ok(empty); }
    let meta = fs::symlink_metadata(&root).map_err(|_| HandoffScanError::RootInaccessible)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Ok(empty);
    }
    let rd = fs::read_dir(&root).map_err(|_| HandoffScanError::RootInaccessible)?;
    let mut out = Vec::new();
    let mut truncated = false;
    let min_age = Duration::from_secs(HANDOFF_MTIME_MINUTES * 60);
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(m) = fs::symlink_metadata(&path) else { continue; };
        if m.file_type().is_symlink() { continue; }
        let Ok(mtime) = m.modified() else { continue; };
        let Some(age) = now.duration_since(mtime).ok() else { continue; }; // clock skew → skip
        if age <= min_age { continue; }
        if out.len() >= MAX_HANDOFF_LEAVES {
            truncated = true;
            break;
        }
        out.push(path);
    }
    out.sort();
    Ok(HandoffSelectResult { paths: out, truncated })
}

pub fn recheck_handoff_pasteboard_entry(path: &Path, home: &Path, now: SystemTime) -> bool {
    if !is_handoff_pasteboard_leaf_path(path, home) { return false; }
    let Ok(m) = fs::symlink_metadata(path) else { return false; };
    if m.file_type().is_symlink() { return false; }
    let Ok(mtime) = m.modified() else { return false; };
    match now.duration_since(mtime) {
        Ok(age) => age > Duration::from_secs(HANDOFF_MTIME_MINUTES * 60),
        Err(_) => false,
    }
}
```

注意：`is_handoff_pasteboard_leaf_path` 里不要调用两次 `components()` 独立 iterator——用一次 collect 或 peek。

- [ ] **Step 4: Tests PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/handoff crates/vole-core/src/lib.rs
git commit -m "feat(handoff): select pasteboard leaves older than 60m"
```

---

### Task 2: Handler + PlanNotice + TOML + label

**Files:**
- Modify: `custom_handlers.rs`、`plan.rs`、`app-caches.toml`

- [ ] **Step 1: Failing tests**

Handler：有旧叶 → paths；根 0o000 → `HandoffPasteboardInaccessible`。

Plan：

```rust
fn handoff_rule() -> Rule { /* id handoff-pasteboard-cache, handler handoff_pasteboard_cache */ }

#[test]
fn plan_handoff_selects_old_leaf_with_label() { /* >60min → 1 entry, label */ }

#[test]
fn plan_handoff_degrades_when_root_unreadable() { /* notice + TccDenied */ }

#[test]
fn plan_handoff_skips_fresh_leaf() { /* <60min → empty */ }
```

- [ ] **Step 2: Run FAIL**

- [ ] **Step 3: Wire**

`CustomDegrade::HandoffPasteboardInaccessible`

```rust
"handoff_pasteboard_cache" => handoff_pasteboard_cache(home),

fn handoff_pasteboard_cache(home: &Path) -> CustomSelectResult {
    match crate::handoff::select_handoff_pasteboard(home, SystemTime::now()) {
        Ok(r) => CustomSelectResult { paths: r.paths, degrade: None, truncated: r.truncated },
        Err(HandoffScanError::RootInaccessible) => CustomSelectResult {
            paths: vec![], degrade: Some(CustomDegrade::HandoffPasteboardInaccessible), truncated: false,
        },
    }
}
```

`PlanNotice::{HandoffPasteboardInaccessible, HandoffPasteboardTruncated}` + plan 分支（同 GroupContainers）+ label：

```rust
} else if rule.id == crate::handoff::HANDOFF_PASTEBOARD_RULE_ID {
    crate::handoff::handoff_pasteboard_label(&path)
```

TOML 追加到 `app-caches.toml` 末尾。

**重要：plan 层不做 validate 豁免。**

- [ ] **Step 4: PASS** + `rules::load` 仍绿

- [ ] **Step 5: Commit**

`feat: wire handoff-pasteboard-cache handler and plan notices`

---

### Task 3: Apply 政策重验（非 carve-out）

**Files:**
- Modify: `crates/vole-core/src/ops/apply_plan.rs`

对齐 orphan 模式（在 `mole_delete` 前 recheck，**保留** `verify_plan_entry_for_apply`）：

```rust
if entry.rule_id == HANDOFF_PASTEBOARD_RULE_ID {
    let home = dirs_home();
    if !recheck_handoff_pasteboard_entry(&entry.path, &home, ctx.now) {
        // skip PathVanished — 然后 continue
    }
}
// 然后照常 verify_plan_entry_for_apply + mole_delete_verified
```

放在 system-services / stub 分支之后、`verify_plan_entry_for_apply` 之前。

- [ ] **Step 1: Tests**

```rust
#[test]
fn apply_handoff_old_leaf_trashes() { /* >60min fixture → succeeded 1 */ }

#[test]
fn apply_handoff_fresh_mtime_skips() { /* plan 时旧、apply 前 touch 新 → skip，文件仍在 */ }

#[test]
fn apply_handoff_outside_root_skips() { /* rule_id 挂 other 路径 → skip */ }
```

- [ ] **Step 2–4: FAIL → 实现 → PASS**

- [ ] **Step 5: Commit**

`feat(apply): recheck handoff pasteboard root and mtime before delete`

---

### Task 4: Coverage + CLI + README

**Files:** `coverage.rs`、`ops/mod.rs`、`clean.rs`、`README.md`

- WARN：`HANDOFF_PASTEBOARD_WARN`、`HANDOFF_PASTEBOARD_TRUNCATED_WARN`
- coverage_note：已落地句加入 Handoff pasteboard（mtime>60min）
- human plan 打印两则 warn
- README 515→516

- [ ] **Step 1: 先改失败断言** → FAIL → 实现 → PASS → Commit

`feat: coverage and CLI warn for handoff-pasteboard-cache`

---

### Task 5: 发版 1.8.0

- bump `Cargo.toml` → 1.8.0
- `docs/releases/v1.8.0.md`、`docs/findings/2026-08-handoff-pasteboard-cache.md`（贴探针矩阵）
- Formula version 占位 sha（发 tag 后回填）
- Full verify：

```bash
cargo fmt --all -- --check
cargo test -p vole-core --lib -- --skip status::collector
cargo clippy -p vole-core -p vole-cli -- -D warnings
./scripts/check-license.sh && ./scripts/check-dep-direction.sh && ./scripts/check-protocol-doc.sh
cargo test -p vole-core --lib protection::
```

- Commit：`chore: release 1.8.0 handoff-pasteboard-cache`
- PR + **security-review** 必过

---

## Self-Review

1. Spec §1–13 均有对应 task；零豁免写进 Global Constraints 与 Task 3。
2. 无 TBD；`recheck` 与 orphan 同威胁模型用语一致。
3. 常量/Notice/Degrade 命名跨 task 一致。
