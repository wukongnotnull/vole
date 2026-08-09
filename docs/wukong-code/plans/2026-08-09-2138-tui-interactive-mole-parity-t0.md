# TUI 交互 mole 级复刻 T0（PaginatedMultiSelect + uninstall）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（本仓默认 inline）或 wukong-code:subagent-driven-development。Steps 用 checkbox（`- [ ]`）跟踪。

**Goal:** 交付共享 `PaginatedMultiSelect`（契约档 A）并让 TTY 裸 `vole uninstall` 走 mole 式多选→确认→apply；自动化路径零破坏；发版 **2.7.0**。

**Architecture:** 纯逻辑 `MenuState`（可单测）+ ratatui 渲染循环在 `vole-cli::tui`；`vole-core` 仅增加「对给定 `AppIdentity` 列表建 uninstall plan」。交互确认后内存 `ProtoPlan` → 现有 `apply_uninstall_plan`。非 TTY / `--plan` / `--json*` 等保持现路径。

**Tech Stack:** Rust 1.97.1、`ratatui` 0.30、`crossterm` 0.28、既有 `TerminalGuard`、`vole-core` uninstall plan/apply。

**Design:** [`../specs/2026-08-09-2136-tui-interactive-mole-parity-design.md`](../specs/2026-08-09-2136-tui-interactive-mole-parity-design.md)

## Global Constraints

- Mole 钉版：`third_party/mole-1.48.1`（`lib/ui/menu_paginated.sh`、`bin/uninstall.sh`）
- 删除只走 `apply_uninstall_plan`；禁止平行 `rm`
- 候选粒度是 **app**（`AppIdentity`），不是 plan 残留路径条目
- **不 bump** `schema_version`；功能就绪后包版本 **2.7.0**（相对 `2.6.0`）
- TDD：先红再绿；每 Task 一次 commit
- 合入用 merge commit（非 squash）；CI 绿后按 `pr-auto-merge-when-ci-green` 自动合并
- 测试：`VOLE_TEST_NO_AUTH=1`；禁止交互路径挂真 sudo / Touch ID

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-cli/src/tui/menu_state.rs` | 纯逻辑：过滤/排序/分页/键位 → `SelectOutcome` |
| `crates/vole-cli/src/tui/paginated_select.rs` | ratatui 循环 + drain + `TerminalGuard` |
| `crates/vole-cli/src/tui/mod.rs` | 模块导出 |
| `crates/vole-core/src/ops/uninstall_plan.rs` | `build_uninstall_plan_for_apps`；导出 menu 用扫描辅助若需 |
| `crates/vole-core/src/ops/mod.rs` | 导出新 API |
| `crates/vole-cli/src/uninstall.rs` | 双轨：interactive vs plan/apply |
| `crates/vole-cli/src/main.rs` | 放宽 `--permanent`；传入 `explicit_plan` |
| `crates/vole-cli/tests/uninstall_cli.rs` | 非交互回归 + help 文案 |
| `README.md` / `docs/releases/v2.7.0.md` / `Cargo.toml` / `Formula/vole.rb` | 文档与版本 |

---

### Task 1: `MenuState` 纯逻辑 + MenuContract 红绿

**Files:**
- Create: `crates/vole-cli/src/tui/menu_state.rs`
- Modify: `crates/vole-cli/src/tui/mod.rs`

**Interfaces:**
- Produces:
  - `pub struct MenuItem { pub label: String, pub filter_name: Option<String>, pub epoch: Option<i64>, pub size_kb: Option<u64> }`
  - `pub enum MenuKey { Up, Down, Space, Enter, Quit, Char(char), Backspace }`
  - `pub enum SelectOutcome { Confirmed(Vec<usize>), Cancelled }`
  - `pub struct MenuConfig { pub sort_mode: SortMode, pub sort_reverse: bool, pub ignore_initial_enter: bool, pub preselected: Vec<usize>, pub term_height: u16 }`
  - `pub enum SortMode { Date, Name, Size }`
  - `pub struct MenuState` with `pub fn new(items: Vec<MenuItem>, cfg: MenuConfig) -> Result<Self, EmptyMenuError>`
  - `pub fn handle_key(&mut self, key: MenuKey) -> Option<SelectOutcome>`
  - `pub fn visible_page(&self) -> &[usize]`（原始下标）
  - `pub fn items_per_page(term_height: u16) -> usize`（reserved=5，clamp 1..=50）
  - `pub fn config_from_env() -> MenuConfig`（读 `VOLE_MENU_*`，缺省再读 `MOLE_MENU_*`）

- [ ] **Step 1: 写失败单测**（`menu_state.rs` 内 `#[cfg(test)]`）

