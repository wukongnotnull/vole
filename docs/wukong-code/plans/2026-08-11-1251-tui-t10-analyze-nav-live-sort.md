# T10：analyze nav aliases + S live sort Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（本仓默认 inline）或 wukong-code:subagent-driven-development。Steps 用 checkbox（`- [ ]`）跟踪。

**Goal:** TTY `vole analyze` 接线导航别名（←→/h/b/l）与扫描中 `S` live sort（continuous / freeze-on-move），含根子项渐进进度；发版 **2.14.0**。

**Architecture:** `scan_directory_with_progress` 每完成一根子项回调；TUI 经 channel 合并 Child 并按 `LiveSortMode` 重排；Done 整表替换为最终 `AnalyzeOutput`。JSON 路径仍用阻塞 `analyze_directory`。无磁盘 cache。

**Tech Stack:** Rust 1.97.1、ratatui、crossterm、`std::sync::mpsc`。

**Design:** [`../specs/2026-08-11-1250-tui-t10-analyze-nav-live-sort-design.md`](../specs/2026-08-11-1250-tui-t10-analyze-nav-live-sort-design.md)

## Global Constraints

- Mole 钉版：`third_party/mole-1.48.1`（`update.go` / `live_config.go`）
- 删除漏斗零变更；禁止平行 `rm`
- footer 声明 ←→（按模式）；不声明 `S Live`
- **不 bump** `schema_version`；包版本 **2.14.0**
- TDD：先红再绿；每 Task 一次 commit
- 合入 merge commit；CI 绿后按 `pr-auto-merge-when-ci-green` 自动合并
- 测试：`VOLE_TEST_NO_AUTH=1`
- 不做磁盘 cache

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/scan/mod.rs` | `scan_directory_with_progress` |
| `crates/vole-core/src/analyze/mod.rs` | progress API |
| `crates/vole-core/src/lib.rs` | re-export（若需要） |
| `crates/vole-cli/src/tui/analyze_state.rs` | Back/Forward/LiveSort + mode |
| `crates/vole-cli/src/tui/widgets.rs` | footer ←→ |
| `crates/vole-cli/src/tui/analyze_view.rs` | footer 断言同步 |
| `crates/vole-cli/src/main.rs` | Child/Done channel |
| `README.md` / `docs/releases/v2.14.0.md` / `Cargo.toml` / `Formula/vole.rb` | 文档与版本 |

---

### Task 1: `scan_directory_with_progress`（红→绿）

**Files:**
- Modify: `crates/vole-core/src/scan/mod.rs`

**Interfaces:**
- `pub fn scan_directory_with_progress<F>(root: &Path, cancel: &CancelToken, mut on_child: F) -> io::Result<ScanResult> where F: FnMut(&DirEntry)`
- `scan_directory` 改为调用上述（空回调）

- [ ] **Step 1: 写失败单测**

在 `scan/mod.rs` tests（或新建）用 tempdir：两文件不同大小，progress 收集到的 path 顺序与完成顺序一致（先小后大写入时，回调顺序为处理顺序），最终 `entries` 仍按 size 降序且行为同 `scan_directory`。

```rust
#[test]
fn progress_emits_each_root_child_before_done() {
    // tempdir with a/ (small) and b/ (larger file)
    // scan_directory_with_progress pushes names to vec
    // assert !names.is_empty() && result.entries.len() == names.len() (or <= MAX)
}
```

- [ ] **Step 2: 跑测确认红**

```bash
cargo test -p vole-core progress_emits_each_root_child -- --nocapture
```

Expected: FAIL（符号不存在）

- [ ] **Step 3: 实现**

将现循环在每个 `child_entries.push(...)` 后 `on_child(&entry)`；`scan_directory` 委托。

- [ ] **Step 4: 绿**

```bash
cargo test -p vole-core progress_emits -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/scan/mod.rs
git commit -m "$(cat <<'EOF'
feat(scan): emit per-child progress during directory scan

EOF
)"
```

---

### Task 2: `analyze_directory_with_progress`（红→绿）

**Files:**
- Modify: `crates/vole-core/src/analyze/mod.rs`
- Modify: `crates/vole-core/src/lib.rs`（若需 pub use）

**Interfaces:**
- `pub enum AnalyzeScanEvent { Child(AnalyzeEntry), }` 或直接回调 `FnMut(AnalyzeEntry)`
- `pub fn analyze_directory_with_progress<F>(path, cancel, on_child: F) -> io::Result<AnalyzeOutput>`

- [ ] **Step 1: 红测** — progress 收到的 entry.path 非空；最终 output 与 `analyze_directory` 同 path 可比（entries 集合一致）

- [ ] **Step 2: 实现并绿**

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(analyze): expose progressive directory scan callback

EOF
)"
```

