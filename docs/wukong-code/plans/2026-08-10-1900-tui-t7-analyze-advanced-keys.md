# T7：analyze 进阶键 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（本仓默认 inline）或 wukong-code:subagent-driven-development。Steps 用 checkbox（`- [ ]`）跟踪。

**Goal:** TTY `vole analyze` 接线 Space 多选、⌫/Delete（保护+废纸篓）、O Open、P Preview、`/` Filter、T Top；诚实 footer；发版 **2.11.0**。

**Architecture:** 在 `vole-cli` 抽出可单测的 `AnalyzeState`（键处理 / 多选 / 过滤 / Top / 删除确认），`cmd_analyze_tui` 只做扫描线程、渲染与副作用（`mole_delete` Trash、`/usr/bin/open`、`qlmanage -p`）。删除禁止平行 `rm`。

**Tech Stack:** Rust 1.97.1、ratatui、crossterm、`vole_core::delete::mole_delete`、`AppProtection`、`MacTrash`。

**Design:** [`../specs/2026-08-10-1859-tui-t7-analyze-advanced-keys-design.md`](../specs/2026-08-10-1859-tui-t7-analyze-advanced-keys-design.md)

## Global Constraints

- Mole 钉版：`third_party/mole-1.48.1`（`cmd/analyze/update.go` / `delete.go`）
- 删除只走 `mole_delete` + `DeleteMode::Trash` + 保护；禁止平行 `rm`
- footer 只声明已接线键；**不**声明 `F File` / `R Refresh`
- **不 bump** `schema_version`；包版本 **2.11.0**（相对 `2.10.0`）
- TDD：先红再绿；每 Task 一次 commit
- 合入用 merge commit（非 squash）；CI 绿后按 `pr-auto-merge-when-ci-green` 自动合并
- 测试：`VOLE_TEST_NO_AUTH=1`；删除测用 `MOLE_TEST_TRASH_DIR`
- 本 plan 范围仅 T7（不做 status cat / optimize --whitelist）

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-cli/src/tui/analyze_state.rs` | 新建：状态机 + 键处理纯逻辑 |
| `crates/vole-cli/src/tui/analyze_actions.rs` | 新建：trash 删除 / open / preview 副作用（可单测 argv 与 trash 路径） |
| `crates/vole-cli/src/tui/analyze_view.rs` | ○/● 行、Top 全屏、filter/confirm UI、footer 入参 |
| `crates/vole-cli/src/tui/widgets.rs` | `AnalyzeFooterMode` + `analyze_footer` |
| `crates/vole-cli/src/tui/mod.rs` | mod / re-export |
| `crates/vole-cli/src/main.rs` | `cmd_analyze_tui` 接线 |
| `README.md` / `docs/releases/v2.11.0.md` / `Cargo.toml` / `Formula/vole.rb` | 文档与版本 |

---

### Task 1: `AnalyzeState` — Space / Filter / Top（红→绿）

**Files:**
- Create: `crates/vole-cli/src/tui/analyze_state.rs`
- Modify: `crates/vole-cli/src/tui/mod.rs`

**Interfaces:**
- Produces:
  - `pub enum AnalyzeKey { Up, Down, Enter, Esc, Quit, Space, Delete, Open, Preview, Filter, Top, FilterChar(char), FilterBackspace }`
  - `pub enum AnalyzeEffect { None, EnterDir(String), GoBack, Quit, Open(Vec<String>), Preview(String), RequestDelete(Vec<String>), ConfirmDelete, CancelDelete }`
  - `pub struct AnalyzeState { selected, show_large_files, multi_selected: BTreeSet<String>, large_multi_selected, entry_filter, large_filter, entry_filtering, large_filtering, delete_confirm, status: String, … }`
  - `impl AnalyzeState { pub fn handle_key(&mut self, key: AnalyzeKey, out: &AnalyzeOutput, scanning: bool, can_go_back: bool) -> AnalyzeEffect }`
  - `pub fn visible_entries<'a>(&self, out: &'a AnalyzeOutput) -> Vec<&'a AnalyzeEntry>`
  - `pub fn visible_large<'a>(&self, out: &'a AnalyzeOutput) -> Vec<&'a AnalyzeFileEntry>`

- [ ] **Step 1: 写失败单测**（文件尚不存在 → 编译失败即红）

