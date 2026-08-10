# T8：status 抛光 + optimize whitelist Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（本仓默认 inline）或 wukong-code:subagent-driven-development。Steps 用 checkbox（`- [ ]`）跟踪。

**Goal:** 交付 `optimize --whitelist`（mole 独立任务清单）、status 动画 cat + `k`/`c` prefs、README/对照表收口，发版 **2.12.0**。

**Architecture:** optimize whitelist 落在 `vole-core::whitelist`（独立文件 `~/.config/mole/whitelist_optimize`，行=任务 id），plan 阶段按 id 过滤，apply 双保险跳过；CLI 复用 clean 的 `PaginatedMultiSelect` 壳。status cat/prefs 为 `vole-cli` 纯逻辑 + `cmd_status_tui` 键位，prefs 写 `~/.config/mole/status_prefs`。

**Tech Stack:** Rust 1.97.1、ratatui、crossterm、clap、既有 `PaginatedMultiSelect`。

**Design:** [`../specs/2026-08-10-2012-tui-t8-status-polish-optimize-whitelist-design.md`](../specs/2026-08-10-2012-tui-t8-status-polish-optimize-whitelist-design.md)

## Global Constraints

- Mole 钉版：`third_party/mole-1.48.1`
- **不 bump** `schema_version`；包版本 **2.12.0**（相对 `2.11.0`）
- footer 只声明已接线键；cat/`k`/`c` 整包交付或整包不做（见 design §3.3）
- TDD：先红再绿；每 Task 一次 commit
- 合入用 merge commit；CI 绿后按 `pr-auto-merge-when-ci-green` 自动合并
- 测试：`VOLE_TEST_NO_AUTH=1`
- optimize whitelist **任务 id**（非路径）；与 clean path whitelist 互不污染
- 既有 `OptimizeApplyContext.whitelist_patterns` 仍是 **路径** 白名单（给 `mole_delete`）；任务白名单用新字段 `task_whitelist: &[String]`

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/whitelist/mod.rs` | optimize load/save/add/remove/list/menu + `is_task_whitelisted` |
| `crates/vole-core/src/ops/optimize_plan.rs` | `OptimizePlanOptions.task_whitelist`；`allow()` 排除 |
| `crates/vole-core/src/ops/optimize_apply.rs` | apply 前 task id 在 whitelist → `SkipReason::Whitelisted` |
| `crates/vole-cli/src/optimize.rs` / `main.rs` | flags + 接线 + gate |
| `crates/vole-cli/tests/optimize_cli.rs` | help / 非 TTY whitelist |
| `crates/vole-cli/src/tui/status_prefs.rs` | 新建：prefs + cpu cycle 纯逻辑 |
| `crates/vole-cli/src/tui/status_cat.rs` | 新建：帧 + 位移渲染纯逻辑 |
| `crates/vole-cli/src/tui/status_view.rs` / `widgets.rs` / `main.rs` | cat 区、cpu 截断、footer、键位 |
| `README.md` / `docs/releases/v2.12.0.md` / `Cargo.toml` / `Formula/vole.rb` | 文档与版本 |

---

### Task 1: optimize whitelist core API（红→绿）

**Files:**
- Modify: `crates/vole-core/src/whitelist/mod.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Produces:
  - `pub fn optimize_config_path() -> PathBuf` → `$HOME/.config/mole/whitelist_optimize`
  - `pub fn load_optimize() -> io::Result<Vec<String>>`
  - `pub fn save_optimize(ids: &[String]) -> io::Result<()>`
  - `pub fn add_optimize(task_id: &str) -> io::Result<()>`（空/未知 id → `InvalidInput`；未知=不在 `optimize_catalog()`）
  - `pub fn remove_optimize(task_id: &str) -> io::Result<bool>`
  - `pub fn is_task_whitelisted(task_id: &str, ids: &[String]) -> bool`（精确匹配）
  - `pub fn build_optimize_whitelist_menu(current: &[String]) -> WhitelistMenuBuild`（entries=catalog 全量，label=title，pattern=id；已选置顶）
  - `pub fn optimize_config_display_path() -> String`
  - Header 常量：`# Mole Optimize Whitelist - Listed tasks are skipped\n# One task id per line\n`

- [ ] **Step 1: 写失败单测**

