# Optimize memory_pressure_relief Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（inline preferred）or wukong-code:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Mole W2b② `memory_pressure_relief` 纳入 `vole optimize` 主路径（`in_m3: true`），经既有 `PrivilegeBackend` + `sudo -n purge` 释放 inactive memory；低压 noop；无凭证 fail-closed + 响亮提示；发 **1.36.0**。

**Architecture:** Catalog 翻转 → plan 产出 sentinel → apply 时探测内存压力（warning|critical）才调用 `PrivilegeBackend::purge_inactive_memory`（`sudo -n purge`）；复用 W2b① 的 privilege / `ensure_privilege_ready` 接线（仅高压时提权探测）。

**Tech Stack:** Rust workspace（vole-core / vole-cli）、既有 `PrivilegeBackend`、`SkipReason::NeedsPrivilege`、`APPLY_PERMISSION_WARN`。

## Global Constraints

- 版本：**1.36.0**（MINOR；若并行轨已占用则顺延）；不 bump `schema_version`
- 仅 `memory_pressure_relief`；**禁止** network_stack / disk_permissions / periodic / spotlight* / disk_verify / login_items / shared_file_list
- 不碰 uninstall / clean
- 禁止第二套特权体系；仅扩展 `PrivilegeBackend`
- `VOLE_TEST_NO_AUTH=1` / `test_no_auth()` 下永不真 sudo；`VOLE_TEST_MEMORY_PRESSURE=1|0` 强制高压/低压
- 全部命令在 worktree：`/Users/wukong/Documents/vole/.worktrees/feat-optimize-memory-pressure`
- 分支：`feat/optimize-memory-pressure`；合入用 **merge commit**（非 squash）

---

## File map

| File | Role |
|---|---|
| `crates/vole-core/src/optimize/catalog.rs` | `in_m3: true`；单测计数 15 |
| `crates/vole-core/src/privilege/mod.rs` + `sudo.rs` | `purge_inactive_memory` + Recording 计数 |
| `crates/vole-core/src/optimize/tasks/actions.rs` | `is_memory_pressure_high`；plan/apply |
| `crates/vole-core/src/optimize/tasks/mod.rs` + `optimize/mod.rs` | re-export |
| `crates/vole-core/src/ops/optimize_plan.rs` | 纳入 plan；更新 coverage 断言 |
| `crates/vole-core/src/ops/optimize_apply.rs` | 高压时 `ensure_privilege_ready`；单测 |
| `Cargo.toml` + crate versions + `docs/releases/v1.36.0.md` + README | 发版 |

---

### Task 1: Catalog `in_m3` 翻转

**Files:**
- Modify: `crates/vole-core/src/optimize/catalog.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Produces: `memory_pressure_relief.in_m3 == true`；主路径长度 **15**

- [ ] **Step 1: 写失败单测（改期望）**

```rust
#[test]
fn m3_main_path_flags() {
    let main: Vec<_> = optimize_catalog()
        .iter()
        .filter(|t| t.in_m3)
        .map(|t| t.id)
        .collect();
    assert!(main.contains(&"cache_refresh"));
    assert!(main.contains(&"system_maintenance"));
    assert!(main.contains(&"network_optimization"));
    assert!(main.contains(&"memory_pressure_relief"));
    assert!(!main.contains(&"network_stack_optimize"));
    assert!(!main.contains(&"spotlight_index_optimize"));
    assert_eq!(main.len(), 15);
}
```

- [ ] **Step 2: RED** — `cargo test -p vole-core catalog::tests::m3_main_path_flags -- --exact`

- [ ] **Step 3: 翻转** `memory_pressure_relief` 的 `in_m3: true`

- [ ] **Step 4: GREEN**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(optimize): enable memory_pressure_relief in main path"
```

---

### Task 2: `PrivilegeBackend::purge_inactive_memory`

**Files:**
- Modify: `crates/vole-core/src/privilege/mod.rs`（trait）
- Modify: `crates/vole-core/src/privilege/sudo.rs`
- Test: `privilege/mod.rs` tests

