# Orphan FDA Loud Hint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `orphaned-app-data` 在 Library 不可读 / 安装扫描失败时不再静默空结果，而是同时通过 Skipped 事件、`Plan.notices`、人读 stderr 与当次 `coverage_note` 追加发出信号。

**Architecture:** `select_custom` 返回 `CustomSelectResult { paths, degrade }`；plan 在 degrade 时 emit `SkipReason::TccDenied` 并写入 `PlanNotice::OrphanLibraryInaccessible`；CLI 组装当次 coverage 警告并在 human plan 的 stderr 再打一行。零 `schema_version` / 零新 `SkipReason`。

**Tech Stack:** Rust，`vole-core` / `vole-cli` / `vole-proto`（只读复用）。

**Design:** [`docs/wukong-code/specs/2026-08-05-1919-orphan-fda-loud-hint-design.md`](../specs/2026-08-05-1919-orphan-fda-loud-hint-design.md)

## Global Constraints

- 不 bump `schema_version`；不新增 `SkipReason`；`LibraryInaccessible` → `TccDenied`。
- 警告文案固定（spec §4.3.1）；json 追加与 stderr 共用同一句。
- 其它 custom handler 行为不变（`degrade: None`）。
- 静态 `coverage_note(enabled)` **不**永久加入 FDA；仅 degraded 当次追加。
- Human 输出顺序：plan 表 → coverage → 警告行。
- 每个 Task 至少一次 commit；本计划可不升版本号（随 1.4.1 发版）。
- TDD：先红后绿。

---

## File Structure

```
crates/vole-core/src/rules/custom_handlers.rs  # CustomSelectResult / CustomDegrade；改 select_custom
crates/vole-core/src/rules/mod.rs              # re-export
crates/vole-core/src/ops/plan.rs               # Plan.notices；Custom 分支 degrade 处理
crates/vole-core/src/ops/mod.rs                # re-export PlanNotice（若需要）
crates/vole-core/src/ops/proto_plan.rs         # 构造 Plan 时 notices: vec![]
crates/vole-cli/src/clean.rs                   # 组装 coverage + human stderr
docs/findings/2026-08-orphan-fda-loud-hint.md  # 短验收（Task 3）
```

常量文案（CLI 或 core 其一，推荐 CLI 旁 `const`，core 只产 notice）：

```text
注意：orphaned-app-data 已跳过（无法读取 ~/Library/Caches 或安装扫描失败）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。
```

---

### Task 1: `CustomSelectResult` + orphan degrade

**Files:**
- Modify: `crates/vole-core/src/rules/custom_handlers.rs`
- Modify: `crates/vole-core/src/rules/mod.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct CustomSelectResult {
      pub paths: Vec<PathBuf>,
      pub degrade: Option<CustomDegrade>,
  }
  pub enum CustomDegrade {
      LibraryInaccessible,
  }
  pub fn select_custom(...) -> CustomSelectResult
  ```
- Consumes: 既有 `select_orphaned_paths` → `Result`

- [ ] **Step 1: Write failing tests**

在 `custom_handlers.rs` 或相邻 test module：

```rust
#[test]
fn orphaned_degrades_when_library_inaccessible() {
    let home = tempfile::tempdir().unwrap();
    // 无 Library/Caches
    let deps = FakeOrphanDeps::default();
    let rule = /* minimal Rule with orphaned handler */;
    let got = select_custom("orphaned_app_data", &[], home.path(), &rule, &deps);
    assert!(got.paths.is_empty());
    assert_eq!(got.degrade, Some(CustomDegrade::LibraryInaccessible));
}

#[test]
fn other_handler_has_no_degrade() {
    let got = select_custom("jetbrains_toolbox_old_versions", &[], Path::new("/tmp"), &rule, &FakeOrphanDeps::default());
    assert!(got.degrade.is_none());
}
```

- [ ] **Step 2: Run — expect FAIL**（签名仍返回 `Vec`）

