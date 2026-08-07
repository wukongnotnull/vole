# Optimize system_maintenance + network_optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans (inline preferred) or wukong-code:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Mole W2b① `system_maintenance` / `network_optimization` 纳入 `vole optimize` 主路径（`in_m3: true`），经既有 `PrivilegeBackend` + `sudo -n` 刷新 DNS/mDNS，无凭证 fail-closed + 响亮提示；发 **1.31.0**。

**Architecture:** Catalog 翻转标志 → plan 产出 sentinel → apply 经 `PrivilegeBackend::flush_dns_cache`（`sudo -n dscacheutil -flushcache` + `sudo -n killall -HUP mDNSResponder`）执行，同 session 用 `dns_flushed` 去重；`OptimizeApplyContext` 注入与 clean 同构的 privilege / `sudo -v` 至多一次。

**Tech Stack:** Rust workspace（vole-core / vole-cli）、既有 `PrivilegeBackend`、`SkipReason::NeedsPrivilege`、`APPLY_PERMISSION_WARN`。

## Global Constraints

- 版本：**1.31.0**（MINOR）；不 bump `schema_version`
- 仅两 task；**禁止** memory_pressure 及更后项 / spotlight* / disk_verify / login_items / shared_file_list
- 不碰 uninstall / clean TM / status 快照
- 禁止第二套特权体系；仅扩展 `PrivilegeBackend`
- `VOLE_TEST_NO_AUTH=1` / `test_no_auth()` 下永不真 sudo
- 全部命令在 worktree：`/Users/wukong/Documents/vole/.worktrees/feat-optimize-system-network`
- 分支：`feat/optimize-system-network`；合入用 **merge commit**（非 squash）

---

## File map

| File | Role |
|---|---|
| `crates/vole-core/src/optimize/catalog.rs` | `in_m3: true` ×2；单测计数 14 |
| `crates/vole-core/src/privilege/mod.rs` + `sudo.rs` | `flush_dns_cache` trait 方法 + 实现 |
| `crates/vole-core/src/optimize/tasks/actions.rs` | plan sentinels；apply DNS/Spotlight；`NeedsPrivilege` |
| `crates/vole-core/src/optimize/tasks/mod.rs` + `optimize/mod.rs` | re-export |
| `crates/vole-core/src/ops/optimize_plan.rs` | 默认扫描纳入两 plan |
| `crates/vole-core/src/ops/optimize_apply.rs` | privilege 注入、dns_flushed、NeedsPrivilege skip |
| `crates/vole-cli/src/optimize.rs` | apply 注入 `SudoNoninteractive`；人读权限提示 |
| `crates/vole-core/src/ops/coverage.rs` | 可选：已落地补 DNS/optimize 一句 |
| `Cargo.toml` + crates versions + `docs/releases/v1.31.0.md` + README | 发版 |

---

### Task 1: Catalog `in_m3` 翻转