```rust
// crates/vole-cli/src/tui/analyze_state.rs
#[cfg(test)]
mod tests {
    use super::*;
    use vole_core::vole_proto::{AnalyzeEntry, AnalyzeFileEntry, AnalyzeOutput};

    fn sample_out() -> AnalyzeOutput {
        AnalyzeOutput {
            path: "/tmp/a".into(),
            overview: false,
            total_size: 300,
            entries: vec![
                AnalyzeEntry {
                    name: "Caches".into(),
                    path: "/tmp/a/Caches".into(),
                    size: 200,
                    is_dir: true,
                    ..Default::default()
                },
                AnalyzeEntry {
                    name: "notes.txt".into(),
                    path: "/tmp/a/notes.txt".into(),
                    size: 100,
                    is_dir: false,
                    ..Default::default()
                },
            ],
            large_files: vec![AnalyzeFileEntry {
                name: "big.dmg".into(),
                path: "/tmp/a/big.dmg".into(),
                size: 1_000_000,
            }],
            total_files: Some(2),
        }
    }

    #[test]
    fn space_toggles_multi_select_after_scan() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        assert!(st.handle_key(AnalyzeKey::Space, &out, true, false) == AnalyzeEffect::None);
        assert!(st.multi_selected.is_empty());
        st.handle_key(AnalyzeKey::Space, &out, false, false);
        assert!(st.multi_selected.contains("/tmp/a/Caches"));
        st.handle_key(AnalyzeKey::Space, &out, false, false);
        assert!(st.multi_selected.is_empty());
    }

    #[test]
    fn filter_applies_and_clears_selection() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        st.handle_key(AnalyzeKey::Space, &out, false, false);
        assert!(!st.multi_selected.is_empty());
        st.handle_key(AnalyzeKey::Filter, &out, false, false);
        st.handle_key(AnalyzeKey::FilterChar('n'), &out, false, false);
        assert!(st.multi_selected.is_empty());
        st.handle_key(AnalyzeKey::Enter, &out, false, false);
        let vis = st.visible_entries(&out);
        assert_eq!(vis.len(), 1);
        assert_eq!(vis[0].name, "notes.txt");
        st.handle_key(AnalyzeKey::Esc, &out, false, false);
        assert_eq!(st.visible_entries(&out).len(), 2);
    }

    #[test]
    fn top_toggles_large_files_mode() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        st.handle_key(AnalyzeKey::Top, &out, false, false);
        assert!(st.show_large_files);
        st.handle_key(AnalyzeKey::Top, &out, false, false);
        assert!(!st.show_large_files);
        st.handle_key(AnalyzeKey::Top, &out, true, false);
        assert!(!st.show_large_files);
    }

    #[test]
    fn overview_disables_space_filter_top() {
        let mut out = sample_out();
        out.overview = true;
        let mut st = AnalyzeState::default();
        st.handle_key(AnalyzeKey::Space, &out, false, false);
        assert!(st.multi_selected.is_empty());
        st.handle_key(AnalyzeKey::Filter, &out, false, false);
        assert!(!st.entry_filtering);
        st.handle_key(AnalyzeKey::Top, &out, false, false);
        assert!(!st.show_large_files);
    }
}
```

- [ ] **Step 2: 跑测确认红**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_state -- --nocapture
```

Expected: FAIL（module/type 不存在）

- [ ] **Step 3: 最小实现**

在 `analyze_state.rs` 实现上述类型与 `handle_key`（本 Task 对 Delete/Open/Preview 可先返回对应 `AnalyzeEffect` 骨架，完整确认流在 Task 3）。`mod.rs` 加 `mod analyze_state;`。

过滤：`name.to_lowercase().contains(query.to_lowercase())`。改 filter 查询时 `multi_selected` / `large_multi_selected` 清空。导航 Up/Down 在 `visible_*` 上移动并 clamp。

- [ ] **Step 4: 跑测确认绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_state
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/tui/analyze_state.rs crates/vole-cli/src/tui/mod.rs
git commit -m "$(cat <<'EOF'
feat(analyze): add AnalyzeState for select/filter/top

Pure key-state machine for T7 Space, /, and T before wiring the TUI loop.
EOF
)"
```

---

### Task 2: 诚实 footer + 行 ○/●（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/tui/widgets.rs`（`analyze_footer`）
- Modify: `crates/vole-cli/src/tui/analyze_view.rs`（`format_analyze_row` 多选标记；`render_analyze` 签名）