Run: `cargo test -p vole-core select_custom --lib`（或测试名）

- [ ] **Step 3: Implement**

```rust
pub fn select_custom(...) -> CustomSelectResult {
    match handler {
        "orphaned_app_data" => orphaned_app_data(...),
        other => CustomSelectResult {
            paths: /* 原逻辑 Vec */,
            degrade: None,
        },
    }
}

fn orphaned_app_data(...) -> CustomSelectResult {
    match select_orphaned_paths(...) {
        Ok(paths) => CustomSelectResult { paths, degrade: None },
        Err(OrphanScanError::LibraryInaccessible) => CustomSelectResult {
            paths: vec![],
            degrade: Some(CustomDegrade::LibraryInaccessible),
        },
    }
}
```

把原 `match` 各分支包进 `CustomSelectResult { paths: ..., degrade: None }`。

- [ ] **Step 4: 临时修好 `plan.rs` 编译**（取 `.paths`），完整 degrade 逻辑放 Task 2。

- [ ] **Step 5: Run — expect PASS**（本 Task 测试）

- [ ] **Step 6: Commit**

```bash
git add crates/vole-core/src/rules/
git commit -m "feat(rules): CustomSelectResult with orphan degrade"
```

---

### Task 2: `Plan.notices` + emit `TccDenied`

**Files:**
- Modify: `crates/vole-core/src/ops/plan.rs`
- Modify: `crates/vole-core/src/ops/proto_plan.rs`（及一切 `Plan {` 构造：`notices: vec![]`）
- Modify: `crates/vole-core/src/ops/mod.rs`（导出 `PlanNotice`）

**Interfaces:**
- Produces: `PlanNotice::OrphanLibraryInaccessible`；degraded 时 `StreamEvent::Skipped { rule_id, reason: TccDenied }`
- Consumes: `CustomSelectResult`

- [ ] **Step 1: Write failing plan test**

```rust
#[test]
fn plan_orphaned_emits_tcc_denied_and_notice_when_degraded() {
    let _guard = test_env::lock();
    let home = scratch("orphan-fda");
    // 无 Library/Caches
    std::env::set_var("HOME", &home);
    let events = Arc::new(Mutex::new(Vec::new()));
    let orch = Orchestrator::new(...).with_orphan_deps(Arc::new(FakeOrphanDeps {
        scan_error: true, // 或仅缺 Caches
        ..Default::default()
    }));
    // attach event capture via Orchestrator event_tx
    let plan = orch.build_plan(&[orphaned_rule(...)], &AppProtection::new(), &[]).unwrap();
    assert!(plan.entries.iter().all(|e| e.rule_id != "orphaned-app-data")
        || plan.entries.is_empty());
    assert!(plan.notices.contains(&PlanNotice::OrphanLibraryInaccessible));
    // assert events contain Skipped { orphaned-app-data, TccDenied }
}
```

（参照同文件既有 orphan 测试如何构造 `Orchestrator` + events；缺 Caches 目录即可触发 FDA 探测失败。）

- [ ] **Step 2: Run — expect FAIL**

- [ ] **Step 3: Implement**

```rust
pub enum PlanNotice {
    OrphanLibraryInaccessible,
}

// build_plan_with:
let result = select_custom(...);
if let Some(CustomDegrade::LibraryInaccessible) = result.degrade {
    self.emit(StreamEvent::Skipped {
        rule_id: rule.id.clone(),
        reason: SkipReason::TccDenied,
    });
    notices.push(PlanNotice::OrphanLibraryInaccessible);
}
for path in result.paths { /* 既有循环 */ }

Ok(Plan { generated_at, ttl, entries, notices })
```

所有 `Plan { ... }` 字面量补 `notices: vec![]` 或真实值。

- [ ] **Step 4: Run**

