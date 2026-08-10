# T6：`clean` / `optimize` TTY 确认双轨 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（本仓默认 inline）或 wukong-code:subagent-driven-development。Steps 用 checkbox（`- [ ]`）跟踪。

**Goal:** TTY 裸 `vole clean` / `vole optimize` 走扫 plan → 人类摘要 → `Proceed? [y/N]`（默认 N）→ 既有 apply；自动化路径零破坏；发版 **2.10.0**。

**Architecture:** 镜像 uninstall/purge 的 `gate_interactive` 双轨，但**不做**分页多选。确认后内存 plan 调用 `apply_proto_plan` / `apply_optimize_plan`。UI 与门控仅在 `vole-cli`。

**Tech Stack:** Rust 1.97.1、既有 `vole-core` plan/apply、clap、`IsTerminal`。

**Design:** [`../specs/2026-08-10-1813-tui-t6-clean-optimize-confirm-design.md`](../specs/2026-08-10-1813-tui-t6-clean-optimize-confirm-design.md)

## Global Constraints

- Mole 钉版：`third_party/mole-1.48.1`（确认文案参照；mole optimize 已去确认，vole 仍保留 `[y/N]`）
- 删除只走既有 apply 漏斗；禁止平行 `rm`
- **不 bump** `schema_version`；包版本 **2.10.0**（相对 `2.9.0`）
- 不做 `optimize --whitelist`（T8）；不做 clean 分页多选
- TDD：先红再绿；每 Task 一次 commit
- 合入用 merge commit（非 squash）；CI 绿后按 `pr-auto-merge-when-ci-green` 自动合并
- 测试：`VOLE_TEST_NO_AUTH=1`；禁止交互路径挂真 sudo / Touch ID
- 本 plan 范围仅 T6

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-cli/src/clean.rs` | `explicit_plan`、`gate_interactive`、`run_interactive`、接线 |
| `crates/vole-cli/src/optimize.rs` | 同上 |
| `crates/vole-cli/src/main.rs` | 传 `explicit_plan`；`--permanent` 放宽；help/`after_help` |
| `crates/vole-cli/tests/interactive_cli.rs` | help 断言改为 T6 语义 |
| `crates/vole-cli/tests/clean_confirm_cli.rs`（可选） | 非 TTY / `--plan` 仍只 plan |
| `README.md` / `docs/releases/v2.10.0.md` / `Cargo.toml` / `Formula/vole.rb` | 文档与版本 |

---

### Task 1: `clean` 门控 `gate_interactive`（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/clean.rs`
- Modify: `crates/vole-cli/src/main.rs`（仅 `CleanOptions` 增加 `explicit_plan` 传参；本 Task 可不接交互）

**Interfaces:**
- Produces:
  - `CleanOptions { explicit_plan: bool, …既有字段 }`
  - `pub(crate) fn gate_interactive(stdin_tty: bool, stdout_tty: bool, opts: &CleanOptions) -> bool`
  - 真值条件：`stdin_tty && stdout_tty && !explicit_plan && !json && !json_stream && plan_out.is_none() && apply_plan.is_none() && !opts.is_whitelist_command()`

- [ ] **Step 1: 写失败单测**（`clean.rs` 内 `#[cfg(test)]`）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn bare_opts() -> CleanOptions {
        CleanOptions {
            explicit_plan: false,
            json: false,
            json_stream: false,
            plan_out: None,
            apply_plan: None,
            permanent: false,
            whitelist: false,
            whitelist_add: None,
            whitelist_remove: None,
            whitelist_list: false,
        }
    }

    #[test]
    fn interactive_gate_requires_bare_tty_flags() {
        let bare = bare_opts();
        assert!(!gate_interactive(false, false, &bare));
        assert!(gate_interactive(true, true, &bare));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                explicit_plan: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                json: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                apply_plan: Some(PathBuf::from("p.json")),
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                whitelist: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &CleanOptions {
                whitelist_list: true,
                ..bare_opts()
            }
        ));
    }
}
```

- [ ] **Step 2: 跑测确认失败**

Run: `VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib clean::tests::interactive_gate_requires_bare_tty_flags`
Expected: FAIL（`gate_interactive` / `explicit_plan` 未定义）

- [ ] **Step 3: 最小实现**

在 `CleanOptions` 加 `pub explicit_plan: bool`。实现：

```rust
pub(crate) fn gate_interactive(stdin_tty: bool, stdout_tty: bool, opts: &CleanOptions) -> bool {
    stdin_tty
        && stdout_tty
        && !opts.explicit_plan
        && !opts.json
        && !opts.json_stream
        && opts.plan_out.is_none()
        && opts.apply_plan.is_none()
        && !opts.is_whitelist_command()
}
```

`main.rs` Clean 分支传 `explicit_plan: plan || dry_run`（并把 `plan: _` / `dry_run: _` 改为绑定 `plan` / `dry_run`）。

本 Task **还不**改 `run_clean_inner` 分支（仍总走 `run_plan`）——单测只测纯函数。

- [ ] **Step 4: 跑测确认通过**

Run: `VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib clean::tests::interactive_gate_requires_bare_tty_flags`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/clean.rs crates/vole-cli/src/main.rs
git commit -m "$(cat <<'EOF'
feat(cli): add clean interactive gate for T6

Pure gate_interactive for bare-TTY confirm track; no apply yet.
EOF
)"
```

