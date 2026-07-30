# Guard `not_running` 子集 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 让 `guards.not_running` 在 plan/apply 生效（精确进程名 / fail-closed），兑现既有 AI/Codex 声明，并落地 Firefox + 云盘缓存静态 guard 子集。

**Architecture:** 新增 `ProcessProbe` trait；默认 `PgrepProcessProbe` 经 `SysCommand` 跑 `pgrep -x`；`Orchestrator` / `ApplyPlanContext` 注入 probe；plan 在规则循环开头跳过，apply 按 `rule_id` 回查规则再检。不改 Plan JSON schema。

**Tech Stack:** Rust 1.97.1、`vole-core` / `vole-sys` / `vole-cli`、现有 clean fixtures。

**Design:** `docs/wukong-code/specs/2026-07-30-guard-not-running-design.md`

## Global Constraints

- 许可证：GPL-3.0-only。
- 平台：仅 macOS；`pgrep` 为系统命令。
- `unsafe` 只在 `vole-sys`；`vole-core` 保持 `#![forbid(unsafe_code)]`。
- 匹配：仅精确名（`pgrep -x`）；Unknown → 视为应跳过。
- 协议：使用既有 `SkipReason::AppRunning`（`app_running`）；不 bump `schema_version`。
- 不做 cmdline / FCP generated / Simulator 复合探测。
- 提交：每个 Task 至少一次 commit；TDD 红→绿。

---

## File Structure

```
crates/vole-core/src/rules/
  process_guard.rs     # NEW: ProcessState, ProcessProbe, PgrepProcessProbe, any_guard_blocks
  mod.rs               # pub use process_guard
crates/vole-core/src/ops/
  mod.rs               # Orchestrator 持有 Arc<dyn ProcessProbe>
  plan.rs              # 规则级 not_running 检查
  apply_plan.rs        # apply 回查规则 + probe
crates/vole-cli/src/clean.rs  # apply 传入 rules + 默认 probe
data/rules/
  user-devtools.toml   # firefox-cache guards；新增 dropbox / google-drive / onedrive
tests/fixtures/clean/
  batch_guard_*.json   # 静态路径选中（默认 Idle probe）
docs/findings/
  2026-07-guard-not-running.md  # 短记
```

---

### Task 1: `ProcessProbe` + 单元测试（TDD）

**Files:**
- Create: `crates/vole-core/src/rules/process_guard.rs`
- Modify: `crates/vole-core/src/rules/mod.rs`
- Test: same module `#[cfg(test)]`

**Interfaces:**
- Produces:
  - `pub enum ProcessState { Running, Idle, Unknown }`
  - `pub trait ProcessProbe: Send + Sync { fn exact_name_running(&self, name: &str) -> ProcessState; }`
  - `pub fn should_skip_for_not_running(probe: &dyn ProcessProbe, names: &[String]) -> bool`
    - 空列表 → `false`
    - 忽略空字符串
    - 任一 `Running` 或 `Unknown` → `true`
  - `pub struct FakeProcessProbe { pub running: std::collections::HashSet<String>, pub unknown: std::collections::HashSet<String> }`（`cfg(test)` 可 `pub` 或 `pub(crate)` 供 ops 测试；生产代码也可用 `#[cfg(test)]` 模块内 Fake，但 apply/plan 测试在 ops —— 将 Fake 放在 `process_guard` 且 `pub` 仅在 test 时不够；**直接 `pub struct FakeProcessProbe`** 供测试与注入）

- [x] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn empty_names_never_skips() {
        let probe = FakeProcessProbe::default();
        assert!(!should_skip_for_not_running(&probe, &[]));
    }

    #[test]
    fn skips_when_any_exact_name_running() {
        let probe = FakeProcessProbe {
            running: HashSet::from(["Firefox".into()]),
            unknown: HashSet::new(),
        };
        assert!(should_skip_for_not_running(
            &probe,
            &["Chrome".into(), "Firefox".into()]
        ));
    }

    #[test]
    fn idle_when_none_running() {
        let probe = FakeProcessProbe::default();
        assert!(!should_skip_for_not_running(
            &probe,
            &["Firefox".into()]
        ));
    }

    #[test]
    fn unknown_fail_closed_skips() {
        let probe = FakeProcessProbe {
            running: HashSet::new(),
            unknown: HashSet::from(["Mail".into()]),
        };
        assert!(should_skip_for_not_running(&probe, &["Mail".into()]));
    }
}
```

- [x] **Step 2: Run tests — expect FAIL**（module / symbols missing）

```bash
cargo test -p vole-core should_skip_for_not_running -- --nocapture
```

- [x] **Step 3: Minimal implementation**

```rust
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Idle,
    Unknown,
}

pub trait ProcessProbe: Send + Sync {
    fn exact_name_running(&self, name: &str) -> ProcessState;
}

#[derive(Debug, Default, Clone)]
pub struct FakeProcessProbe {
    pub running: HashSet<String>,
    pub unknown: HashSet<String>,
}