**Files:**
- Modify: `crates/vole-core/src/optimize/catalog.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Produces: `system_maintenance.in_m3 == true`，`network_optimization.in_m3 == true`；主路径长度 **14**

- [ ] **Step 1: 写失败单测（改期望）**

把 `m3_main_path_flags` 改为：

```rust
#[test]
fn m3_main_path_flags() {
    let main: Vec<_> = optimize_catalog()
        .iter()
        .filter(|t| t.in_m3)
        .map(|t| t.id)
        .collect();
    assert!(main.contains(&"cache_refresh"));
    assert!(main.contains(&"saved_state_cleanup"));
    assert!(main.contains(&"system_maintenance"));
    assert!(main.contains(&"network_optimization"));
    assert!(!main.contains(&"memory_pressure_relief"));
    assert!(!main.contains(&"spotlight_index_optimize"));
    assert_eq!(main.len(), 14);
}
```

- [ ] **Step 2: 跑测确认 RED**

Run: `cargo test -p vole-core catalog::tests::m3_main_path_flags -- --exact`
Expected: FAIL（仍 12 / 不含 system_maintenance）

- [ ] **Step 3: 翻转 catalog 两处 `in_m3: true`**

`system_maintenance` 与 `network_optimization` 的 `in_m3` 改为 `true`。注释可改为「主路径（含需 `sudo -n` 的 DNS）」。

- [ ] **Step 4: GREEN**

Run: `cargo test -p vole-core catalog::tests::m3_main_path_flags -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/optimize/catalog.rs
git commit -m "feat(optimize): enable system_maintenance and network_optimization in main path"
```

---

### Task 2: `PrivilegeBackend::flush_dns_cache`

**Files:**
- Modify: `crates/vole-core/src/privilege/mod.rs`（trait）
- Modify: `crates/vole-core/src/privilege/sudo.rs`（三种 backend）
- Test: `privilege/mod.rs` tests 或 `sudo.rs`

**Interfaces:**
- Consumes: 既有 `PrivilegeBackend`、`test_no_auth()`、`PrivilegeError`
- Produces:
```rust
fn flush_dns_cache(&self) -> Result<(), PrivilegeError>;
```
  - `SudoNoninteractive`: 若 `test_no_auth` → `Unavailable`；否则 `sudo -n dscacheutil -flushcache` 与 `sudo -n killall -HUP mDNSResponder` 皆成功才 `Ok(())`，否则 `CommandFailed` / 失败 status
  - `NoPrivilege`: 恒 `Err(Unavailable)`
  - `RecordingPrivilege`: 若 `probe` false → `Unavailable`；否则 push 计数/`flushed` 标志并 `Ok(())`（不真执行）

- [ ] **Step 1: 失败单测**

```rust
#[test]
fn recording_flush_dns_requires_probe() {
    let b = RecordingPrivilege::denying();
    assert!(matches!(
        b.flush_dns_cache(),
        Err(PrivilegeError::Unavailable)
    ));
    assert_eq!(*b.flush_dns_calls.lock().unwrap(), 0);
}