---

### Task 2: `clean` 确认轨 `run_interactive` + 接线

**Files:**
- Modify: `crates/vole-cli/src/clean.rs`
- Modify: `crates/vole-cli/src/main.rs`（`--permanent` 去掉 `requires = "apply"`；Clean help 文案）

**Interfaces:**
- Consumes: Task 1 `gate_interactive`；既有 `Orchestrator::build_plan` / `plan_to_proto` / `apply_proto_plan` / `print_human_plan` / `print_human_hints` / `print_human_report`
- Produces: `fn run_interactive(opts: &CleanOptions) -> io::Result<()>`；`run_clean_inner` 在 apply 之后、`run_plan` 之前调用门控

- [ ] **Step 1: 写失败 CLI/help 断言（或扩展 lib 测）**

在 `main.rs` Clean 的 doc comment / `--help` 期望出现 `TTY` 与确认语义。先改 `interactive_cli.rs` 顶层 help 断言为 T6（本步可先让其失败）：

```rust
// crates/vole-cli/tests/interactive_cli.rs — 替换 plan-only 断言
assert!(
    stdout.to_lowercase().contains("confirm")
        || stdout.contains("Proceed")
        || stdout.to_lowercase().contains("tty"),
    "expected T6 confirm-track mention in help: {stdout}"
);
assert!(
    !stdout.contains("plan-only until"),
    "stale T5 caveat still in help: {stdout}"
);
```

同时可加：

```rust
#[test]
fn clean_help_mentions_tty_confirm() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["clean", "--help"])
        .output()
        .expect("help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.to_lowercase().contains("tty") || stdout.contains("Proceed"),
        "clean help={stdout}"
    );
}
```

- [ ] **Step 2: 跑测确认失败**

Run: `VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test interactive_cli`
Expected: FAIL（仍含 plan-only / 缺 confirm）

- [ ] **Step 3: 实现 `run_interactive` 并接线**

`run_clean_inner`：

```rust
if let Some(ref plan_path) = opts.apply_plan {
    return run_apply(&opts, plan_path);
}
if gate_interactive(io::stdin().is_terminal(), io::stdout().is_terminal(), &opts) {
    return run_interactive(&opts);
}
run_plan(opts)
```

`run_interactive` 要点（复用 `run_plan` 的扫规则 / whitelist / protection；**无** json_stream）：

1. `load_rules` + `whitelist::load_clean` + `AppProtection` + `Orchestrator::new(cancel, None)` → `build_plan`
2. `entries.is_empty()` → `eprintln!("Nothing to clean."); return Ok(())`
3. `plan_to_proto`；`print_human_plan` + `print_human_hints`（coverage/hints 与 plan 路径一致）
4. `eprint!("Proceed with clean? [y/N] ");` flush stderr；`read_line`；非 `y`/`Y` → `Aborted.`
5. `apply_proto_plan(&proto, …, ApplyPlanOptions { permanent: opts.permanent }, …, None)` → `print_human_report`

`main.rs`：Clean 命令 about 改为类似 uninstall：

```rust
/// 清理缓存与残留文件。
///
/// TTY 裸调用：扫 plan → 确认 → apply；`--plan` / `--json` / 非 TTY 只产出计划。
```