Run: `cargo test -p vole-core --lib plan_orphaned`
Expected: PASS（含既有 orphan 测）

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(clean): emit TccDenied and PlanNotice on orphan degrade"
```

---

### Task 3: CLI 组装 coverage + human stderr + findings

**Files:**
- Modify: `crates/vole-cli/src/clean.rs`
- Create: `docs/findings/2026-08-orphan-fda-loud-hint.md`
- Modify: design status → 已实现（实现完成后）

**Interfaces:**
- Consumes: `plan.notices`
- Produces: 当次 `coverage_note` 追加；human stderr 警告行

- [ ] **Step 1: 在 `clean.rs` 加常量与 helper**

```rust
const ORPHAN_LIBRARY_WARN: &str = "注意：orphaned-app-data 已跳过（无法读取 ~/Library/Caches 或安装扫描失败）。若为权限问题，请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中允许当前终端或 Vole 后重试。";

fn coverage_with_orphan_notices(base: &str, plan: &Plan) -> String {
    if plan.notices.contains(&PlanNotice::OrphanLibraryInaccessible) {
        format!("{base}\n{ORPHAN_LIBRARY_WARN}")
    } else {
        base.to_string()
    }
}
```

- [ ] **Step 2: `run_plan` 使用组装后的 note**

```rust
let note = coverage_with_orphan_notices(&coverage_note(enabled), &plan);
proto.coverage_note = Some(note.clone());
write_plan_output(...)
```

- [ ] **Step 3: `print_human_plan`**

```rust
fn print_human_plan(plan: &Plan, coverage: &str) {
    // 既有表 + eprintln coverage
    if plan.notices.contains(&PlanNotice::OrphanLibraryInaccessible) {
        eprintln!("{ORPHAN_LIBRARY_WARN}");
    }
}
```

注意：若 `coverage` 已含警告句，human 路径会打两次——**避免双重**：要么 coverage 追加只给 json，human 单独 eprintln；要么 coverage 含警告且 human **不再**重复。

**写死选法（推荐）：**

- `coverage_with_orphan_notices` 用于 **json / plan-out / json-stream Done report**
- human：`print_human_plan` 打印**原始** `coverage_note(enabled)`，然后 **额外** `eprintln!(ORPHAN_LIBRARY_WARN)`  
→ 人读不会在 coverage 大段里埋警告两次，json 仍能在 note 末尾看到。

即 `run_plan`：

```rust
let base = coverage_note(enabled);
let note_for_proto = coverage_with_orphan_notices(&base, &plan);
// stream Done 用 note_for_proto
// write_plan_output：json 用 note_for_proto；human 传 base + plan（内部打 WARN）
```

- [ ] **Step 4: 单测或集成**

优先在 `vole-core` 测 notice；CLI 若无可测点，findings 记录手工：`--json` note 含关键字；human stderr 含关键字。

至少加 `vole-core` 测：`coverage_with_orphan_notices` 若放在 core 可单测；若仅 CLI，则 findings + 手工。

**推荐**：把 `ORPHAN_LIBRARY_WARN` 与 `coverage_with_orphan_notices` 放在 `vole-core::ops::coverage` 旁，便于单测：

```rust
#[test]
fn appends_warn_only_when_notice_present() { ... }
```

- [ ] **Step 5: Run**

Run: `cargo test -p vole-core --lib`
Run: `cargo clippy -p vole-core -p vole-cli --all-targets -- -D warnings`
Run: `cargo fmt --all -- --check`

- [ ] **Step 6: findings + commit**

```bash
git commit -m "feat(cli): loud orphan library-inaccessible hint on plan"
```

---

## Self-Review

1. Spec §4.2/4.3.1 三条通道 → Tasks 2–3 覆盖。  
2. 无 TBD；双重 stderr 风险已用「json 追加 / human 分打」写死。  
3. `Plan.notices` 与 `CustomSelectResult` 命名前后任务一致。

---

## Execution Handoff

Plan complete: `docs/wukong-code/plans/2026-08-05-1923-orphan-fda-loud-hint.md`.

按仓库惯例默认 **Inline Execution**。