#[test]
fn recording_flush_dns_counts_when_allowing() {
    let b = RecordingPrivilege::allowing();
    b.flush_dns_cache().unwrap();
    assert_eq!(*b.flush_dns_calls.lock().unwrap(), 1);
}
```

先加字段 `flush_dns_calls: Mutex<u32>` 到 `RecordingPrivilege` 的构造（allowing/denying 初始化 0），trait 方法尚未实现 → 编译失败即 RED。

- [ ] **Step 2: 实现 trait + backends**

在 trait 增加：

```rust
fn flush_dns_cache(&self) -> Result<(), PrivilegeError>;
```

`SudoNoninteractive::flush_dns_cache`:

```rust
fn flush_dns_cache(&self) -> Result<(), PrivilegeError> {
    if test_no_auth() {
        return Err(PrivilegeError::Unavailable);
    }
    let flush = Command::new("sudo")
        .args(["-n", "dscacheutil", "-flushcache"])
        .status()
        .map_err(|e| PrivilegeError::CommandFailed(e.to_string()))?;
    if !flush.success() {
        return Err(PrivilegeError::CommandFailed(format!(
            "dscacheutil exit {flush}"
        )));
    }
    let hup = Command::new("sudo")
        .args(["-n", "killall", "-HUP", "mDNSResponder"])
        .status()
        .map_err(|e| PrivilegeError::CommandFailed(e.to_string()))?;
    if hup.success() {
        Ok(())
    } else {
        Err(PrivilegeError::CommandFailed(format!(
            "killall mDNSResponder exit {hup}"
        )))
    }
}
```

`NoPrivilege` → `Err(Unavailable)`。

`RecordingPrivilege`：增加 `flush_dns_calls: Mutex<u32>`；实现同上测试语义。更新 `allowing`/`denying` 与所有字面构造处。

- [ ] **Step 3: GREEN**

Run: `cargo test -p vole-core privilege::tests::recording_flush_dns -- --nocapture`
Expected: PASS（匹配上述两测名）

- [ ] **Step 4: Commit**

```bash
git add crates/vole-core/src/privilege/
git commit -m "feat(privilege): add PrivilegeBackend::flush_dns_cache via sudo -n"
```

---

### Task 3: Plan sentinels

**Files:**
- Modify: `crates/vole-core/src/optimize/tasks/actions.rs`
- Modify: `crates/vole-core/src/optimize/tasks/mod.rs`
- Modify: `crates/vole-core/src/optimize/mod.rs`
- Modify: `crates/vole-core/src/ops/optimize_plan.rs`
- Test: `actions.rs` / `optimize_plan.rs`

**Interfaces:**
- Produces:
```rust
pub fn plan_system_maintenance(home: &Path) -> OptimizeCandidate;
pub fn plan_network_optimization(home: &Path) -> OptimizeCandidate;
```
  labels 可用 `"DNS & Spotlight Check"` / `"Network Cache Refresh"`（对齐 catalog title）

- [ ] **Step 1: 失败单测**

In `optimize_plan.rs` tests，扩展或新增：

```rust
#[test]
fn build_plan_includes_system_and_network_sentinels() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let catalog = ProtectionCatalog::embedded();
    let protection = AppProtection::new();
    let plan = build_optimize_plan(
        &catalog,
        &protection,
        &OptimizePlanOptions {
            home,
            ttl_secs: 900,
            only_task: None,
        },
    )
    .unwrap();
    assert!(plan
        .entries
        .iter()
        .any(|e| e.rule_id == "optimize:action:system_maintenance"));
    assert!(plan
        .entries
        .iter()
        .any(|e| e.rule_id == "optimize:action:network_optimization"));
    let note = plan.coverage_note.unwrap();
    assert!(!note.contains("DNS & Spotlight Check"));
    assert!(!note.contains("Network Cache Refresh"));
    assert!(note.contains("Memory Optimization") || note.contains("memory"));
}
```

（标题来自 catalog `title` 字段进长尾 note。）

- [ ] **Step 2: RED**

Run: `cargo test -p vole-core ops::optimize_plan::tests::build_plan_includes_system_and_network_sentinels -- --exact`
Expected: FAIL（entries 缺 sentinel / note 仍含 title）

- [ ] **Step 3: 实现 plan 函数并联线**

```rust
pub fn plan_system_maintenance(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "system_maintenance", "DNS & Spotlight Check")
}

pub fn plan_network_optimization(home: &Path) -> OptimizeCandidate {
    action_sentinel(home, "network_optimization", "Network Cache Refresh")
}
```

在 `build_optimize_plan`：

```rust
if allow("system_maintenance") {
    candidates.push(plan_system_maintenance(opts.home));
}
if allow("network_optimization") {
    candidates.push(plan_network_optimization(opts.home));
}
```

re-export 从 `tasks/mod.rs` 与 `optimize/mod.rs`。

- [ ] **Step 4: GREEN** + 确认旧测 `build_plan_includes_old_saved_state_and_coverage` 仍过（长尾仍有 Memory Optimization）

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/optimize/ crates/vole-core/src/ops/optimize_plan.rs
git commit -m "feat(optimize): plan sentinels for system_maintenance and network_optimization"
```

---

### Task 4: Apply handlers + dns_flushed + NeedsPrivilege

**Files:**
- Modify: `crates/vole-core/src/optimize/tasks/actions.rs`
- Modify: `crates/vole-core/src/ops/optimize_apply.rs`
- Test: 两处