```rust
#[test]
fn optimize_whitelist_roundtrip_and_menu() {
    let _guard = test_env::lock();
    let home = std::env::temp_dir().join(format!("vole-owl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("h")).unwrap();
    std::env::set_var("HOME", home.join("h"));

    assert!(load_optimize().unwrap().is_empty());
    add_optimize("dock_refresh").unwrap();
    let err = add_optimize("not_a_real_task").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    let loaded = load_optimize().unwrap();
    assert_eq!(loaded, vec!["dock_refresh".to_string()]);
    assert!(is_task_whitelisted("dock_refresh", &loaded));
    assert!(!is_task_whitelisted("cache_refresh", &loaded));

    let menu = build_optimize_whitelist_menu(&loaded);
    assert_eq!(menu.entries[0].pattern, "dock_refresh");
    assert_eq!(menu.preselected, vec![0]);
    assert!(menu.entries.iter().any(|e| e.pattern == "cache_refresh"));

    assert!(remove_optimize("dock_refresh").unwrap());
    assert!(load_optimize().unwrap().is_empty());

    std::env::remove_var("HOME");
    let _ = std::fs::remove_dir_all(&home);
}
```

- [ ] **Step 2: Run → FAIL**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-core optimize_whitelist_roundtrip -- --nocapture
```

Expected: FAIL（符号未定义）

- [ ] **Step 3: 最小实现**（与 clean API 并列；配置路径 `whitelist_optimize`；`add_optimize` 校验 `optimize_catalog().iter().any(|t| t.id == id)`）

- [ ] **Step 4: Run → PASS**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-core optimize_whitelist -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/whitelist/mod.rs
git commit -m "$(cat <<'EOF'
feat(whitelist): add optimize task whitelist store

Persist mole-compatible ~/.config/mole/whitelist_optimize by task id.
EOF
)"
```

---

### Task 2: plan/apply 跳过白名单任务（红→绿）

**Files:**
- Modify: `crates/vole-core/src/ops/optimize_plan.rs`
- Modify: `crates/vole-core/src/ops/optimize_apply.rs`
- Modify: 所有构造 `OptimizePlanOptions { ... }` 的调用点（加 `task_whitelist: &[]` 或实参）

**Interfaces:**
- Consumes: `whitelist::is_task_whitelisted`
- Produces:
  - `OptimizePlanOptions<'a> { home, ttl_secs, only_task, task_whitelist: &'a [String] }`
  - `allow(task_id)` 额外：`!is_task_whitelisted(task_id, opts.task_whitelist)`
  - `OptimizeApplyContext` 增加 `task_whitelist: &'a [String]`；在 `parse_optimize_rule_id` 成功后、执行前：若 whitelisted → skip `Whitelisted`

- [ ] **Step 1: 写失败单测**（`optimize_plan.rs`）

```rust
#[test]
fn build_plan_skips_whitelisted_task_ids() {
    let home = tempfile_home(); // 或既有测试 helper
    let catalog = ProtectionCatalog::embedded();
    let protection = AppProtection::new();
    let wl = vec!["dock_refresh".to_string()];
    let plan = build_optimize_plan(
        &catalog,
        &protection,
        &OptimizePlanOptions {
            home: &home,
            ttl_secs: 900,
            only_task: None,
            task_whitelist: &wl,
        },
    )
    .unwrap();
    assert!(!plan
        .entries
        .iter()
        .any(|e| e.rule_id.contains("dock_refresh")));
}
```

（若 `tempfile_home` 不存在，复用同文件既有测试的 home 构造方式。）

- [ ] **Step 2: Run → FAIL**（缺字段 / 不过滤）

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-core build_plan_skips_whitelisted -- --nocapture
```

- [ ] **Step 3: 实现字段 + 过滤 + apply 双保险；修编译断点**

- [ ] **Step 4: Run → PASS**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-core --lib ops::optimize_plan
VOLE_TEST_NO_AUTH=1 cargo test -p vole-core --lib ops::optimize_apply
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/optimize_plan.rs crates/vole-core/src/ops/optimize_apply.rs crates/vole-cli/src/optimize.rs
git commit -m "$(cat <<'EOF'
feat(optimize): skip whitelisted task ids in plan/apply

Honor task_whitelist at plan discovery and as apply defense-in-depth.
EOF
)"
```

---

### Task 3: CLI `--whitelist*` + gate（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/main.rs`（`Command::Optimize` 加四 flag，分发）
- Modify: `crates/vole-cli/src/optimize.rs`（`OptimizeOptions` + `run_whitelist` + `gate_interactive`）
- Modify: `crates/vole-cli/tests/optimize_cli.rs`

**Interfaces:**
- Produces: 与 clean 同形交互壳（标题含 `optimize_config_display_path()`；保存 `save_optimize`）
- `gate_interactive`：增加 `!opts.is_whitelist_command()`

