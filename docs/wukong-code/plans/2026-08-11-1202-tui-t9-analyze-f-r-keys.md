# T9：analyze F + R Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（本仓默认 inline）或 wukong-code:subagent-driven-development。Steps 用 checkbox（`- [ ]`）跟踪。

**Goal:** TTY `vole analyze` 接线 `F` Finder reveal（`open -R`）与 `R` Refresh（强制重扫当前路径）；诚实 footer；发版 **2.13.0**。

**Architecture:** 复用 T7 `AnalyzeState` / `paths_for_action` / `spawn_detached`；新增 `reveal_argv` 与 `AnalyzeEffect::{Reveal,Refresh}`；`cmd_analyze_tui` 对 Refresh 置 `scanning=true` 并丢弃进行中扫描。不引入 mole 磁盘 cache。

**Tech Stack:** Rust 1.97.1、ratatui、crossterm、`/usr/bin/open -R`。

**Design:** [`../specs/2026-08-11-1201-tui-t9-analyze-f-r-keys-design.md`](../specs/2026-08-11-1201-tui-t9-analyze-f-r-keys-design.md)

## Global Constraints

- Mole 钉版：`third_party/mole-1.48.1`（`cmd/analyze/update.go` / `safeOpen(..., true)`）
- 删除漏斗零变更；F/R 不触发删除；禁止平行 `rm`
- footer 接线后声明 `F File` / `R Refresh`；不声明 `S Live` / 导航别名
- **不 bump** `schema_version`；包版本 **2.13.0**（相对 `2.12.0`）
- TDD：先红再绿；每 Task 一次 commit
- 合入用 merge commit（非 squash）；CI 绿后按 `pr-auto-merge-when-ci-green` 自动合并
- 测试：`VOLE_TEST_NO_AUTH=1`
- 本 plan 范围仅 T9（不做 ←→/`h`/`b`、不做 `S` live sort）

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-cli/src/tui/analyze_actions.rs` | `reveal_argv` |
| `crates/vole-cli/src/tui/analyze_state.rs` | `Reveal`/`Refresh` 键与 effect |
| `crates/vole-cli/src/tui/widgets.rs` | footer 追加 F/R |
| `crates/vole-cli/src/tui/analyze_view.rs` | 若有 footer 相关断言则同步 |
| `crates/vole-cli/src/tui/mod.rs` | re-export `reveal_argv` |
| `crates/vole-cli/src/main.rs` | `cmd_analyze_tui` 接线 |
| `README.md` / `docs/releases/v2.13.0.md` / `Cargo.toml` / `Formula/vole.rb` | 文档与版本 |

---

### Task 1: `reveal_argv` + 键映射 / `begin_reveal` / `Refresh` effect（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/tui/analyze_actions.rs`
- Modify: `crates/vole-cli/src/tui/analyze_state.rs`
- Modify: `crates/vole-cli/src/tui/mod.rs`

**Interfaces:**
- Consumes: `MAX_BATCH_OPEN`、`paths_for_action`、`AnalyzeState::handle_key`
- Produces:
  - `pub fn reveal_argv(path: &str) -> Vec<String>`
  - `AnalyzeKey::{Reveal, Refresh}`
  - `AnalyzeEffect::{Reveal(Vec<String>), Refresh}`
  - `map_analyze_key`: `'f'|'F'`→Reveal，`'r'|'R'`→Refresh（非 filtering）
  - `begin_reveal`：同 `begin_open` 批量上限语义，文案 `Too many items to reveal, max {MAX_BATCH_OPEN}, selected {n}`

- [ ] **Step 1: 写失败单测**

在 `analyze_actions.rs` tests 追加：

```rust
#[test]
fn reveal_argv_uses_open_r() {
    assert_eq!(
        reveal_argv("/tmp/a"),
        vec![
            "/usr/bin/open".to_string(),
            "-R".to_string(),
            "/tmp/a".to_string()
        ]
    );
}
```

在 `analyze_state.rs` tests 追加：