```rust
#[test]
fn items_per_page_clamps() {
    assert_eq!(MenuState::items_per_page(3), 1);
    assert_eq!(MenuState::items_per_page(20), 15); // 20-5
    assert_eq!(MenuState::items_per_page(200), 50);
}

#[test]
fn empty_menu_errors() {
    assert!(MenuState::new(vec![], MenuConfig::default()).is_err());
}

#[test]
fn space_toggles_enter_returns_original_indices() {
    let items = vec![
        MenuItem { label: "B".into(), filter_name: None, epoch: Some(2), size_kb: Some(20) },
        MenuItem { label: "A".into(), filter_name: None, epoch: Some(1), size_kb: Some(10) },
    ];
    let mut st = MenuState::new(items, MenuConfig {
        sort_mode: SortMode::Name,
        ..MenuConfig::default()
    }).unwrap();
    // name 排序后视图: A(1), B(0)
    assert_eq!(st.handle_key(MenuKey::Space), None);
    assert_eq!(st.handle_key(MenuKey::Enter), Some(SelectOutcome::Confirmed(vec![1])));
}

#[test]
fn quit_cancels_filter_clear_first() {
    let items = vec![MenuItem { label: "Alpha".into(), filter_name: None, epoch: None, size_kb: None }];
    let mut st = MenuState::new(items, MenuConfig::default()).unwrap();
    st.handle_key(MenuKey::Char('a'));
    assert!(st.handle_key(MenuKey::Quit).is_none()); // 清过滤
    assert_eq!(st.handle_key(MenuKey::Quit), Some(SelectOutcome::Cancelled));
}

#[test]
fn ignore_initial_enter() {
    let items = vec![MenuItem { label: "X".into(), filter_name: None, epoch: None, size_kb: None }];
    let mut st = MenuState::new(items, MenuConfig {
        ignore_initial_enter: true,
        ..MenuConfig::default()
    }).unwrap();
    assert!(st.handle_key(MenuKey::Enter).is_none());
    st.handle_key(MenuKey::Space);
    assert_eq!(st.handle_key(MenuKey::Enter), Some(SelectOutcome::Confirmed(vec![0])));
}

#[test]
fn no_epoch_metadata_forces_name_sort() {
    let items = vec![
        MenuItem { label: "B".into(), filter_name: None, epoch: None, size_kb: Some(1) },
        MenuItem { label: "A".into(), filter_name: None, epoch: None, size_kb: Some(2) },
    ];
    let st = MenuState::new(items, MenuConfig {
        sort_mode: SortMode::Date,
        ..MenuConfig::default()
    }).unwrap();
    assert_eq!(st.visible_page()[0], 1); // A
}
```

另测：`preselected` 初始勾选；`SortMode::Size` 有 `size_kb` 时生效。

- [ ] **Step 2: Run 确认红**

Run: `cargo test -p vole-cli menu_state -- --nocapture`  
Expected: FAIL（模块不存在）

- [ ] **Step 3: 最小实现**