`--permanent`：去掉 `requires = "apply"`（与 uninstall 一致）。

更新 `after_help`：去掉 “plan-only until the confirm track ships”，改为说明 TTY 确认执行。

- [ ] **Step 4: 跑测确认通过**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib clean::tests
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test interactive_cli
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test clean_hints_cli
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test clean_apply_stream
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/clean.rs crates/vole-cli/src/main.rs crates/vole-cli/tests/interactive_cli.rs
git commit -m "$(cat <<'EOF'
feat(cli): TTY confirm-then-apply for vole clean

Bare TTY scans a plan, prompts Proceed? [y/N], then apply_proto_plan.
EOF
)"
```

---

### Task 3: `optimize` 门控 + 确认轨（红→绿）

**Files:**
- Modify: `crates/vole-cli/src/optimize.rs`
- Modify: `crates/vole-cli/src/main.rs`

**Interfaces:**
- Produces:
  - `OptimizeOptions { explicit_plan: bool, … }`
  - `gate_interactive(stdin_tty, stdout_tty, &OptimizeOptions) -> bool`（无 whitelist；`--task` **不**挡门控）
  - `run_interactive`：`build_optimize_plan` → 摘要 → `Proceed with optimize? [y/N]` → `apply_optimize_plan`

- [ ] **Step 1: 写失败单测**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn bare_opts() -> OptimizeOptions {
        OptimizeOptions {
            explicit_plan: false,
            json: false,
            json_stream: false,
            plan_out: None,
            apply_plan: None,
            permanent: false,
            task: None,
        }
    }

    #[test]
    fn interactive_gate_requires_bare_tty_flags() {
        let bare = bare_opts();
        assert!(!gate_interactive(false, false, &bare));
        assert!(gate_interactive(true, true, &bare));
        assert!(!gate_interactive(
            true,
            true,
            &OptimizeOptions {
                explicit_plan: true,
                ..bare_opts()
            }
        ));
        assert!(!gate_interactive(
            true,
            true,
            &OptimizeOptions {
                json_stream: true,
                ..bare_opts()
            }
        ));
        assert!(gate_interactive(
            true,
            true,
            &OptimizeOptions {
                task: Some("dns_flush".into()),
                ..bare_opts()
            }
        ));
    }
}
```

另加 `optimize --help` CLI 断言（可放 `interactive_cli.rs`）。

- [ ] **Step 2: 跑测确认失败**

Run: `VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib optimize::tests::interactive_gate_requires_bare_tty_flags`
Expected: FAIL

- [ ] **Step 3: 实现**

`run_optimize_inner` 在 apply 后门控；`run_interactive`：

1. `build_optimize_plan`（尊重 `opts.task`）
2. 空 → `eprintln!("Nothing to optimize."); return Ok(())`
3. `print_human_plan(&plan)`
4. `Proceed with optimize? [y/N]`
5. `apply_optimize_plan` + `print_human_report`

`main.rs`：传 `explicit_plan: plan || dry_run`；Optimize about 双轨文案；`--permanent` 放宽。

- [ ] **Step 4: 跑测确认通过**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib optimize::tests
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test interactive_cli
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-cli/src/optimize.rs crates/vole-cli/src/main.rs crates/vole-cli/tests/interactive_cli.rs
git commit -m "$(cat <<'EOF'
feat(cli): TTY confirm-then-apply for vole optimize

Mirror clean dual-track: plan summary, Proceed? [y/N], apply_optimize_plan.
EOF
)"
```

---

### Task 4: README + 自动化回归断言

**Files:**
- Modify: `README.md`
- Modify: `crates/vole-cli/tests/interactive_cli.rs`（若尚有遗漏）
- Optional Create: `crates/vole-cli/tests/clean_confirm_cli.rs` — 非 TTY `vole clean` 仍成功且 stdout/stderr 含 Plan（不挂确认）

- [ ] **Step 1: 更新 README（失败定义=文档仍写 T6 未交付）**

替换要点：

1. 删掉「Clean/Optimize still plan-only until T6」类句子
2. TUI 表增加：

| `vole clean` | 扫 plan → `Proceed? [y/N]` → apply | `--plan` / `--apply` / `--json*` / whitelist 系 / 非 TTY |
| `vole optimize` | 同上 | `--plan` / `--apply` / `--json*` / 非 TTY |

3. 「先预览再执行」改为：TTY 裸 clean/optimize 为确认后执行；脚本请显式 `--plan` / `--apply`
4. 成熟度行：去掉「余项：Clean/Optimize 确认双轨（T6）」；版本改 **2.10.0**（可与 Task 5 同改，本 Task 至少改语义句）
5. 示例：`vole clean` / `vole optimize` 注明 TTY 确认

非 TTY 回归测（推荐）：

```rust
// crates/vole-cli/tests/clean_confirm_cli.rs
use std::process::{Command, Stdio};

