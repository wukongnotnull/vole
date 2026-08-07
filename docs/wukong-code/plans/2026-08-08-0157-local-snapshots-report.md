# Local Snapshots Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `vole status`（及可选 `analyze`）上报 Time Machine 本地快照数量与 review 提示，对齐 Mole `clean_local_snapshots` 报告面；**禁止删除**。

**Architecture:** 独立 `vole-core::localsnapshots` 模块 + 可注入 `LocalSnapshotDeps`；采集结果挂到 `StatusSnapshot.local_snapshots`（optional）；`StatusCollector` 缓存探测以免 Fast 刷新打爆 `tmutil`。不触碰 `tmbackup` / clean apply。

**Tech Stack:** Rust / macOS / `tmutil listlocalsnapshots` / `vole-sys::timeouts::SHORT_QUERY` / 既有 status TUI

## Global Constraints

- 版本：**1.29.0**（workspace）；规则数不变（仍约 **533**）
- 仅 `listlocalsnapshots`；禁止 `deletelocalsnapshots` / 任何 clean 删除接线
- fail-closed：`tmutil` 失败/超时 → Quiet，不 invent count
- 不改 W2、不改 `tm-failed-backups`
- coverage「仍未移植」只删「本地快照报告」，保留「桌面 SMAppService / 特权助手」
- 中文进度；任务级 commit
- 工作树：`/Users/wukong/Documents/vole/.worktrees/feat-local-snapshots-report`（branch `feat/local-snapshots-report`）

---

## File Structure

| 路径 | 职责 |
|---|---|
| `crates/vole-proto/src/status.rs` | `LocalSnapshotsInfo` + `StatusSnapshot.local_snapshots` |
| `crates/vole-core/src/localsnapshots/mod.rs` | 探测逻辑、deps、文案、单测 |
| `crates/vole-core/src/lib.rs` | `pub mod localsnapshots` |
| `crates/vole-core/src/status/collector.rs` | 调用探测并写入快照（缓存） |
| `crates/vole-sys/src/macos/status.rs` | **可不改**（字段在 core 层填 Default 后 overlay） |
| `crates/vole-cli/src/tui/status_view.rs` | 渲染 tip 行 |
| `crates/vole-cli/src/tui/analyze_view.rs` + `main.rs` | 可选 tip |
| `crates/vole-core/src/ops/coverage.rs` | 覆盖文案 + 测试断言 |
| `Cargo.toml` / `docs/releases/v1.29.0.md` / `README.md` | 发版 |

---

### Task 1: Proto — `LocalSnapshotsInfo`

**Files:**
- Modify: `crates/vole-proto/src/status.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Produces: `LocalSnapshotsInfo { count: Option<u64>, message: String }`；`StatusSnapshot.local_snapshots: Option<LocalSnapshotsInfo>`

- [ ] **Step 1: Write the failing assertion in proto test**

在 `status_snapshot_roundtrip_snake_case` 旁新增：

```rust
#[test]
fn local_snapshots_omitted_when_none() {
    let snap = StatusSnapshot::default();
    let v = serde_json::to_value(&snap).unwrap();
    assert!(v.get("local_snapshots").is_none());
}