- [ ] **Step 1: 写/扩 CLI 测试**

```rust
#[test]
fn optimize_help_mentions_whitelist() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["optimize", "--help"])
        .output()
        .expect("help");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("whitelist"), "{stdout}");
}

#[test]
fn optimize_whitelist_list_add_remove_non_tty() {
    let home = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_vole");
    let status = std::process::Command::new(bin)
        .env("HOME", home.path())
        .env("VOLE_TEST_NO_AUTH", "1")
        .args(["optimize", "--whitelist-add", "dock_refresh"])
        .status()
        .unwrap();
    assert!(status.success());
    let out = std::process::Command::new(bin)
        .env("HOME", home.path())
        .env("VOLE_TEST_NO_AUTH", "1")
        .args(["optimize", "--whitelist-list"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("dock_refresh"), "{stdout}");
}
```

（若仓未依赖 `tempfile` 于 cli tests，用 `std::env::temp_dir()` + 唯一子目录，对齐 `whitelist_cli.rs`。）

- [ ] **Step 2: Run → FAIL**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test optimize_cli optimize_help_mentions_whitelist -- --nocapture
```

- [ ] **Step 3: 接线 flags + `run_whitelist` + plan/interactive/apply 加载 `load_optimize()` 传入 `task_whitelist`**

注意：路径型 `whitelist_patterns`（给 delete）保持 `&[]` 或既有语义；**不要**把任务 id 塞进路径 whitelist。

- [ ] **Step 4: Run → PASS**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test optimize_cli
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib gate_interactive
./scripts/check-command-surface.sh --enforce
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/main.rs crates/vole-cli/src/optimize.rs crates/vole-cli/tests/optimize_cli.rs
git commit -m "$(cat <<'EOF'
feat(cli): wire optimize --whitelist management flags

TTY paginated manager plus add/remove/list; gate skips confirm track.
EOF
)"
```

---

### Task 4: status prefs + cpu cycle（红→绿）

**Files:**
- Create: `crates/vole-cli/src/tui/status_prefs.rs`
- Modify: `crates/vole-cli/src/tui/mod.rs`

**Interfaces:**
- Produces:
  - `pub struct StatusPrefs { pub cat_hidden: bool, pub cpu_cores: i32 }`（`0` = all；默认 `cpu_cores = 2`）
  - `pub fn load_status_prefs() -> StatusPrefs`
  - `pub fn save_cat_hidden(hidden: bool)`
  - `pub fn save_cpu_cores(n: i32)`
  - `pub fn next_cpu_cores(current: i32) -> i32` // cycle `[2,4,8,0]`
  - `pub fn smaller_cpu_cores(current: i32) -> i32` // 不环绕
  - 路径：`$HOME/.config/mole/status_prefs`

- [ ] **Step 1: 失败单测**

```rust
#[test]
fn next_cpu_cores_cycles() {
    assert_eq!(next_cpu_cores(2), 4);
    assert_eq!(next_cpu_cores(4), 8);
    assert_eq!(next_cpu_cores(8), 0);
    assert_eq!(next_cpu_cores(0), 2);
    assert_eq!(next_cpu_cores(99), 2);
}

#[test]
fn status_prefs_roundtrip() {
    let _guard = /* 若有 env lock 用；否则唯一 HOME */;
    // set HOME temp, save_cat_hidden(true), save_cpu_cores(8),
    // load → cat_hidden && cpu_cores==8
}
```

- [ ] **Step 2–4: 红→实现→绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::status_prefs
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/tui/status_prefs.rs crates/vole-cli/src/tui/mod.rs
git commit -m "$(cat <<'EOF'
feat(status): add mole-compatible status_prefs store