impl ProcessProbe for FakeProcessProbe {
    fn exact_name_running(&self, name: &str) -> ProcessState {
        if self.running.contains(name) {
            ProcessState::Running
        } else if self.unknown.contains(name) {
            ProcessState::Unknown
        } else {
            ProcessState::Idle
        }
    }
}

pub fn should_skip_for_not_running(probe: &dyn ProcessProbe, names: &[String]) -> bool {
    names
        .iter()
        .filter(|n| !n.is_empty())
        .any(|n| !matches!(probe.exact_name_running(n), ProcessState::Idle))
}
```

Export from `rules/mod.rs`.

- [x] **Step 4: Tests PASS**

```bash
cargo test -p vole-core process_guard -- --nocapture
```

- [x] **Step 5: Commit**

```bash
git add crates/vole-core/src/rules/process_guard.rs crates/vole-core/src/rules/mod.rs
git commit -m "$(cat <<'EOF'
feat(rules): add ProcessProbe and not_running skip helper

EOF
)"
```

---

### Task 2: `PgrepProcessProbe`（真实探测）

**Files:**
- Modify: `crates/vole-core/src/rules/process_guard.rs`

**Interfaces:**
- Consumes: `vole_sys::macos::MacSysCommand`, `vole_sys::SysCommand`, timeout ~2s
- Produces: `pub struct PgrepProcessProbe;` + `impl ProcessProbe`
  - `pgrep -x <name>`：exit 0 → Running；exit 1 → Idle；其它 / Timeout / spawn 失败 → Unknown

- [x] **Step 1: Write failing test**（用假 SysCommand 较难；对本任务用文档化行为 + 可选集成测）

优先：抽出内部函数便于测：

```rust
pub(crate) fn state_from_pgrep_status(code: Option<i32>, timed_out: bool) -> ProcessState {
    if timed_out {
        return ProcessState::Unknown;
    }
    match code {
        Some(0) => ProcessState::Running,
        Some(1) => ProcessState::Idle,
        _ => ProcessState::Unknown,
    }
}
```

测试上述映射；`PgrepProcessProbe` 实现调用 `MacSysCommand.run(&["pgrep", "-x", name], Duration::from_secs(2))`。

- [x] **Step 2: Run mapping tests — FAIL then implement — PASS**

```bash
cargo test -p vole-core state_from_pgrep_status -- --nocapture
```

- [x] **Step 3: Commit**

```bash
git commit -am "$(cat <<'EOF'
feat(rules): implement PgrepProcessProbe via pgrep -x

EOF
)"
```

---

### Task 3: Plan 管线接入

**Files:**
- Modify: `crates/vole-core/src/ops/mod.rs`
- Modify: `crates/vole-core/src/ops/plan.rs`
- Modify: `crates/vole-core/src/clean_fixture.rs`（若 `Orchestrator::new` 签名变）

**Interfaces:**
- `Orchestrator` 增加字段 `process_probe: Arc<dyn ProcessProbe>`
- `Orchestrator::new(cancel, events)` → 默认 `Arc::new(PgrepProcessProbe)`
- `Orchestrator::with_process_probe(cancel, events, probe: Arc<dyn ProcessProbe>)` 供测试
- 在 `build_plan_with` 的 `for rule in rules` 内、`disabled` 检查之后、`resolve_strategy` 之前：

```rust
if should_skip_for_not_running(self.process_probe.as_ref(), &rule.guards.not_running) {
    self.emit(StreamEvent::Skipped {
        rule_id: rule.id.clone(),
        reason: SkipReason::AppRunning,
    });
    continue;
}
```

- [x] **Step 1: Failing ops test**

在 `plan.rs` `#[cfg(test)]`：

```rust
#[test]
fn plan_skips_rule_when_not_running_guard_hits() {
    let probe = Arc::new(FakeProcessProbe {
        running: HashSet::from(["claude".into()]),
        unknown: HashSet::new(),
    });
    let orch = Orchestrator::with_process_probe(CancelToken::new(), None, probe);
    // Rule: id claude-code-old-versions-like, paths under VOLE_TEST_HOME, not_running=["claude"]
    // Materialize one candidate dir; assert plan.entries.is_empty()
    // With events channel, assert Skipped{AppRunning}
}
```

（用 `test_env` + 临时 HOME，仿现有 plan 测试。）

- [x] **Step 2: Run — FAIL（无跳过）**

```bash
cargo test -p vole-core plan_skips_rule_when_not_running_guard_hits -- --nocapture
```

- [x] **Step 3: Wire Orchestrator + plan check — PASS**

- [x] **Step 4: 加对称测试** `plan_selects_when_process_idle`

- [x] **Step 5: Commit**

```bash
git commit -am "$(cat <<'EOF'
feat(ops): skip clean rules when not_running guard matches

EOF
)"
```

---

### Task 4: Apply 再检

**Files:**
- Modify: `crates/vole-core/src/ops/apply_plan.rs`
- Modify: `crates/vole-cli/src/clean.rs`