#[test]
fn local_snapshots_present_serializes() {
    let snap = StatusSnapshot {
        local_snapshots: Some(LocalSnapshotsInfo {
            count: Some(2),
            message: "Time Machine local snapshots · 2 (review: tmutil listlocalsnapshots /)".into(),
        }),
        ..Default::default()
    };
    let v = serde_json::to_value(&snap).unwrap();
    assert_eq!(v["local_snapshots"]["count"], 2);
    assert!(v["local_snapshots"]["message"].as_str().unwrap().contains("review"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vole-proto local_snapshots -- --nocapture`
Expected: FAIL（类型/字段不存在）

- [ ] **Step 3: Minimal types**

在 `StatusSnapshot` 末尾字段后追加：

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub local_snapshots: Option<LocalSnapshotsInfo>,
```

并新增：

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocalSnapshotsInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    pub message: String,
}
```

- [ ] **Step 4: Run tests pass**

Run: `cargo test -p vole-proto -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-proto/src/status.rs
git commit -m "feat(proto): optional local_snapshots on StatusSnapshot"
```

---

### Task 2: Core module — probe + Fake deps（TDD）

**Files:**
- Create: `crates/vole-core/src/localsnapshots/mod.rs`
- Modify: `crates/vole-core/src/lib.rs`（`pub mod localsnapshots;`）

**Interfaces:**
- Consumes: `vole_sys::{timeouts::SHORT_QUERY, MacSysCommand}` + `SysCommand`（仅 Live）
- Produces:
  - `enum LocalSnapshotReport { Quiet, Present { count: u64 }, SkippedBusy, SkippedUnknown }`
  - `trait LocalSnapshotDeps { fn tmutil_exists(&self) -> bool; fn auto_backup_configured(&self) -> bool; fn running_state(&self) -> LocalTmRunningState; fn list_localsnapshots(&self) -> Result<String, ()>; }`
  - `fn probe_local_snapshots(deps: &dyn LocalSnapshotDeps) -> LocalSnapshotReport`
  - `fn count_tm_snapshot_lines(stdout: &str) -> u64`
  - `fn to_info(report: LocalSnapshotReport) -> Option<LocalSnapshotsInfo>`
  - `fn format_message(report: &LocalSnapshotReport) -> Option<String>`
  - `struct LiveLocalSnapshotDeps;`

`LocalTmRunningState`：`Running | Idle | Unknown`（本模块自有，**不**从 `tmbackup` import，避免耦合）。

门控顺序写死（design §5）：
1. `!tmutil_exists` → Quiet
2. `!auto_backup_configured` → Quiet
3. Unknown → SkippedUnknown
4. Running → SkippedBusy
5. `list_localsnapshots` `Err` → Quiet
6. regex count；0 → Quiet；>0 → Present

Regex：`com\.apple\.TimeMachine\.[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}`

文案：
- Present: `Time Machine local snapshots · {N} (review: tmutil listlocalsnapshots /)`
- SkippedUnknown: `Snapshot check · skipped (Time Machine status unknown)`
- SkippedBusy: `Snapshot check · skipped (backup in progress)`

`to_info`：Quiet → None；其余 → Some(LocalSnapshotsInfo { count: Present 时 Some(n)，否则 None, message })

- [ ] **Step 1: Failing unit tests**（写在新模块 `#[cfg(test)]`）

```rust
#[test]
fn no_tmutil_is_quiet() { /* Fake { tmutil:false, .. } */ }

#[test]
fn auto_backup_bad_is_quiet() { /* auto:false */ }

#[test]
fn running_is_skipped_busy() { /* running Running */ }

#[test]
fn unknown_is_skipped_unknown() {}

#[test]
fn list_err_is_quiet_fail_closed() { /* list: Err(()) */ }

#[test]
fn parses_mole_shaped_lines() {
    let out = "Snapshots for volume group containing disk /:\ncom.apple.TimeMachine.2026-08-01-120000.local\ncom.apple.TimeMachine.2026-08-02-130000.local\n";
    assert_eq!(count_tm_snapshot_lines(out), 2);
    let r = probe_local_snapshots(&Fake { list: Ok(out.into()), ..idle_ok() });
    assert!(matches!(r, LocalSnapshotReport::Present { count: 2 }));
}

#[test]
fn zero_matches_quiet() {
    let r = probe_local_snapshots(&Fake { list: Ok("Snapshots for volume group...\n".into()), ..idle_ok() });
    assert!(matches!(r, LocalSnapshotReport::Quiet));
}

#[test]
fn module_source_forbids_delete_subcommand() {
    let src = include_str!("mod.rs");
    assert!(!src.contains("deletelocalsnapshots"));
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p vole-core localsnapshots -- --nocapture`
Expected: FAIL（module 不存在）

- [ ] **Step 3: Implement module + Live deps**

`LiveLocalSnapshotDeps::list_localsnapshots`：

```rust
use vole_sys::macos::syscommand::MacSysCommand; // 若 macos 不对外，用 vole_sys 已导出的 API
```

检查 `vole_sys` 公开面：若 `MacSysCommand` 未 re-export，用 `Command` + 自写短轮询，或 `vole_sys` 已有公开 runner。**优先** `vole_sys` 能用的 timeout 路径；否则：

```rust
fn list_localsnapshots(&self) -> Result<String, ()> {
    let cmd = MacSysCommand; // from crate:: or vole_sys
    let out = cmd.run(&["tmutil", "listlocalsnapshots", "/"], SHORT_QUERY).map_err(|_| ())?;
    if !out.status.success() { return Err(()); }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
```

`auto_backup_configured` / `tmutil_exists` / `running_state` 逻辑对齐 `LiveTmDeps`（复制，**不**改 `tmbackup`）。

- [ ] **Step 4: Tests PASS**

Run: `cargo test -p vole-core localsnapshots -- --nocapture`

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/localsnapshots crates/vole-core/src/lib.rs
git commit -m "feat(core): probe local Time Machine snapshots (report-only)"
```

---

### Task 3: Wire status collector + TUI（+ optional analyze）

**Files:**
- Modify: `crates/vole-core/src/status/collector.rs`
- Modify: `crates/vole-cli/src/tui/status_view.rs`
- Modify: `crates/vole-cli/src/tui/analyze_view.rs`（可选 tip 参数）
- Modify: `crates/vole-cli/src/main.rs`（analyze 一次探测）

**Interfaces:**
- Consumes: `probe_local_snapshots` / `to_info` / `LiveLocalSnapshotDeps`
- Produces: `StatusSnapshot.local_snapshots` 已填充；TUI 显示 message

- [ ] **Step 1: Collector cache**

```rust
pub struct StatusCollector {
    backend: MacStatusCollector,
    local_snapshots: Option<LocalSnapshotsInfo>,
    local_snapshots_ready: bool,
}

// in collect():
if mode == CollectionMode::Full || !self.local_snapshots_ready {
    let report = crate::localsnapshots::probe_local_snapshots(
        &crate::localsnapshots::LiveLocalSnapshotDeps,
    );
    self.local_snapshots = crate::localsnapshots::to_info(report);
    self.local_snapshots_ready = true;
}
snap.local_snapshots = self.local_snapshots.clone();
```

注：`MacStatusCollector::collect_snapshot` 仍 Default 该字段；core overlay。

- [ ] **Step 2: status_view tip**

在 header 与 CPU 之间或 Disks 下方加一行：若 `snap.local_snapshots.as_ref()` 有值，渲染 `message`。可用 `Constraint::Length(1)` 或并入 header 第二行。

- [ ] **Step 3: analyze optional**

`cmd_analyze_tui` 启动时：

```rust
let snap_tip = vole_core::localsnapshots::to_info(
    vole_core::localsnapshots::probe_local_snapshots(
        &vole_core::localsnapshots::LiveLocalSnapshotDeps,
    ),
);
let tip = snap_tip.as_ref().map(|i| i.message.clone());
```

`render_analyze(..., tip: Option<&str>)`：有 tip 时 header 附加 ` — {tip}`（截断过长可仅 Present 短文案）。

- [ ] **Step 4: Build + focused tests**

Run: `cargo test -p vole-core localsnapshots status::collector -- --nocapture`
Run: `cargo check -p vole-cli`

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/status/collector.rs crates/vole-cli/src/tui/status_view.rs crates/vole-cli/src/tui/analyze_view.rs crates/vole-cli/src/main.rs
git commit -m "feat(status): surface local snapshot tip on status and analyze"
```

---

### Task 4: Coverage + 发版 1.29.0

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs`（note + tests）
- Modify: `Cargo.toml` workspace version → `1.29.0`
- Create: `docs/releases/v1.29.0.md`
- Modify: `README.md` 成熟度一行

文案变更（精确）：

- 「已落地」列表追加：`本地快照报告（status/analyze · 仅 list）、`
- 「仍未移植：本地快照报告、桌面 SMAppService / 特权助手。」→「仍未移植：桌面 SMAppService / 特权助手。」

测试：
- `unported.contains("快照")` 改为 **断言不包含**「快照」/`本地快照`
- **仍**断言 `unported.contains("SMAppService")`
- 断言 note 含「本地快照报告」

`v1.29.0.md`：

```markdown
# v1.29.0

## 新增

- 本地快照报告：`tmutil listlocalsnapshots /` → 数量 + review 提示
  - 挂载：`vole status`（TUI/JSON）；可选 `analyze` 提示行
  - 仅报告；不删除本地快照

## 仍未移植

- 桌面 SMAppService / 特权助手

## 规则

533（不变）
```

README：`1.28.0`…余项快照报告 → `1.29.0`：本地快照报告；余项：桌面 Helper

- [ ] **Step 1: 改 coverage 测试（先红）** 再改 note
- [ ] **Step 2:** `cargo test -p vole-core coverage -- --nocapture`
- [ ] **Step 3:** bump version + release + README
- [ ] **Step 4: Commit**

```bash
git add crates/vole-core/src/ops/coverage.rs Cargo.toml docs/releases/v1.29.0.md README.md
git commit -m "chore(release): 1.29.0 local snapshots report coverage"
```

---

### Task 5: Verify + PR + merge

- [ ] **Step 1: Verification**

```bash
cargo fmt --all -- --check
cargo test -p vole-proto -p vole-core -- --nocapture
cargo clippy -p vole-proto -p vole-core -p vole-cli -- -D warnings
```

（本机 macOS 可加 `cargo test -p vole-cli`）

- [ ] **Step 2: Push + PR**

```bash
git push -u origin HEAD
gh pr create --title "feat: local snapshots report (W1, 1.29.0)" --body "..."
```

- [ ] **Step 3: CI 绿后 merge**

```bash
gh pr merge <N> --merge --delete-branch
```

禁止 squash。不主动开 security-review（无删除敏感路径）；若审 diff 触及 `apply_plan`/delete 再开。

---

## Self-Review

1. Spec coverage：报告、门控、fail-closed、status/analyze、禁删、coverage、1.29.0 → Tasks 1–4  
2. Placeholder scan：无 TBD  
3. Type consistency：`LocalSnapshotsInfo` / `LocalSnapshotReport` / `to_info` 贯穿 T2–T3  