实现 `MenuState`：维护 `view_indices`、`selected: HashSet<usize>`（原始下标）、`cursor`/`top`、`filter_text`、`ignore_initial_enter` 一次性消费。  
排序：有 epoch 才允许 Date；有 size_kb 才允许 Size；否则 Name。  
`Char` 追加过滤（大小写不敏感，对 `filter_name.unwrap_or(label)`）；`Backspace` 删字符。  
循环切换排序可用后续扩展键；T0 至少通过 `MenuConfig.sort_mode` 固定模式满足契约测。

- [ ] **Step 4: 测试绿 + Commit**

```bash
cargo test -p vole-cli menu_state
git add crates/vole-cli/src/tui/menu_state.rs crates/vole-cli/src/tui/mod.rs
git commit -m "$(cat <<'EOF'
feat(cli): MenuState for paginated multi-select contract

Pure key/filter/sort/page logic with MenuContract unit tests,
independent of ratatui, for mole-aligned uninstall TUI.
EOF
)"
```

---

### Task 2: ratatui `paginated_select` + drain

**Files:**
- Create: `crates/vole-cli/src/tui/paginated_select.rs`
- Modify: `crates/vole-cli/src/tui/mod.rs`
- Modify: `crates/vole-cli/src/terminal.rs`（仅当需要「无 mouse 的 enter」时；默认复用 `TerminalGuard::enter`）

**Interfaces:**
- Consumes: `MenuState`, `MenuItem`, `MenuConfig`, `SelectOutcome`, `TerminalGuard`
- Produces:
  - `pub fn drain_pending_input(timeout: Duration)`
  - `pub fn run_paginated_select(title: &str, items: Vec<MenuItem>, cfg: MenuConfig) -> io::Result<SelectOutcome>`

- [ ] **Step 1: 写失败单测（drain / 键映射）**

在 `paginated_select.rs`：

```rust
#[test]
fn map_crossterm_key_space_enter_quit() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    assert!(matches!(map_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)), Some(MenuKey::Space)));
    assert!(matches!(map_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), Some(MenuKey::Enter)));
    assert!(matches!(map_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)), Some(MenuKey::Quit)));
    assert!(matches!(map_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), Some(MenuKey::Quit)));
}
```

`run_paginated_select` 本身用手工/expect 验证；本 Task 单测覆盖映射即可。

- [ ] **Step 2: Run 确认红**

Run: `cargo test -p vole-cli paginated_select -- --nocapture`  
Expected: FAIL

- [ ] **Step 3: 实现**

```rust
pub fn drain_pending_input(timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !event::poll(Duration::from_millis(10)).unwrap_or(false) {
            continue;
        }
        let _ = event::read();
    }
}

pub fn run_paginated_select(
    title: &str,
    items: Vec<MenuItem>,
    cfg: MenuConfig,
) -> io::Result<SelectOutcome> {
    let mut guard = TerminalGuard::enter()?;
    drain_pending_input(Duration::from_millis(200));
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut state = MenuState::new(items, cfg).map_err(|e| io::Error::other(e.to_string()))?;
    loop {
        term.draw(|f| render_menu(f, title, &state))?;
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let Some(mk) = map_key(key) else { continue };
        if let Some(out) = state.handle_key(mk) {
            guard.restore();
            return Ok(out);
        }
    }
}
```

渲染：标题、可见页（`[x]`/`[ ]` + label + 可选 size）、footer `Space | Enter Confirm | Q Cancel`。  
注意：若 `crossterm` 版本产生 Repeat，只处理 `Press`。

- [ ] **Step 4: 测试绿 + Commit**

```bash
cargo test -p vole-cli paginated_select
git add crates/vole-cli/src/tui/paginated_select.rs crates/vole-cli/src/tui/mod.rs
git commit -m "$(cat <<'EOF'
feat(cli): ratatui paginated multi-select runner

Wire MenuState through TerminalGuard with input drain and
mole-like footer shortcuts for shared TUI selection.
EOF
)"
```

---

### Task 3: core — `build_uninstall_plan_for_apps`

**Files:**
- Modify: `crates/vole-core/src/ops/uninstall_plan.rs`
- Modify: `crates/vole-core/src/ops/mod.rs`