#[test]
fn clean_non_tty_stays_plan_only() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["clean"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("VOLE_TEST_NO_AUTH", "1")
        .output()
        .expect("run");
    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("Plan:") || combined.contains("candidate"),
        "expected plan output, got {combined}"
    );
    assert!(!combined.contains("Proceed with clean?"), "must not prompt on non-TTY");
}
```

同类可对 `optimize` 断言 `optimize plan:` 且无 `Proceed with optimize?`。

- [ ] **Step 2: 跑测**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test clean_confirm_cli
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --test interactive_cli
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add README.md crates/vole-cli/tests/clean_confirm_cli.rs crates/vole-cli/tests/interactive_cli.rs
git commit -m "$(cat <<'EOF'
docs(cli): document T6 clean/optimize confirm track

Replace T5 plan-only caveats; add non-TTY plan-only CLI regression.
EOF
)"
```

---

### Task 5: 版本 2.10.0 + release notes + Formula 占位

**Files:**
- Modify: `Cargo.toml`（workspace `version = "2.10.0"`）
- Modify: `Cargo.lock`（随 build）
- Create: `docs/releases/v2.10.0.md`
- Modify: `Formula/vole.rb`（version 2.10.0；sha256 可先占位，tag 后 `update-homebrew-formula`）
- Modify: `README.md` 版本号引用（若 Task 4 未改完）

- [ ] **Step 1: 写 `docs/releases/v2.10.0.md`**

结构对齐 `v2.9.0.md`：相对 2.9.0 的 T6 确认双轨、有意不做（T7/T8）、验收命令、Formula 说明。

- [ ] **Step 2: bump 版本并同步 Formula version 字段**

```bash
# Cargo.toml version = "2.10.0"
# cargo update -p vole-cli 或一次 cargo test 刷新 lock
```

- [ ] **Step 3: 验证**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli
./scripts/check-command-surface.sh --enforce
cargo fmt --all -- --check
```

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock docs/releases/v2.10.0.md Formula/vole.rb README.md
git commit -m "$(cat <<'EOF'
chore(release): bump to 2.10.0 for T6 confirm track

Document clean/optimize TTY confirm-then-apply; Formula sha pending tag.
EOF
)"
```

---

### Task 6: 收口验证 + PR（执行 finishing skill）

**Files:** 无新代码（除非 CI/fmt 修补）

- [ ] **Step 1: 全量相关验证**

```bash
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib clean::tests
VOLE_TEST_NO_AUTH=1 cargo test -p vole-cli --lib optimize::tests
./scripts/check-command-surface.sh --enforce
cargo fmt --all -- --check
```

- [ ] **Step 2: 按 finishing-a-development-branch → Option 2 开 PR**

Push `feat/t6-clean-optimize-confirm`；`gh pr create`；CI 绿后 `gh pr merge --merge --delete-branch`。

- [ ] **Step 3: 发版运营（合入后）**

对齐 2.9.0：annotated tag `v2.10.0` → 等 Release assets → `bash scripts/update-homebrew-formula.sh 2.10.0` → Formula PR merge。

---

## Self-Review

| Spec 要求 | Task |
|---|---|
| clean 门控（含 whitelist 排除） | Task 1 |
| clean plan→确认→`apply_proto_plan` | Task 2 |
| optimize 门控 + 确认→`apply_optimize_plan` | Task 3 |
| `--permanent` 交互可用 | Task 2/3 |
| 自动化路径不变 | Task 2/3/4 回归 |
| README/help 去 T5 caveat | Task 2/4 |
| 2.10.0 / 不 bump schema | Task 5 |
| 无分页多选 / 无 optimize --whitelist | 无对应 Task（有意不做） |

无 TBD；确认文案固定为 `Proceed with clean?` / `Proceed with optimize?`。