---

### Task 3: 导航别名 + footer ←→（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/tui/analyze_state.rs`
- Modify: `crates/vole-cli/src/tui/widgets.rs`
- Modify: `crates/vole-cli/src/tui/analyze_view.rs`（断言）

**Interfaces:**
- `AnalyzeKey::{Back, Forward}`
- map: Left/h/H/b/B→Back；Right/l/L→Forward
- `handle_key(Back)` ≡ Esc 分支；`Forward` ≡ Enter 分支
- footer: Directory `can_go_back` → `↑↓←→`；否则 `↑↓→`；Top → `↑↓←`

- [ ] **Step 1: 红测** map + Back/Forward 行为 + footer 含 `←`

- [ ] **Step 2: 实现并绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole map_key_nav -- --nocapture
```

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(analyze): add arrow and vim nav aliases with footer

EOF
)"
```

---

### Task 4: LiveSort 状态机（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/tui/analyze_state.rs`

**Interfaces:**
- `LiveSortMode::{FreezeOnMove, Continuous}` + `live_sort_mode_from_env()`
- `AnalyzeState { live_sort_mode, auto_sort_live }`
- `AnalyzeKey::LiveSort`；`s`/`S` → LiveSort（非 filtering）
- `handle_key(LiveSort)`：仅 scanning && !overview && !show_large_files 时切换并设 status
- `note_live_cursor_move(prev_selected)`：freeze 模式下选中变化则 `auto_sort_live=false`
- `apply_live_sort(entries: &mut [AnalyzeEntry])` 或由 main 调用的纯函数 `sort_entries_by_size`
- helpers: `upsert_live_child`、`finish_live_scan_selection`（pinFirstRow）

- [ ] **Step 1: 红测**

```rust
#[test]
fn live_sort_toggles_only_while_scanning() { ... }

#[test]
fn freeze_on_move_stops_after_effective_down() { ... }

#[test]
fn continuous_keeps_selected_path_after_sort() { ... }
```

- [ ] **Step 2: 实现并绿**

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(analyze): add S live sort modes freeze and continuous

EOF
)"
```

---

### Task 5: `cmd_analyze_tui` 接线进度 channel（红→绿 / 集成）

**Files:**
- Modify: `crates/vole-cli/src/main.rs`

**行为:**
- 扫描线程：`analyze_directory_with_progress`，每 Child `tx.send(Ok(Event::Child))`，结束 `Done(output)`
- 主循环：`try_recv` 合并 Child（upsert、total_size、按 mode 排序、clamp）；Done 替换 out、按 pin 规则设 selected、`scanning=false`
- Refresh / EnterDir / GoBack：丢弃旧 rx，重置 state（保留或重读 env 的 live_sort_mode）
- Up/Down 后调用 `note_live_cursor_move`

- [ ] **Step 1: 实现接线**（逻辑已有单测；此步手工/既有 bin 测不挂）

- [ ] **Step 2: 全量**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-core -p vole-cli --bin vole
cargo fmt --all -- --check
./scripts/check-command-surface.sh --enforce
```

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(analyze): stream scan children into TUI for live sort

EOF
)"
```

---

### Task 6: README + release 2.14.0 + bump

**Files:**
- `README.md`、`docs/releases/v2.14.0.md`、`Cargo.toml`、`Cargo.lock`、`Formula/vole.rb`

- [ ] **Step 1: 写 release notes**（相对 2.13.0；有意不做 cache；验收命令）

- [ ] **Step 2: README** analyze 行补导航别名与扫描中 S；成熟度 2.14.0

- [ ] **Step 3: bump + commit**

```bash
git commit -m "$(cat <<'EOF'
chore(release): bump to 2.14.0 for T10 analyze nav and live sort

EOF
)"
```

---

### Task 7: PR + CI 绿合并

- [ ] push + `gh pr create`
- [ ] CI 绿后 `gh pr merge <N> --merge --delete-branch`

---

## Spec coverage

| Spec | Task |
|---|---|
| ←→/h/b/l 导航 | Task 3 |
| footer ←→ | Task 3 |
| progress API | Task 1–2 |
| S live sort + env | Task 4–5 |
| JSON 不变 | Task 2（不改 analyze_directory）+ Task 5 仅 TUI |
| 2.14.0 | Task 6 |
| PR | Task 7 |