**Interfaces:**
- Produces:
  - `pub enum AnalyzeFooterMode { Directory { can_go_back: bool, selected_count: usize, large_count: usize }, Top { selected_count: usize }, Filtering, DeleteConfirm }`
  - `pub fn analyze_footer(mode: AnalyzeFooterMode) -> String`
  - `format_analyze_row(..., multi_marked: Option<bool>)`：`Some(true)`→`●`，`Some(false)`→`○`，`None` 保持原 `▶`/空格前缀

- [ ] **Step 1: 改既有测试为新契约（先红）**

```rust
#[test]
fn analyze_footer_declares_wired_keys_only() {
    let f = analyze_footer(AnalyzeFooterMode::Directory {
        can_go_back: true,
        selected_count: 0,
        large_count: 3,
    });
    assert!(f.contains("Space"));
    assert!(f.contains("⌫") || f.contains("Del"));
    assert!(f.contains("O Open"));
    assert!(f.contains("P Preview"));
    assert!(f.contains("/ Filter"));
    assert!(f.contains("T Top"));
    assert!(!f.contains("F File"));
    assert!(!f.contains("R Refresh"));
}

#[test]
fn row_shows_multi_select_marks() {
    let entry = AnalyzeEntry {
        name: "Caches".into(),
        path: "/tmp/Caches".into(),
        size: 50,
        is_dir: true,
        ..Default::default()
    };
    let marked = format_analyze_row(&entry, 0, true, 100, 100, 12, false, Some(true));
    assert!(marked.contains('●'));
    let unmarked = format_analyze_row(&entry, 0, false, 100, 100, 12, false, Some(false));
    assert!(unmarked.contains('○'));
}
```

同步改 `footer_omits_unwired_actions` / `analyze_footer_modes` 调用点。

- [ ] **Step 2: 跑测确认红**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::widgets::tests::analyze_footer
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_view
```

Expected: FAIL

- [ ] **Step 3: 实现 footer + 行标记；更新 `render_analyze` 接受 `AnalyzeState` 只读视图字段（或显式参数：`multi_selected`、`show_large_files`、`footer_mode`、`status`）**

Top 全屏时：列表渲染 `large_files`；隐藏底部只读 Large files 摘要。

- [ ] **Step 4: 跑测绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::widgets
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_view
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/tui/widgets.rs crates/vole-cli/src/tui/analyze_view.rs
git commit -m "$(cat <<'EOF'
feat(analyze): honest footer and multi-select row marks

Declare only wired T7 keys; show ○/● when selection is active.
EOF
)"
```

---

### Task 3: 删除确认 + `mole_delete` Trash 漏斗（红→绿）

**Files:**
- Create: `crates/vole-cli/src/tui/analyze_actions.rs`
- Modify: `crates/vole-cli/src/tui/analyze_state.rs`（Delete / ConfirmDelete / CancelDelete）
- Modify: `crates/vole-cli/src/tui/mod.rs`

**Interfaces:**
- Produces:
  - `AnalyzeState::handle_key`：`Delete` → 设 `delete_confirm=true` 并 `RequestDelete` 仅作通知；确认态下 `Enter`→`ConfirmDelete`，`Esc`→`CancelDelete`
  - `pub fn trash_analyze_paths(paths: &[String]) -> TrashAnalyzeReport`
  - `pub struct TrashAnalyzeReport { pub removed: Vec<String>, pub errors: Vec<String> }`
  - 实现：对路径按分隔符深度降序排序；逐个 `mole_delete(path, &AppProtection::new(), &[], MoleDeleteOptions { mode: DeleteMode::Trash, dry_run: false, needs_sudo: false, privilege: None }, &MacTrash, &DeletionLogger::with_path(...), &mut OperationLogger::new("analyze"))`
  - **禁止** `std::fs::remove_file` / `remove_dir_all` 出现在 analyze 删除路径

- [ ] **Step 1: 写失败单测**