**Interfaces:**
- Consumes: 现有 `build_uninstall_plan_with_brew` 循环体
- Produces:
  - `pub fn build_uninstall_plan_for_apps(catalog, protection, opts, apps: &[AppIdentity]) -> Result<ProtoPlan, OpsError>`
  - `pub fn build_uninstall_plan_for_apps_with_brew(..., brew: &dyn BrewDeps) -> Result<ProtoPlan, OpsError>`
- 行为：与全量 plan 相同保护/leftovers/brew/login/system 逻辑，但 **只遍历传入的 apps**（不再 `scan_applications`）；`opts.target_bundle_or_name` 在此 API 上忽略（调用方已选定）。

- [ ] **Step 1: 写失败单测**

```rust
#[test]
fn plan_for_apps_only_includes_selected() {
    let dir = tempfile::tempdir().unwrap();
    let apps_dir = dir.path().join("Applications");
    // 创建 FixtureA.app + FixtureB.app（Info.plist bundle id 不同）
    // ...
    let scanned = scan_applications(&[apps_dir]).unwrap();
    assert_eq!(scanned.len(), 2);
    let only = vec![scanned[0].clone()];
    let catalog = ProtectionCatalog::embedded();
    let protection = AppProtection::new();
    let home = dir.path().join("home");
    fs::create_dir_all(home.join("Library")).unwrap();
    let opts = UninstallPlanOptions {
        applications_dirs: &[],
        home: &home,
        target_bundle_or_name: None,
        ttl_secs: 900,
    };
    let plan = build_uninstall_plan_for_apps(&catalog, &protection, &opts, &only).unwrap();
    assert!(plan.entries.iter().any(|e| e.path == only[0].app_path));
    assert!(!plan.entries.iter().any(|e| e.path == scanned[1].app_path));
}
```

- [ ] **Step 2: Run 确认红**

Run: `cargo test -p vole-core plan_for_apps_only_includes_selected -- --nocapture`  
Expected: FAIL

- [ ] **Step 3: 重构实现**

将 `build_uninstall_plan_with_brew` 改为：

```rust
let apps = scan_applications(opts.applications_dirs)?;
build_uninstall_plan_for_apps_with_brew(catalog, protection, opts, &apps, brew)
```

抽出共享循环到 `build_uninstall_plan_for_apps_with_brew`。保持既有测试全绿。

- [ ] **Step 4: 测试绿 + Commit**

```bash
cargo test -p vole-core uninstall_plan
git add crates/vole-core/src/ops/uninstall_plan.rs crates/vole-core/src/ops/mod.rs
git commit -m "$(cat <<'EOF'
feat(core): build uninstall plan for selected apps

Extract plan builder over an explicit AppIdentity list so TTY
multi-select can apply without rescanning unrelated apps.
EOF
)"
```

---

### Task 4: CLI — uninstall 双轨接线

**Files:**
- Modify: `crates/vole-cli/src/uninstall.rs`
- Modify: `crates/vole-cli/src/main.rs`（`Uninstall` clap：`permanent` 去掉 `requires = "apply"`；把 `plan`/`dry_run` 传入 opts）

**Interfaces:**
- Extends `UninstallOptions`:
  ```rust
  pub explicit_plan: bool, // --plan || --dry-run
  pub permanent: bool,     // 交互路径可用
  ```
- Produces: `fn should_run_interactive(opts: &UninstallOptions) -> bool`
- Produces: `fn run_interactive(opts: &UninstallOptions) -> io::Result<()>`

- [ ] **Step 1: 写失败 CLI 测**（扩展 `uninstall_cli.rs`）

```rust
#[test]
fn uninstall_help_mentions_interactive_tty() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["uninstall", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.to_lowercase().contains("interactive")
            || stdout.contains("TTY")
            || stdout.contains("多选"),
        "{stdout}"
    );
}

#[test]
fn uninstall_plan_flag_on_piped_stdout_still_json_plan() {
    // 既有 fixture 测保持：--plan --json 成功且含 schema_version
}
```