```rust
#[test]
fn map_key_reveal_and_refresh() {
    assert_eq!(
        map_analyze_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), false),
        Some(AnalyzeKey::Reveal)
    );
    assert_eq!(
        map_analyze_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE), false),
        Some(AnalyzeKey::Refresh)
    );
    assert_eq!(
        map_analyze_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), true),
        Some(AnalyzeKey::FilterChar('f'))
    );
}

#[test]
fn reveal_respects_batch_limit_and_refresh_effect() {
    let out = sample_out();
    let mut st = AnalyzeState::default();
    for i in 0..21 {
        st.multi_selected.insert(format!("/tmp/a/item{i}"));
    }
    assert_eq!(st.handle_key(AnalyzeKey::Reveal, &out, false, false), AnalyzeEffect::None);
    assert!(st.status.contains("Too many items to reveal"));

    st.multi_selected.clear();
    st.multi_selected.insert("/tmp/a/Caches".into());
    assert_eq!(
        st.handle_key(AnalyzeKey::Reveal, &out, false, false),
        AnalyzeEffect::Reveal(vec!["/tmp/a/Caches".into()])
    );

    st.delete_confirm = true;
    assert_eq!(st.handle_key(AnalyzeKey::Refresh, &out, false, false), AnalyzeEffect::None);

    st.delete_confirm = false;
    assert_eq!(
        st.handle_key(AnalyzeKey::Refresh, &out, false, false),
        AnalyzeEffect::Refresh
    );
}
```

- [ ] **Step 2: 跑测确认红**

```bash
cd /Users/wukong/Documents/vole
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole reveal_argv_uses_open_r map_key_reveal_and_refresh reveal_respects_batch_limit -- --nocapture
```

Expected: 编译失败或 FAIL（缺符号 / 断言失败）

- [ ] **Step 3: 最小实现**

`analyze_actions.rs`：

```rust
pub fn reveal_argv(path: &str) -> Vec<String> {
    vec!["/usr/bin/open".into(), "-R".into(), path.to_string()]
}
```

`analyze_state.rs`：

- `AnalyzeKey` 增 `Reveal`, `Refresh`
- `AnalyzeEffect` 增 `Reveal(Vec<String>)`, `Refresh`
- `map_analyze_key` 非 filtering 分支：`'f'|'F' => Reveal`, `'r'|'R' => Refresh`
- `handle_key` 正常模式：`Reveal => begin_reveal(out)`；`Refresh =>` 若 `delete_confirm` 已由分支处理；否则清多选并 `AnalyzeEffect::Refresh`（状态行可先设 `Refreshing...`，main 也会 reset）
- `handle_delete_confirm`：对 `Reveal`/`Refresh` 返回 `None`（忽略）
- `begin_reveal`：复制 `begin_open`，上限文案用 reveal

`mod.rs`：

```rust
pub use analyze_actions::{
    apply_removals, open_argv, preview_target, reveal_argv, spawn_detached, trash_analyze_paths,
};
```

- [ ] **Step 4: 跑测确认绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole reveal_argv_uses_open_r map_key_reveal_and_refresh reveal_respects_batch_limit
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/tui/analyze_actions.rs crates/vole-cli/src/tui/analyze_state.rs crates/vole-cli/src/tui/mod.rs
git commit -m "$(cat <<'EOF'
feat(analyze): add F reveal and R refresh state keys

EOF
)"
```

---

### Task 2: 诚实 footer 声明 F/R（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/tui/widgets.rs`
- Modify: `crates/vole-cli/src/tui/analyze_view.rs`（若测试仍断言不含 F File）

**Interfaces:**
- Consumes: `AnalyzeFooterMode`
- Produces: Directory / Top footer 含 `| F File | R Refresh`（位置：在 `P Preview` 与删除键之间，或 Preview 后紧接——与 mole 顺序接近即可）

- [ ] **Step 1: 改单测为正向断言（先改测→红）**

`widgets.rs` `analyze_footer_declares_wired_keys_only`：

```rust
assert!(f.contains("F File"));
assert!(f.contains("R Refresh"));
assert!(!f.contains("S Live"));
```

`analyze_view.rs` 同名断言同步。

- [ ] **Step 2: 跑测确认红**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole analyze_footer_declares_wired_keys_only
```

Expected: FAIL（footer 尚无 F/R）

- [ ] **Step 3: 更新 `analyze_footer`**

Directory：

```rust
format!("↑↓ | Space | Enter | / Filter | O Open | P Preview | F File | R Refresh | {del}{top} | {esc}")
```

Top：

```rust
format!("↑↓ | Space | / Filter | O Open | P Preview | F File | R Refresh | {del} | Esc Back | Q/Ctrl+C Quit")
```

- [ ] **Step 4: 跑测确认绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole analyze_footer
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/tui/widgets.rs crates/vole-cli/src/tui/analyze_view.rs
git commit -m "$(cat <<'EOF'
feat(analyze): declare F File and R Refresh in footer

EOF
)"
```

---

### Task 3: `cmd_analyze_tui` 接线 Reveal + Refresh（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/main.rs`（`cmd_analyze_tui` match arm）

**Interfaces:**
- Consumes: `AnalyzeEffect::Reveal` / `Refresh`；`reveal_argv`；`spawn_detached`
- Produces: Reveal 副作用；Refresh 重扫当前 stack 顶

