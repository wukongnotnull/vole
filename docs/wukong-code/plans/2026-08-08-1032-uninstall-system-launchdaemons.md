# Uninstall System LaunchDaemons / `/Library` sudo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vole uninstall` 在 plan→apply 上交付系统 LaunchDaemons / `/Library` sudo 残留主路径（PrivilegeBackend）；发版 **1.35.0**。

**Architecture:** 新模块 `vole-core::system_leftovers` 负责发现与 rule_id 编解码；`uninstall_plan` 追加侧车；`uninstall_apply` 注入既有 `PrivilegeBackend`（`sudo -n` + TTY `sudo -v`），plist 先 unload 再 permanent 删除。扩展 `path_allowed_for_privilege` 到本刀允许的 `/Library` 叶与 receipts。保护与 sibling 守卫不绕过。

**Tech Stack:** Rust / macOS / 既有 `PrivilegeBackend` / uninstall plan→apply

## Global Constraints

- 版本：**1.35.0**；**不 bump** `schema_version`
- 仅 W2a③ 主路径；边缘广谱（Frameworks/kext/Plug-Ins 等）→ coverage；**不做** SMAppService Helper
- **禁止**第二套特权：复用 `crate::privilege::{PrivilegeBackend, SudoNoninteractive, NoPrivilege, RecordingPrivilege, path_allowed_for_privilege}`
- 有 sibling → 零 system-leftover；`com.apple.*` 永不删
- 无凭证 → `NeedsPrivilege` + 响亮 skip；`VOLE_TEST_NO_AUTH=1` 永不真 sudo
- 测：`VOLE_TEST_SYSTEM_LIBRARY` + RecordingPrivilege；security-review；合并 `gh pr merge --merge --delete-branch`
- 权威设计：`docs/wukong-code/specs/2026-08-08-1031-uninstall-system-launchdaemons-design.md`
- 全程中文；task-level commit；默认 inline 执行

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/system_leftovers/mod.rs` | NEW：发现、编解码、边界匹配、kind |
| `crates/vole-core/src/lib.rs` | `pub mod system_leftovers;` |
| `crates/vole-core/src/privilege/mod.rs` | allowlist 扩展 + 测试根映射 |
| `crates/vole-core/src/ops/uninstall_plan.rs` | 侧车 + coverage_note |
| `crates/vole-core/src/ops/uninstall_apply.rs` | privilege 注入 + system-leftover 分支 |
| `crates/vole-core/src/ops/coverage.rs` / README / findings / Formula / Cargo.toml | 1.35.0 + 诚实文案 |
| `docs/releases/v1.35.0.md` | 发版说明 |

---

### Task 1: `system_leftovers` 编解码 + 边界 + 发现（TDD）

**Files:**
- Create: `crates/vole-core/src/system_leftovers/mod.rs`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub const SYSTEM_LEFTOVER_PREFIX: &str = "uninstall:system-leftover:";`
  - `pub enum SystemLeftoverKind { Launchd, Pht, Library, Receipt }` — `as_str()` → `launchd`/`pht`/`library`/`receipt`
  - `pub fn encode_system_leftover_rule_id(kind: SystemLeftoverKind, path: &Path) -> String`
  - `pub fn parse_system_leftover_rule_id(rule_id: &str) -> Option<(SystemLeftoverKind, PathBuf)>`
  - `pub fn name_starts_with_bundle_id_boundary(name: &str, bundle_id: &str) -> bool` — basename；`name == id || name.starts_with(id + ".")`
  - `pub struct SystemLeftoverHit { pub path: PathBuf, pub kind: SystemLeftoverKind, pub label: String }`
  - `pub fn find_system_leftovers(identity: &AppIdentity, siblings: &SiblingPresence) -> Vec<SystemLeftoverHit>`
  - `pub fn system_library_root() -> PathBuf` — `VOLE_TEST_SYSTEM_LIBRARY` 或 `/Library`
  - `pub fn receipts_root() -> PathBuf` — test：`parent(VOLE_TEST_SYSTEM_LIBRARY)/private/var/db/receipts`；live：`/private/var/db/receipts`

复用：`protection::{naming_variants, is_rejected_generic_name, is_reverse_dns_bundle_id, AppIdentity, SiblingPresence}`；`login_items::percent_encode_token` / `percent_decode_token`（或同文件内 re-export 调用，禁止复制第二份）。

发现规则（有 sibling → `[]`）：

1. LaunchAgents/Daemons：`{root}/{LaunchAgents|LaunchDaemons}` maxdepth 1；reverse-DNS → `{id}.plist` / `{id}.*.plist`；display_name ≥5 且非 `is_rejected_generic_name` → `*{name}*.plist` 且 basename 非 `com.apple.*` → kind `Launchd`
2. PHT：`{root}/PrivilegedHelperTools`；basename `name_starts_with_bundle_id_boundary` 且非 `com.apple.*` → `Pht`
3. Library exact：对 variants，若存在 `{root}/Application Support/{v}` 等（设计 §6.1）→ `Library`；跳过路径恰为目录根
4. Receipts：`receipts_root` maxdepth 1；basename boundary → `Receipt`