**Interfaces:**
- Consumes: `PrivilegeBackend::flush_dns_cache`，`SkipReason::NeedsPrivilege`
- Produces:
```rust
pub enum OptimizeActionError {
    Failed,
    Skipped,
    NeedsPrivilege,
}

pub fn apply_optimize_action(
    task_id: &str,
    path: &Path,
    privilege: Option<&dyn PrivilegeBackend>,
    dns_flushed: &mut bool,
) -> Result<(), OptimizeActionError>;
```

行为：
- `system_maintenance` / `network_optimization`：
  1. 若 `!*dns_flushed`：取 backend（`None`→视为 Unavailable）；先可不在此做 acquire（由 apply context 外层 ensure）；调用 `flush_dns_cache`；`Unavailable`/`Refused` → `NeedsPrivilege`；`CommandFailed` → `Failed`；`Ok` → 置 `*dns_flushed = true`
  2. 若已 flushed：跳过 flush，记成功
  3. `system_maintenance` 额外：`Command::new("mdutil").args(["-s", "/"])` 只读（失败不影响 succeeded——对齐 Mole 仅展示）
- 其它既有 action：忽略 privilege/dns_flushed，行为不变

`OptimizeApplyContext` 新增：
```rust
pub privilege: Option<&'a dyn PrivilegeBackend>,
pub privilege_acquire_attempted: bool,
pub dns_flushed: bool,
```

抽取与 clean 同构的 `ensure_privilege_ready`（可复制小函数到 `optimize_apply.rs`，避免跨模块大 refactor）：

```rust
fn ensure_privilege_ready(ctx: &mut OptimizeApplyContext<'_>, backend: &dyn PrivilegeBackend) -> bool {
    if backend.probe_noninteractive() {
        return true;
    }
    if ctx.privilege_acquire_attempted {
        return false;
    }
    ctx.privilege_acquire_attempted = true;
    if std::io::stdin().is_terminal() {
        let _ = writeln!(std::io::stderr(), "正在请求管理员权限以执行系统优化…");
    }
    backend.acquire_interactive() && backend.probe_noninteractive()
}
```

在 Action 分支：若 task 为两 DNS 任务且有 backend，先 `ensure_privilege_ready`；再 `apply_optimize_action(..., ctx.privilege, &mut ctx.dns_flushed)`。

Skip 映射：
```rust
Err(OptimizeActionError::NeedsPrivilege) => {
    skipped += 1;
    skip_tracker.record(SkipReason::NeedsPrivilege, &entry.rule_id);
}
Err(OptimizeActionError::Skipped) => { /* PathVanished 保持 */ }
```

- [ ] **Step 1: 失败单测**

```rust
#[test]
fn apply_dns_tasks_skip_without_privilege() {
    // build minimal ProtoPlan with two action entries
    // ctx.privilege = Some(&NoPrivilege)
    // apply → both NeedsPrivilege in skipped_by_reason; flush never counted
}

#[test]
fn apply_dns_tasks_flush_once_with_recording() {
    // privilege = RecordingPrivilege::allowing()
    // both entries → succeeded 2; flush_dns_calls == 1
}
```

- [ ] **Step 2: RED**（编译或断言失败）

- [ ] **Step 3: 实现上述接口与接线**；更新所有 `apply_optimize_action` 调用点

- [ ] **Step 4: GREEN**