- [ ] **Step 1: 若无可单独编译的 CLI 测，用手工契约注释 + 编译检查；优先加轻量单元测覆盖 effect 处理纯函数则本步跳过红测，直接实现后跑全量 analyze 测**

本 Task 以集成行为为主。实现前确认 Task 1/2 绿。

- [ ] **Step 2: 在 `match effect` 增加**

```rust
tui::AnalyzeEffect::Reveal(paths) => {
    let n = paths.len();
    for p in paths {
        let argv = tui::reveal_argv(&p);
        if let Err(e) = tui::spawn_detached(&argv) {
            state.status = format!("Reveal failed: {e}");
        }
    }
    if state.status.is_empty() {
        state.status = if n == 1 {
            "Showing in Finder…".into()
        } else {
            format!("Showing {n} items in Finder…")
        };
    }
}
tui::AnalyzeEffect::Refresh => {
    scanning = true;
    scan_rx = None;
    state = tui::AnalyzeState::default();
    state.status = "Refreshing...".into();
}
```

注意：`AnalyzeState::default()` 后立刻写 `status`；若 Refresh 在 state 内已设 status，main reset 会清掉——以 main 赋值 `Refreshing...` 为准。

- [ ] **Step 3: 编译 + 相关测**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole analyze_
```

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vole-cli/src/main.rs
git commit -m "$(cat <<'EOF'
feat(analyze): wire Finder reveal and rescan on F/R

EOF
)"
```

---

### Task 4: README + release 2.13.0 + 版本 bump

**Files:**
- Modify: `README.md`（analyze 行注明 F/R；成熟度 **2.13.0**）
- Create: `docs/releases/v2.13.0.md`
- Modify: `Cargo.toml` workspace `version = "2.13.0"`（及 lock 同步）
- Modify: `Formula/vole.rb`（version/URL → 2.13.0；sha256 占位 `0…0` 或按仓惯例）

- [ ] **Step 1: 写 `docs/releases/v2.13.0.md`**

结构对齐 `v2.12.0.md`：相对 2.12.0 的 T9 F/R；硬约束；有意不做（←→、S live sort）；验收命令；Formula 说明。

- [ ] **Step 2: README**

- TUI 表 analyze 行：补 `F` Finder / `R` Refresh
- 成熟度行 → **2.13.0**；去掉「不虚标 F/R」类旧措辞（若仍有）
- 预编译示例版本号若写死则改 `v2.13.0`

- [ ] **Step 3: bump 版本**

```bash
# Cargo.toml version = "2.13.0"
# 同步 Cargo.lock / Formula
```

- [ ] **Step 4: 全量校验**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole
./scripts/check-command-surface.sh --enforce
cargo fmt --all -- --check
```

Expected: 全绿

- [ ] **Step 5: Commit**

```bash
git add README.md docs/releases/v2.13.0.md Cargo.toml Cargo.lock Formula/vole.rb
git commit -m "$(cat <<'EOF'
chore(release): bump to 2.13.0 for T9 analyze F/R

EOF
)"
```

---

### Task 5: PR + CI 绿合并

**Files:** 无新代码（推送已有 commits）

- [ ] **Step 1: 推分支并开 PR**

```bash
git checkout -b feat/t9-analyze-f-r-keys
git push -u origin HEAD
gh pr create --title "feat(tui): T9 analyze F reveal + R refresh (2.13.0)" --body "$(cat <<'EOF'
## Summary
- Wire `F` Finder reveal (`open -R`) and `R` force rescan in `vole analyze` TUI
- Honest footer declares F/R; bump to 2.13.0

## Test plan
- [x] `VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --bin vole`
- [x] `./scripts/check-command-surface.sh --enforce`
- [x] `cargo fmt --all -- --check`
- [ ] TTY: `vole analyze ~` → F opens Finder; R rescans

EOF
)"
```

- [ ] **Step 2: 等 CI；按 `pr-auto-merge-when-ci-green` 校验后 `gh pr merge <N> --merge --delete-branch`**

---

## Spec coverage (self-review)

| Spec 要求 | Task |
|---|---|
| `F` → `open -R`，批量 20 | Task 1 + 3 |
| `R` → 重扫当前路径，stack 不变 | Task 1 + 3 |
| footer 声明 F/R | Task 2 |
| 删除/JSON 零破坏 | Task 3（不改删除臂）+ 既有测 |
| 不做 ←→ / S | 无对应 Task |
| 2.13.0 发版文档 | Task 4 |
| PR 合并 | Task 5 |