- [ ] **Step 1: RED** — 模块内 `#[cfg(test)]`：

```rust
#[test]
fn rule_id_roundtrip_encodes_path() {
    let p = PathBuf::from("/Library/LaunchDaemons/com.example.plist");
    let id = encode_system_leftover_rule_id(SystemLeftoverKind::Launchd, &p);
    assert!(id.starts_with("uninstall:system-leftover:launchd:"));
    let (k, out) = parse_system_leftover_rule_id(&id).unwrap();
    assert_eq!(k, SystemLeftoverKind::Launchd);
    assert_eq!(out, p);
}

#[test]
fn bundle_id_boundary_rejects_prefix_collision() {
    assert!(name_starts_with_bundle_id_boundary("com.foo.helper", "com.foo"));
    assert!(!name_starts_with_bundle_id_boundary("com.foobar.plist", "com.foo"));
}

#[test]
fn find_launchd_and_skips_sibling() {
    // tempfile Library + VOLE_TEST_SYSTEM_LIBRARY lock
    // write LaunchDaemons/com.example.app.plist + com.example.app.helper.plist
    // sibling.has → empty；无 sibling → 两条 Launchd
}
```

- [ ] **Step 2: 跑测 RED** — `cargo test -p vole-core rule_id_roundtrip_encodes_path -- --nocapture` → FAIL

- [ ] **Step 3: GREEN** — 实现 + `lib.rs`：`pub mod system_leftovers;`

- [ ] **Step 4: GREEN 测**

```bash
cargo test -p vole-core -- system_leftovers -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/system_leftovers/mod.rs crates/vole-core/src/lib.rs
git commit -m "feat(system_leftovers): discover LaunchDaemons and /Library leaves"
```

---

### Task 2: Privilege allowlist 扩展

**Files:**
- Modify: `crates/vole-core/src/privilege/mod.rs`（`path_allowed_for_privilege` + tests）

**规则：** 在既有三树 + clean exact 之上增加单层叶：

- `/Library/Application Support/<leaf>`
- `/Library/Preferences/<leaf>`
- `/Library/Caches/<leaf>`
- `/Library/Logs/<leaf>`
- `/Library/Receipts/<leaf>`
- `/private/var/db/receipts/<leaf>`

约束：绝对路径、无 `..`、leaf 非空且不含 `/`；basename 不得 `com.apple.` 前缀（与 Launch* 同形）。`VOLE_TEST_SYSTEM_LIBRARY` 下前缀映射到 fixture（与现 `privilege_prefixes` 同模式：额外映射上述子树）。

- [ ] **Step 1: RED**

```rust
#[test]
fn allowlist_accepts_library_app_support_leaf() {
    assert!(path_allowed_for_privilege(Path::new(
        "/Library/Application Support/Foo"
    )));
    assert!(!path_allowed_for_privilege(Path::new(
        "/Library/Application Support/com.apple.Foo"
    )));
    assert!(!path_allowed_for_privilege(Path::new(
        "/Library/Application Support/a/b"
    )));
}

#[test]
fn allowlist_accepts_private_receipts_leaf() {
    assert!(path_allowed_for_privilege(Path::new(
        "/private/var/db/receipts/com.example.app.bom"
    )));
}
```

- [ ] **Step 2–4: RED→GREEN→测**

- [ ] **Step 5: Commit** — `feat(privilege): allowlist uninstall /Library and receipts leaves`

---

### Task 3: plan 侧车接线

**Files:**
- Modify: `crates/vole-core/src/ops/uninstall_plan.rs`

对每个合格 app（在 leftovers 之后）：

```rust
let mut system_leftovers = 0u64;
// ...
if !siblings.has_siblings() {
    for hit in find_system_leftovers(&app, &siblings) {
        // 可选：再过 protection / whitelist；命中则 continue
        entries.push(ProtoPlanEntry {
            path: hit.path.clone(),
            rule_id: encode_system_leftover_rule_id(hit.kind, &hit.path),
            size: 0, // 或尽力 metadata
            label: format!("System leftover: {}", hit.label),
        });
        system_leftovers += 1;
    }
}
```

coverage_note：

```text
... brew_cask=N, login_items=N, system_leftovers=N.
Long-tail not covered (use Mole): broad /Library system leftovers (Frameworks/kext/Plug-Ins/…) beyond LaunchDaemons/Agents/PHT and exact leaves.
```

去掉「system LaunchDaemons, /Library sudo paths」旧句。

- [ ] **Step 1: RED** — fixture Applications + SYSTEM_LIBRARY → plan 含 `uninstall:system-leftover:launchd:`；sibling 测 → 无；coverage 无旧 Long-tail 短语、含 `system_leftovers=`