Run: `cargo test -p vole-core ops::optimize_apply -- --nocapture` 与 `optimize::tasks::actions`

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/optimize/crates/vole-core/src/ops/optimize_apply.rs
# fix path if typo — use both dirs
git add crates/vole-core/src/optimize crates/vole-core/src/ops/optimize_apply.rs
git commit -m "feat(optimize): apply DNS flush via sudo -n with dedupe and NeedsPrivilege"
```

---

### Task 5: CLI 注入 `SudoNoninteractive` + 响亮提示

**Files:**
- Modify: `crates/vole-cli/src/optimize.rs`
- Modify: `crates/vole-core/src/ops/optimize_apply.rs`（`apply_optimize_plan` 注入 sudo，对齐 clean）

**Interfaces:**
- `apply_optimize_plan` 内创建 `SudoNoninteractive` 填入 context（与 `apply_proto_plan` 同模式），使 CLI 无需重复逻辑；测试可走 `apply_optimize_proto_plan` 注入 `NoPrivilege`/`Recording`

- [ ] **Step 1:** 人读 apply 路径：若 `report_has_permission_skips`，stderr/`print_human_report` 追加 `APPLY_PERMISSION_WARN`（对照 clean / 现有 JSON 分支已有 `coverage_with_apply_permission_hint`）

检查 `print_human_report`：无则补：

```rust
if report_has_permission_skips(report) {
    eprintln!("{APPLY_PERMISSION_WARN}");
}
```

- [ ] **Step 2:** 确认 `apply_optimize_plan` 注入 `SudoNoninteractive` + `privilege_acquire_attempted: false` + `dns_flushed: false`

- [ ] **Step 3:** `cargo test -p vole-core` 相关 + 若有 CLI 测则跑

- [ ] **Step 4: Commit**

```bash
git add crates/vole-cli/src/optimize.rs crates/vole-core/src/ops/optimize_apply.rs
git commit -m "feat(cli): wire sudo -n optimize apply and loud permission hint"
```

---

### Task 6: 版本 1.31.0 + coverage / release

**Files:**
- Modify: workspace `Cargo.toml` version `1.31.0`（及各 crate 若非 workspace inherit 的显式版本）
- Create: `docs/releases/v1.31.0.md`
- Modify: `crates/vole-core/src/ops/coverage.rs`（「已落地」可补「optimize DNS/mDNS（system_maintenance / network_optimization + sudo -n）」；长尾列表以 optimize plan note 为准）
- Modify: `README.md` optimize 一句（12→14 主路径；提及 sudo -n DNS）
- Test: coverage 测若断言「已落地」结构，保持通过

- [ ] **Step 1:** bump version 到 **1.31.0**

- [ ] **Step 2:** 写 `docs/releases/v1.31.0.md`：

```markdown
# v1.31.0

## 新增

- `vole optimize`：`system_maintenance` / `network_optimization` 进入主路径
  - plan sentinel；apply 经 `sudo -n` 刷新 DNS 并 HUP mDNSResponder
  - 同 session 只 flush 一次；无凭证 → NeedsPrivilege + 响亮提示
  - `system_maintenance` 另只读校验 Spotlight（`mdutil -s /`）

## 仍未移植（optimize 长尾）

- memory_pressure / network_stack / disk_permissions / periodic / spotlight* / disk_verify / login_items / shared_file_list
- 本地快照报告、桌面 SMAppService / 特权助手
```

- [ ] **Step 3:** 更新 coverage「已落地」与 README「12 项」→「14 项」

- [ ] **Step 4:** `cargo test -p vole-core ops::coverage`；`cargo fmt`；必要 clippy

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/*/Cargo.toml docs/releases/v1.31.0.md \
  crates/vole-core/src/ops/coverage.rs README.md
git commit -m "chore(release): bump 1.31.0 for optimize DNS/network main path"
```

---

### Task 7: PR + merge（merge commit）

- [ ] **Step 1:** `git push -u origin HEAD`

- [ ] **Step 2:** `gh pr create` 标题/正文含 W2b①、版本 1.31.0、边界（不做 memory_pressure+）

- [ ] **Step 3:** 等 CI 绿；`gh pr merge <N> --merge --delete-branch`

- [ ] **Step 4:** 回报：branch、PR URL、版本、测试、范围边界

---

## Self-review

1. **Spec coverage:** catalog / plan / apply / PrivilegeBackend / fail-closed 响亮 / 去重 / 1.31.0 / 非目标 — 均有 task  
2. **Placeholders:** 无 TBD  
3. **Types:** `flush_dns_cache` / `OptimizeActionError::NeedsPrivilege` / context 字段前后一致  