交互路径的「真 TTY 多选」用单元测覆盖 `should_run_interactive`（可 `pub(crate)`）：

```rust
#[test]
fn interactive_gate_requires_bare_tty_flags() {
    let bare = UninstallOptions { explicit_plan: false, json: false, json_stream: false, plan_out: None, apply_plan: None, permanent: false, target: None };
    // 在测里直接测纯函数条件组合，不依赖真实 TTY：
    assert!(!gate_interactive(false, false, &bare));
    assert!(gate_interactive(true, true, &bare));
    assert!(!gate_interactive(true, true, &UninstallOptions { explicit_plan: true, ..bare }));
}
```

把判定抽成：

```rust
pub(crate) fn gate_interactive(stdin_tty: bool, stdout_tty: bool, opts: &UninstallOptions) -> bool {
    stdin_tty
        && stdout_tty
        && !opts.explicit_plan
        && !opts.json
        && !opts.json_stream
        && opts.plan_out.is_none()
        && opts.apply_plan.is_none()
        && opts.target.is_none()
}
```

- [ ] **Step 2: Run 确认红**

Run: `cargo test -p vole-cli uninstall -- --nocapture`  
Expected: help/gate 断言失败或符号缺失

- [ ] **Step 3: 实现接线**

`run_uninstall_inner`：

```rust
if let Some(ref plan_path) = opts.apply_plan {
    return run_apply(opts, plan_path);
}
if gate_interactive(io::stdin().is_terminal(), io::stdout().is_terminal(), &opts) {
    return run_interactive(&opts);
}
run_plan(opts)
```

`run_interactive`：

1. `scan_applications` + 过滤 `should_protect_from_uninstall` / official uninstaller（与 plan 跳过一致，避免菜单里选了却 0 条目）
2. 构建 `MenuItem`：`label = display_name`；`size_kb = measure_path_size_kb(app_path).ok()`；`epoch = None`（T0；无 date 元数据则 Name 排序）
3. `cfg = MenuConfig { ignore_initial_enter: true, ..MenuConfig::config_from_env() }`；必要时覆盖 `term_height`
4. `run_paginated_select("Select Apps to Remove", items, cfg)?`
5. `Cancelled` → Ok(())
6. `Confirmed(idxs)` 空 → eprintln!("No apps selected"); **loop** 回步骤 4（对齐 mole）
7. 摘要打印到 stderr；`read_line` 确认 `y/Y`
8. `build_uninstall_plan_for_apps(..., &selected_apps)` → `apply_uninstall_plan`（`permanent: opts.permanent`）
9. `print_human_report`

`main.rs` clap：

```rust
/// 永久删除而非移入废纸篓（`--apply` 或交互卸载）。
#[arg(long)]
permanent: bool,
```

去掉 `requires = "apply"`。  
`UninstallOptions { explicit_plan: plan || dry_run, ... }`。  
更新命令 doc comment：TTY 裸调用交互；`--plan` 只产出计划。

- [ ] **Step 4: 测试绿 + Commit**

```bash
cargo test -p vole-cli uninstall
cargo test -p vole-core uninstall_plan
git add crates/vole-cli/src/uninstall.rs crates/vole-cli/src/main.rs crates/vole-cli/tests/uninstall_cli.rs
git commit -m "$(cat <<'EOF'
feat(cli): TTY interactive uninstall via paginated select

Bare uninstall on a TTY multi-selects apps then applies through
the existing plan funnel; --plan/--json/non-TTY stay automated.
EOF
)"
```

---

### Task 5: README + coverage 措辞

**Files:**
- Modify: `README.md`（uninstall 用法：双轨说明）
- Modify: `docs/findings/2026-07-v2-m0-uninstall-spike.md`（「无 TTY 多选 UI」一行改为已落地 / 指向本 design；勿改写历史叙事过度，一句更新即可）