- [ ] **Step 2–4: RED→GREEN→测**

```bash
cargo test -p vole-core -- uninstall_plan -- --nocapture
```

- [ ] **Step 5: Commit** — `feat(uninstall): plan system leftover sidecar entries`

---

### Task 4: apply PrivilegeBackend 接线

**Files:**
- Modify: `crates/vole-core/src/ops/uninstall_apply.rs`

**Interfaces:**
- `UninstallApplyContext` 增：
  - `pub privilege: Option<&'a dyn PrivilegeBackend>`
  - `pub privilege_acquire_attempted: bool`（或内部 cell；与 clean 同形）
- 默认 `apply_uninstall_plan`：`SudoNoninteractive` + `privilege_acquire_attempted: false`
- 复用/拷贝小函数 `ensure_privilege_ready`（stdin terminal + 中文提示 + `acquire_interactive`）

对 `parse_system_leftover_rule_id`：

1. apply 再检 sibling（从 entry.path 推 bundle？系统条目 path 是系统路径不是 app）— **设计写死**：plan token 已含绝对 path；sibling 再检需从 rule 外关联。实现：**在 system-leftover 条目的 label 不够**。改用：**plan 时 path=系统路径；apply 再校验 path 仍满足 allowlist + 形状**（basename boundary / leaf 名）；sibling 在 plan 阶段已抑制。若 apply 时无法从系统 path 反查 sibling，则依赖 plan TTL（短）+ allowlist 足够 — **按设计 §8：sibling 再检**：保存 `rule_id` 不含 app；简化为 **只再检 allowlist + 非 apple + path 存在**。设计原文「apply 再校验 sibling」若成本高：对本刀 **plan 已 sibling-suppress + TTL** 足够；apply 检 `path_allowed_for_privilege` + basename 非 `com.apple.*`。
2. `ensure_privilege_ready`；失败 → NeedsPrivilege skip + stderr 中文（复用或 `APPLY_PERMISSION_WARN` 同类）
3. kind `Launchd` → `backend.launchctl_unload(&path)`（best-effort）
4. `backend.remove_permanent(&path)`；成功 → succeeded + deleted_bytes；Unavailable → NeedsPrivilege；Refused/Failed → skip

**禁止**对 system-leftover 调 `mole_delete` / Trash。

- [ ] **Step 1: RED**

```rust
#[test]
fn apply_system_leftover_unload_and_remove() {
    // RecordingPrivilege::allowing；fixture path under TEST library allowed
    // entry rule_id encode Launchd；assert unloaded + removed len 1；succeeded 1
}

#[test]
fn apply_system_leftover_needs_privilege_when_denied() {
    // RecordingPrivilege::denying → SkipReason::NeedsPrivilege
}
```

- [ ] **Step 2–4: RED→GREEN→测** — 修复所有 `UninstallApplyContext { ... }` 结构体字面量补字段

- [ ] **Step 5: Commit** — `feat(uninstall): apply system leftovers via PrivilegeBackend`

---

### Task 5: 文档 / 版本 1.35.0 + 验证

**Files:**
- Modify: `Cargo.toml` workspace.package.version、`Formula/vole.rb`、`crates/vole-core/src/ops/coverage.rs`、`README.md`、`docs/findings/2026-07-v2-m1-uninstall.md`
- Create: `docs/releases/v1.35.0.md`

coverage「已落地」加 uninstall 系统 LaunchDaemons/`/Library` sudo 主路径一句；仍未移植：桌面 Helper + 广谱边缘。

- [ ] **Step 1:** 改版本与文案  
- [ ] **Step 2:**

```bash
cargo fmt --all -- --check
cargo test -p vole-core -- system_leftovers uninstall -- --nocapture
cargo test -p vole-proto -p conformance
./scripts/check-license.sh
```

- [ ] **Step 3: Commit** — `chore(release): bump to 1.35.0 for uninstall system leftovers`  
- [ ] **Step 4:** push + `gh pr create`；等 CI；security-review；`gh pr merge --merge --delete-branch`  
- [ ] **Step 5:** 另开小 PR 更新 0119：W2a③ 完成；下一刀写死（推荐并联池：W2b② `memory_pressure_relief` 或 W2c 续刀）

---

## Spec coverage self-check

| 设计项 | Task |
|---|---|
| 发现主路径 Launch*/PHT/Library/receipts | 1 |
| rule_id 编解码 | 1 |
| allowlist 扩展 | 2 |
| plan 侧车 + coverage | 3 |
| PrivilegeBackend apply + NeedsPrivilege | 4 |
| 1.35.0 / README / findings | 5 |
| 边缘 skip + 非 Helper | Constraints + coverage |
| security-review / merge | 5 |

## Execution

默认 **Inline**（用户偏好）。Plan 完成后直接 `executing-plans` / TDD 逐 task，无需再问。