**Interfaces:**
- `ApplyPlanContext` 增加：
  - `rules: &'a [Rule]`
  - `process_probe: &'a dyn ProcessProbe`
- `apply_proto_plan(...)` 签名增加 `rules` + 使用 `&PgrepProcessProbe`（或 `Arc`）
- 对每个 entry，在 delete 前：

```rust
if let Some(rule) = ctx.rules.iter().find(|r| r.id == entry.rule_id) {
    if should_skip_for_not_running(ctx.process_probe, &rule.guards.not_running) {
        // emit Skipped AppRunning, count skipped, continue
    }
}
```

若 `rule_id` 找不到：不因 guard 跳过（仍走路径校验；与「规则已卸载」一致）。

- [x] **Step 1: Failing apply test** — FakeTrash + running probe + rule with not_running → 不调用 trash，skipped++

- [x] **Step 2: Implement + CLI 传入 `load_rules` 结果与 `PgrepProcessProbe`

- [x] **Step 3: PASS + Commit**

```bash
git commit -am "$(cat <<'EOF'
feat(ops): re-check not_running guards on clean --apply

EOF
)"
```

---

### Task 5: 规则数据子集

**Files:**
- Modify: `data/rules/user-devtools.toml`
- Create: `tests/fixtures/clean/batch_guard_firefox_cache_selects_child.json`
- Create: `tests/fixtures/clean/batch_guard_dropbox_cache_selects_child.json`
- Create: `tests/fixtures/clean/batch_guard_google_drive_cache_selects_child.json`
- Create: `tests/fixtures/clean/batch_guard_onedrive_cache_selects_child.json`

**Rules:**

1. `firefox-cache` — 追加：

```toml
[rule.guards]
not_running = ["Firefox"]
```

2. 新增（标签对齐 mole）：

```toml
[[rule]]
id = "dropbox-cache"
category = "user-devtools"
label = "Dropbox cache"
platform = ["macos"]
paths = [
  "~/Library/Caches/com.dropbox.*",
  "~/Library/Caches/com.getdropbox.dropbox",
]
last_verified = "2026-07"

[rule.strategy]
kind = "all"

[rule.guards]
not_running = ["Dropbox"]

[[rule]]
id = "google-drive-cache"
...
paths = ["~/Library/Caches/com.google.GoogleDrive"]
[rule.guards]
not_running = ["Google Drive"]

[[rule]]
id = "onedrive-cache"
...
paths = ["~/Library/Caches/com.microsoft.OneDrive"]
[rule.guards]
not_running = ["OneDrive"]
```

Fixtures：mkdir 子路径 + `expect_selected`（默认 pgrep 在测试机上通常 Idle；若 CI 碰巧跑着同名进程，单测 Fake 已覆盖守卫逻辑 —— fixture 只验路径选择）。

- [x] **Step 1: 写 TOML + fixtures**

- [x] **Step 2:**

```bash
cargo test -p vole-core verify_clean_fixtures -- --nocapture
cargo test -p vole-core -- --nocapture
```

Expected: PASS

- [x] **Step 3: Commit**

```bash
git commit -am "$(cat <<'EOF'
feat(rules): add Firefox/Dropbox/Drive/OneDrive not_running guards

EOF
)"
```

---

### Task 6: 文档与收口

**Files:**
- Create: `docs/findings/2026-07-guard-not-running.md`
- Modify: `crates/vole-core/src/ops/coverage.rs`（可选：coverage_note 去掉或弱化「需进程检测的 guard」一句，改为「部分 guard 已落地；generated/cmdline 仍未移植」）
- Modify: `README.md` 仅当规则计数变化时更新总数（470 + 3 = **473**）

- [x] **Step 1: findings 短记**（引擎行为、子集列表、非目标）

- [x] **Step 2: 规则计数**

```bash
rg -c '^\[\[rule\]\]' data/rules/*.toml
```

更新 README「470」→ 实际总数。

- [x] **Step 3:**

```bash
cargo fmt --all -- --check
cargo clippy -p vole-core -p vole-cli --all-targets -- -D warnings
cargo test -p vole-core
```

- [x] **Step 4: Commit**

```bash
git commit -am "$(cat <<'EOF'
docs: record guard not_running subset landing

EOF
)"
```

- [x] **Step 5: 勾选本计划全部 checkbox 为 `[x]` 并 commit**

---

## Self-Review

1. **Spec coverage:** 引擎 / 精确匹配 / fail-closed / plan+apply / 既有声明 / Firefox+云盘 / 非目标 — 均有 Task。
2. **Placeholders:** 无 TBD。
3. **Types:** `ProcessProbe` / `should_skip_for_not_running` / `FakeProcessProbe` / `PgrepProcessProbe` 前后一致。

## Execution Handoff

Plan complete and saved to `docs/wukong-code/plans/2026-07-30-guard-not-running.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — executing-plans in this session with checkpoints  

Which approach?
