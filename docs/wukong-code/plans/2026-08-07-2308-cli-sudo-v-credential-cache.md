# CLI sudo -v Credential Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** TTY 下特权 apply 可至多一次 `sudo -v` 缓存凭证，随后仍用 `sudo -n` 删除（1.26.0）。

**Architecture:** 扩展 `PrivilegeBackend::acquire_interactive`；`ApplyPlanContext` 增加 latch + `ensure_privilege_ready()`，统一 15 处 `probe_noninteractive` 调用。非 TTY / `VOLE_TEST_NO_AUTH` 零行为变化。

**Tech Stack:** Rust / macOS / vole-core privilege + apply_plan / IsTerminal

## Global Constraints

- 版本：**1.26.0**（MINOR）；规则数不变；**不 bump** `schema_version`
- 删除永远 `sudo -n`；plan 零 sudo；无 SMAppService
- 合并：`gh pr merge --merge`（禁止 squash）
- 全程中文进度；task-level commit

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/privilege/mod.rs` | trait 加 `acquire_interactive` 默认 false |
| `crates/vole-core/src/privilege/sudo.rs` | `SudoNoninteractive` / `RecordingPrivilege` 实现 |
| `crates/vole-core/src/ops/apply_plan.rs` | latch + helper；替换 probe 站点 |
| `crates/vole-core/src/ops/coverage.rs` | 覆盖文案 |
| `Cargo.toml` / `Formula/vole.rb` / `README.md` | 版本与对外句 |
| `docs/releases/v1.26.0.md` + findings | 发版 |

---

## Task 1: PrivilegeBackend `acquire_interactive` + 单测

**Files:**
- Modify: `crates/vole-core/src/privilege/mod.rs`
- Modify: `crates/vole-core/src/privilege/sudo.rs`

- [ ] **Step 1: RED** — `RecordingPrivilege` 增加字段 `acquire_calls: Mutex<u32>`、`acquire_ok: bool`；写测：默认 `acquire_interactive` → false 且可计数

- [ ] **Step 2: 跑测确认 RED**

```bash
cargo test -p vole-core recording_acquire -- --nocapture
```

- [ ] **Step 3: GREEN** — trait：

```rust
fn acquire_interactive(&self) -> bool {
    false
}
```

`SudoNoninteractive::acquire_interactive`：
- `test_no_auth()` → false
- `!std::io::stdin().is_terminal()` → false
- `Command::new("sudo").args(["-v"]).status()` success → true

`RecordingPrivilege`：bump `acquire_calls`，返回 `acquire_ok`。

- [ ] **Step 4: 跑测 GREEN**

```bash
cargo test -p vole-core recording_acquire privilege -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/privilege/mod.rs crates/vole-core/src/privilege/sudo.rs
git commit -m "$(cat <<'EOF'
feat(privilege): PrivilegeBackend::acquire_interactive for sudo -v

EOF
)"
```

---

## Task 2: ApplyPlanContext latch + `ensure_privilege_ready`

**Files:**
- Modify: `crates/vole-core/src/ops/apply_plan.rs`

- [ ] **Step 1: RED** — 新测 `ensure_privilege_ready_acquires_once_then_probes`：Recording probe 初 false；`acquire_ok=true`；第一次 ensure → true 且 acquire_calls==1；第二次 ensure（probe 已 true）→ acquire 仍 1

- [ ] **Step 2: 在 `ApplyPlanContext` 加：**

```rust
pub privilege_acquire_attempted: bool, // 或 cell/interior；单线程 apply 可用 bool 字段 mutate
```

注意：`apply_plan` 取 `&mut ApplyPlanContext`，字段可直接写。

```rust
fn ensure_privilege_ready(ctx: &mut ApplyPlanContext<'_>, backend: &dyn PrivilegeBackend) -> bool {
    if backend.probe_noninteractive() {
        return true;
    }
    if ctx.privilege_acquire_attempted {
        return false;
    }
    ctx.privilege_acquire_attempted = true;
    // 可选：TTY 时 eprintln 中文「正在请求管理员权限以清理系统路径…」
    if backend.acquire_interactive() && backend.probe_noninteractive() {
        return true;
    }
    false
}
```

- [ ] **Step 3:** 将全部 `if !backend.probe_noninteractive()` 特权分支改为 `if !ensure_privilege_ready(ctx, backend)`（约 15 处；含 system-services）。`mole_delete` 路径若已在分支外 probe，保持一致：只改 apply_plan 站点。

- [ ] **Step 4: GREEN**

```bash
cargo test -p vole-core ensure_privilege_ready apply_rosetta apply_gpu_metal -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/apply_plan.rs
git commit -m "$(cat <<'EOF'
feat(apply): once-per-apply sudo -v before privilege probe retry

EOF
)"
```

---

## Task 3: 版本 1.26.0 + coverage / README / release

**Files:**
- Modify: `Cargo.toml`, `Formula/vole.rb`, `README.md`
- Modify: `crates/vole-core/src/ops/coverage.rs`
- Create: `docs/releases/v1.26.0.md`
- Create: `docs/findings/2026-08-cli-sudo-v-credential-cache.md`

- [ ] coverage：交互提权句移入「已落地」；仍未移植 → `Install macOS*.app`、桌面 SMAppService（去掉泛「交互提权」若已落）
- [ ] 单测断言同步
- [ ] README / Formula / workspace version **1.26.0**
- [ ] releases + findings（贴验收：TTY vs 非 TTY）

```bash
cargo test -p vole-core coverage -- --nocapture
cargo fmt --all -- --check
```

- [ ] Commit

```bash
git commit -m "$(cat <<'EOF'
chore(release): 1.26.0 CLI sudo -v credential cache

EOF
)"
```

---

## Task 4: PR → security-review → CI → merge commit

- [ ] Push branch `feat/cli-sudo-v-credential-cache`
- [ ] `gh pr create`（标题含 1.26.0）
- [ ] Task security-review（提权面 / 始终 `-n` / latch / 非 TTY）
- [ ] `gh pr checks --watch`；**`gh pr merge --merge --delete-branch`**（禁止 squash）
- [ ] `git checkout main && git pull --ff-only`

---

## Done criteria

1. TTY+过期凭证：特权 apply 至多一次密码，删走 `sudo -n`
2. CI/pipe/`VOLE_TEST_NO_AUTH`：零 `sudo -v`
3. coverage 反映交互提权已落地；桌面 Helper / Installer 仍未移植
4. 1.26.0 合入 main（merge commit）