Persist cat_hidden and cpu_cores under ~/.config/mole/status_prefs.
EOF
)"
```

---

### Task 5: status cat 帧 + 视图/footer/键位（红→绿）

**Files:**
- Create: `crates/vole-cli/src/tui/status_cat.rs`
- Modify: `crates/vole-cli/src/tui/status_view.rs`
- Modify: `crates/vole-cli/src/tui/widgets.rs`（`status_footer` → 含 `K Cat | C Cores | …`）
- Modify: `crates/vole-cli/src/tui/mod.rs`
- Modify: `crates/vole-cli/src/main.rs`（`cmd_status_tui`）

**Interfaces:**
- Produces:
  - `pub fn render_mole_frame(anim_frame: u64, term_width: usize) -> String`（移植 mole 四帧 + mirror + 水平往返）
  - `render_status(..., opts: StatusRenderOpts { cat_hidden, anim_frame, cpu_cores })`
  - CPU 卡：`per_core` 排序后 `take(limit)`，`limit = if cpu_cores==0 { usize::MAX } else { cpu_cores as usize }`
  - 每 tick：`anim_frame = anim_frame.wrapping_add(1 + (cpu_usage/25.0) as u64)`（近似 mole 加速）
  - 键：`k` toggle+save；`c` cycle+save；大小写不敏感

- [ ] **Step 1: 失败单测**

```rust
#[test]
fn mole_frame_contains_ears_and_moves() {
    let a = render_mole_frame(0, 80);
    let b = render_mole_frame(10, 80);
    assert!(a.contains("/\\_/\\"));
    assert_ne!(a, b);
}

#[test]
fn status_footer_declares_cat_and_cores() {
    let f = status_footer();
    assert!(f.contains('K') || f.contains("Cat"));
    assert!(f.contains('C') || f.contains("Cores"));
    assert!(f.contains('Q'));
}
```

- [ ] **Step 2–4: 红→移植帧→改 render/CPU take→接线 main→绿**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::status_cat
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::status_view
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib tui::widgets
VOLE_TEST_NO_AUTH=1 cargo build -p vole-cli
```

若窄屏/布局无法稳定验收 → 按 design §3.3 **整包撤销** cat/`k`/`c`，在 release notes 标 won't-do；本 Task commit 改为文档说明，**不得** footer 虚标。

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/tui/ crates/vole-cli/src/main.rs
git commit -m "$(cat <<'EOF'
feat(status): animate mole cat with k/c prefs

Render walking ASCII cat; toggle visibility and cycle CPU core rows.
EOF
)"
```

---

### Task 6: 文档 + 版本 2.12.0

**Files:**
- Modify: `README.md`（TUI 表、成熟度、去掉 T8 余项措辞）
- Create: `docs/releases/v2.12.0.md`
- Modify: `Cargo.toml` → `2.12.0`；同步 `Cargo.lock`
- Modify: `Formula/vole.rb`（version；sha256 待 tag 后脚本）

- [ ] **Step 1: 写 release notes（中文）** — whitelist、cat/`k`/`c`（或 won't-do）、文档收口、不 bump schema、验收命令

- [ ] **Step 2: README 收口** — 清除「有意未接线 status cat / optimize --whitelist」「余项 T8」；status/optimize 行与表对齐交付物

- [ ] **Step 3: 验证**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-core -p vole-cli
./scripts/check-command-surface.sh --enforce
cargo fmt --all -- --check
```

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock README.md docs/releases/v2.12.0.md Formula/vole.rb
git commit -m "$(cat <<'EOF'
chore(release): bump to 2.12.0 for T8 closeout

Document optimize whitelist and status cat prefs; Formula sha pending tag.
EOF
)"
```

---

### Task 7: 验证 + PR + 发版运营

**Files:** 无新代码（除非 CI/fmt 修补）

- [ ] **Step 1: verification-before-completion**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-core -p vole-cli
./scripts/check-command-surface.sh --enforce
./scripts/check-protocol-doc.sh
./scripts/check-license.sh
cargo fmt --all -- --check
```

Expected: 全部 PASS 后才宣称完成。

- [ ] **Step 2: finishing-a-development-branch → PR**

分支已是 `feat/t8-status-polish-optimize-whitelist`；push；`gh pr create`；CI 绿且 MERGEABLE → `gh pr merge --merge --delete-branch`。

- [ ] **Step 3: 发版运营（合入后）**

annotated tag `v2.12.0` → 等 Release assets → `bash scripts/update-homebrew-formula.sh 2.12.0` → Formula sha PR merge（对齐 #129）。

---

## Self-Review

| Spec 要求 | Task |
|---|---|
| optimize `--whitelist` 独立清单 | Task 1–3 |
| plan/apply 跳过任务 id | Task 2 |
| CLI 双轨 + gate | Task 3 |
| 动画 cat | Task 5 |
| `k` / `c` prefs | Task 4–5 |
| 诚实 footer | Task 5 |
| README / 对照表收口 | Task 6 |
| 2.12.0 / 不 bump schema | Task 6 |
| cat 整包或 won't-do | Task 5 闸门 |
| VOLE_TEST_NO_AUTH | 全局 |

无 TBD；路径 whitelist 与任务 whitelist 字段分离已写明。