**Interfaces:**
- Produces:
```rust
fn purge_inactive_memory(&self) -> Result<(), PrivilegeError>;
```
  - `SudoNoninteractive`: `test_no_auth` → `Unavailable`；否则 `sudo -n purge` 成功才 `Ok`
  - `NoPrivilege`: 恒 `Unavailable`
  - `RecordingPrivilege`: probe false → `Unavailable`；否则计数 + `Ok`（不真执行）
  - 字段：`RecordingPrivilege::purge_memory_calls: Mutex<u32>`

- [ ] **Step 1: 失败单测** `recording_purge_memory_requires_probe` / `recording_purge_memory_counts_when_allowing`

- [ ] **Step 2: RED**

- [ ] **Step 3: 实现 trait + 三 backend**

- [ ] **Step 4: GREEN**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(privilege): add PrivilegeBackend::purge_inactive_memory via sudo -n"
```

---

### Task 3: plan sentinel + apply 压力门 + purge

**Files:**
- Modify: `crates/vole-core/src/optimize/tasks/actions.rs`
- Modify: `crates/vole-core/src/optimize/tasks/mod.rs`、`optimize/mod.rs`
- Modify: `crates/vole-core/src/ops/optimize_plan.rs`
- Modify: `crates/vole-core/src/ops/optimize_apply.rs`
- Test: actions / optimize_plan / optimize_apply

**Interfaces:**
- Produces: `plan_memory_pressure_relief(home) -> OptimizeCandidate`
- Produces: `is_memory_pressure_high() -> bool`（`VOLE_TEST_MEMORY_PRESSURE`：`1|true` → true；`0|false` → false；否则跑 `memory_pressure -Q`，匹配 `(?i)warning|critical`）
- `apply_optimize_action("memory_pressure_relief", …)`：非高压 → `Ok(())`；高压 → `purge_inactive_memory`，映射错误同 DNS
- `optimize_apply`：`needs_optimize_privilege(task_id)` 包含高压时的 memory_pressure；复用 `ensure_privilege_ready`
- plan：`allow("memory_pressure_relief")` 时 push sentinel
- 单测断言：coverage note **不含** "Memory Optimization"；仍含 Network Stack / Spotlight 等长尾

- [ ] **Step 1: 失败单测**（plan 含 sentinel；低压不调 purge；高压 NoPrivilege → NeedsPrivilege；高压 Recording → 1 次 purge）

- [ ] **Step 2: RED**

- [ ] **Step 3: 实现 plan/apply/接线**

- [ ] **Step 4: GREEN** — `cargo test -p vole-core optimize_ -- --nocapture` 相关；`VOLE_TEST_NO_AUTH=1`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(optimize): plan/apply memory_pressure_relief via PrivilegeBackend purge"
```

---

### Task 4: 版本 1.36.0 + release + PR

**Files:**
- Modify: workspace / crate `Cargo.toml` versions → `1.36.0`
- Create: `docs/releases/v1.36.0.md`
- Modify: `README.md` 版本或长尾提及（若有）
- Check: coverage / optimize 人读长尾列表若硬编码「Memory Optimization」则去掉

- [ ] **Step 1: bump + release 短记**

- [ ] **Step 2: 全量验证**

```bash
cargo fmt --all -- --check
cargo test -p vole-core -p vole-cli
# 或 macOS CI 等价
```

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: release 1.36.0 memory_pressure_relief"
```

- [ ] **Step 4: Push + `gh pr create`**（边界写清：不做 W2b③+）

- [ ] **Step 5: CI 绿后** `gh pr merge --merge --delete-branch`

- [ ] **Step 6: 小 PR 更新 `0119`**：W2b② 完成；下一刀写死（W2b③ 或等 W2c）

---

## Spec coverage self-check

1. catalog / plan / apply / PrivilegeBackend / 低压 noop / 高压 fail-closed 响亮 / 1.36.0 / 非目标 — 均有 task
2. 无第二套特权；与 W2b① 模式对称
3. 并行轨冲突时 rebase + 版本顺延