- [ ] **Step 1: 更新 README 片段**

在 uninstall 相关章节写明：

```markdown
# TTY 裸调用：分页多选 → 确认 → 卸载（默认废纸篓；可加 --permanent）
vole uninstall

# 自动化 / 脚本：只产出 plan
vole uninstall --plan --json
vole uninstall --apply /path/to/plan.json
```

- [ ] **Step 2: findings 一行更新**

将 spike 表中「`--plan` / `--json`（无 TTY 多选 UI）」改为注明 T0 已提供 TTY 多选；自动化仍用 plan/json。

- [ ] **Step 3: Commit**

```bash
git add README.md docs/findings/2026-07-v2-m0-uninstall-spike.md
git commit -m "$(cat <<'EOF'
docs: document TTY interactive uninstall dual-track

README and uninstall spike findings now describe bare-TTY
multi-select versus --plan/--json automation.
EOF
)"
```

---

### Task 6: 发版 2.7.0

**Files:**
- Modify: `Cargo.toml`（workspace `version = "2.7.0"`）
- Create: `docs/releases/v2.7.0.md`
- Modify: `Formula/vole.rb` / `README.md` 版本钉（按仓内既有发版惯例；若 sha256 需 release 资产后再 pin，本 Task 至少 bump crate 版本与 release notes）

- [ ] **Step 1: 写 `docs/releases/v2.7.0.md`**

要点：

- TTY 裸 `vole uninstall`：分页多选 → 确认 → apply
- 共享 `PaginatedMultiSelect`（后续 purge/installer 复用）
- `--plan` / `--json` / 非 TTY 行为不变
- `--permanent` 可用于交互路径

- [ ] **Step 2: bump 版本 + 测**

```bash
# 编辑 Cargo.toml version = "2.7.0"
cargo test -p vole-cli
cargo test -p vole-core uninstall_plan
```

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml docs/releases/v2.7.0.md README.md Formula/vole.rb
git commit -m "$(cat <<'EOF'
chore(release): 2.7.0 interactive uninstall TUI

Ship paginated multi-select uninstall on TTY and note dual-track
automation paths in release notes.
EOF
)"
```

---

### Task 7: 自检闸门（合入前）

- [ ] **Step 1: 对照 design §6**

| 验收项 | 证据 |
|---|---|
| MenuContract A | `cargo test -p vole-cli menu_state` |
| 取消不删 / 空选重入 | `run_interactive` 逻辑 + 可选手工 TTY |
| `--plan`/非 TTY 不变 | `uninstall_cli` 既有测 |
| `--permanent` 交互可用 | clap 无 `requires=apply`；help 文案 |
| TerminalGuard | `run_paginated_select` 使用既有 guard |
| 文档双轨 | README + v2.7.0 |

- [ ] **Step 2: 全量相关测**

```bash
cargo test -p vole-cli
cargo test -p vole-core uninstall
cargo fmt --all -- --check
```

- [ ] **Step 3: 开 PR（若执行阶段需要）**

标题建议：`feat(cli): TTY paginated uninstall (2.7.0)`  
Body 链到 design + 本 plan；Test plan 勾选上表。

---

## Spec coverage (self-review)

| Spec 要求 | Task |
|---|---|
| PaginatedMultiSelect 契约 A | Task 1–2 |
| drain / TerminalGuard | Task 2 |
| `VOLE_MENU_*` / 可选 `MOLE_MENU_*` | Task 1 `config_from_env` |
| uninstall 双轨门控 | Task 4 |
| 内存 ProtoPlan → apply | Task 3–4 |
| app 粒度候选 | Task 4 |
| `--permanent` 交互 | Task 4 |
| README / 去掉「无 TTY 多选」长尾 | Task 5 |
| MINOR 发版 | Task 6 |
| T1–T4 不做 | 无 Task（刻意） |

## Placeholder scan

无 TBD/TODO；类型名在 Task 1 定义，后续 Task 引用一致。