```rust
#[test]
fn delete_enters_confirm_then_cancel() {
    let out = sample_out();
    let mut st = AnalyzeState::default();
    let eff = st.handle_key(AnalyzeKey::Delete, &out, false, false);
    assert!(matches!(eff, AnalyzeEffect::RequestDelete(_)));
    assert!(st.delete_confirm);
    st.handle_key(AnalyzeKey::Esc, &out, false, false);
    assert!(!st.delete_confirm);
}

#[test]
fn trash_analyze_paths_uses_test_trash_dir() {
    let _guard = /* 若 cli 无 test_env lock，用临时 env + scopeguard 模式 */;
    let root = std::env::temp_dir().join(format!("vole-analyze-del-{}", std::process::id()));
    let victim = root.join("victim.txt");
    let trash = root.join("Trash");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&victim, b"x").unwrap();
    std::env::set_var("VOLE_TEST_NO_AUTH", "1");
    std::env::set_var("MOLE_TEST_TRASH_DIR", &trash);
    let report = trash_analyze_paths(&[victim.to_string_lossy().into_owned()]);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(!victim.exists());
    std::env::remove_var("MOLE_TEST_TRASH_DIR");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn trash_analyze_paths_rejects_protected() {
    std::env::set_var("VOLE_TEST_NO_AUTH", "1");
    let report = trash_analyze_paths(&["/System/Library".into()]);
    assert!(report.removed.is_empty());
    assert!(!report.errors.is_empty());
}
```

- [ ] **Step 2: 跑测红**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_actions
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_state
```

Expected: FAIL

- [ ] **Step 3: 实现 `trash_analyze_paths` + 确认态键处理；删除成功后由调用方从 `AnalyzeOutput` 移除条目（纯函数 `fn apply_removals(out: &mut AnalyzeOutput, removed: &[String])` 放 `analyze_actions.rs`）**

- [ ] **Step 4: 跑测绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_actions
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_state
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/tui/analyze_actions.rs crates/vole-cli/src/tui/analyze_state.rs crates/vole-cli/src/tui/mod.rs
git commit -m "$(cat <<'EOF'
feat(analyze): delete via protection and trash funnel

Confirm-then-mole_delete(Trash) for analyze selections; no parallel rm.
EOF
)"
```

---

### Task 4: Open / Preview 副作用 + 状态机接线（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/tui/analyze_actions.rs`
- Modify: `crates/vole-cli/src/tui/analyze_state.rs`

**Interfaces:**
- Produces:
  - `pub fn open_argv(path: &str) -> Vec<String>` → `["/usr/bin/open", path]`
  - `pub fn preview_argv(path: &str) -> Option<Vec<String>>` → 文件：`Some(["/usr/bin/qlmanage", "-p", path])`；目录：`None`
  - `pub const MAX_BATCH_OPEN: usize = 20`
  - `handle_key(Open)`：多选优先，超量则 `status` 提示且 `AnalyzeEffect::None`；否则 `Open(paths)`
  - `handle_key(Preview)`：仅当前可见行且非目录 → `Preview(path)`

- [ ] **Step 1: 写失败单测**

```rust
#[test]
fn open_and_preview_argv_shapes() {
    assert_eq!(
        open_argv("/tmp/a"),
        vec!["/usr/bin/open".into(), "/tmp/a".into()]
    );
    assert_eq!(
        preview_argv("/tmp/a.txt"),
        Some(vec![
            "/usr/bin/qlmanage".into(),
            "-p".into(),
            "/tmp/a.txt".into()
        ])
    );
    assert!(preview_argv("/tmp/dir").is_none()); // 调用方对目录传 is_dir
}

#[test]
fn open_caps_batch_at_20() {
    let mut out = sample_out();
    out.entries = (0..25)
        .map(|i| AnalyzeEntry {
            name: format!("f{i}"),
            path: format!("/tmp/a/f{i}"),
            size: 1,
            is_dir: false,
            ..Default::default()
        })
        .collect();
    let mut st = AnalyzeState::default();
    for i in 0..25 {
        st.selected = i;
        st.handle_key(AnalyzeKey::Space, &out, false, false);
    }
    let eff = st.handle_key(AnalyzeKey::Open, &out, false, false);
    assert_eq!(eff, AnalyzeEffect::None);
    assert!(st.status.contains("max 20"));
}
```

（`preview_argv` 若只看 path 字符串，则单测改为 `preview_target(path, is_dir)`。）

- [ ] **Step 2: 红 → Step 3 实现 → Step 4 绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_actions
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_state
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/tui/analyze_actions.rs crates/vole-cli/src/tui/analyze_state.rs
git commit -m "$(cat <<'EOF'
feat(analyze): wire Open and Preview effects

Batch-open capped at 20; Quick Look preview for files only.
EOF
)"
```

---

### Task 5: `cmd_analyze_tui` 接线（红→绿 / 集成）

**Files:**
- Modify: `crates/vole-cli/src/main.rs`（`cmd_analyze_tui`）
- Modify: `crates/vole-cli/src/tui/analyze_view.rs`（最终 render 签名）
- Modify: `crates/vole-cli/src/tui/mod.rs`（导出 `map_analyze_key` 若放 state）

**Interfaces:**
- `map_analyze_key(KeyEvent) -> Option<AnalyzeKey>`：` `→Space；`Backspace`/`Delete`→Delete；`o`/`O`→Open；`p`/`P`→Preview；`/`→Filter；`t`/`T`→Top；过滤输入态下普通字符→`FilterChar`
- 循环：`state.handle_key` → match `AnalyzeEffect`：
  - `EnterDir` / `GoBack`：维持现有 stack + rescan
  - `ConfirmDelete`：`trash_analyze_paths` → `apply_removals` → 清多选 → `status`
  - `Open` / `Preview`：`std::process::Command` 分离启动（忽略失败码，写 status）
  - `Quit`：cancel

- [ ] **Step 1: 写 `map_analyze_key` 单测（放 `analyze_state.rs`）**

```rust
#[test]
fn map_analyze_key_core_bindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    assert_eq!(
        map_analyze_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
        Some(AnalyzeKey::Space)
    );
    assert_eq!(
        map_analyze_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        Some(AnalyzeKey::Delete)
    );
    assert_eq!(
        map_analyze_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)),
        Some(AnalyzeKey::Filter)
    );
}
```

- [ ] **Step 2: 红→实现 map + 改 TUI 循环→绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_state
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::analyze_view
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::widgets
```

Expected: PASS；`cargo build -p vole-cli` 成功。

- [ ] **Step 3: Commit**

```bash
git add crates/vole-cli/src/main.rs crates/vole-cli/src/tui/
git commit -m "$(cat <<'EOF'
feat(analyze): connect advanced keys in TUI loop

Drive AnalyzeState from crossterm; apply trash/open/preview side effects.
EOF
)"
```

---

### Task 6: 文档 + 版本 2.11.0 + release notes

**Files:**
- Modify: `README.md`（去掉「analyze 删除/多选/Open/Preview 有意未接线」；成熟度/对照表勾 T7）
- Create: `docs/releases/v2.11.0.md`
- Modify: `Cargo.toml`（`version = "2.11.0"`）
- Modify: `Cargo.lock`
- Modify: `Formula/vole.rb`（version 字段；sha256 占位待 tag 后脚本）

- [ ] **Step 1: 写 `docs/releases/v2.11.0.md`**

相对 2.10.0：T7 analyze 进阶键；硬约束 trash-only；有意不做（F/R、T8）；验收命令；Formula 说明。

- [ ] **Step 2: bump + README**

- [ ] **Step 3: 验证**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli
./scripts/check-command-surface.sh --enforce
cargo fmt --all -- --check
```

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock README.md docs/releases/v2.11.0.md Formula/vole.rb
git commit -m "$(cat <<'EOF'
chore(release): bump to 2.11.0 for T7 analyze keys

Document Space/Delete/Open/Preview/Filter/Top; Formula sha pending tag.
EOF
)"
```

---

### Task 7: 收口验证 + PR + 发版运营

**Files:** 无新代码（除非 CI/fmt 修补）

- [ ] **Step 1: 全量相关验证**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli
./scripts/check-command-surface.sh --enforce
./scripts/check-protocol-doc.sh
cargo fmt --all -- --check
```

- [ ] **Step 2: finishing-a-development-branch → 开 PR**

分支 `feat/t7-analyze-advanced-keys`；`gh pr create`；CI 绿后 `gh pr merge --merge --delete-branch`。

- [ ] **Step 3: 发版运营（合入后）**

对齐 T5/T6：annotated tag `v2.11.0` → 等 Release assets → `bash scripts/update-homebrew-formula.sh 2.11.0` → Formula PR merge。

---

## Self-Review

| Spec 要求 | Task |
|---|---|
| Space 多选 | Task 1/5 |
| ⌫/Delete + 确认 | Task 3/5 |
| 删除只走保护+废纸篓 | Task 3 |
| O Open / P Preview | Task 4/5 |
| `/` Filter | Task 1/5 |
| T Top | Task 1/2/5 |
| footer 只声明已接线 | Task 2 |
| 无 F/R 虚标 | Task 2 |
| JSON 零破坏 | Task 5 不改 JSON 枝 |
| 2.11.0 / 不 bump schema | Task 6 |
| VOLE_TEST_NO_AUTH + test trash | Task 3 |

无 TBD；`MAX_BATCH_OPEN = 20`；删除模式固定 `Trash`。
