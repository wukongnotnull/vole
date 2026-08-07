# Rosetta `/Library` Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地 `rosetta-2-cache`：`/Library/Apple/.../rosetta_update_bundle` 在 arm64 上可 plan，并通过 `sudo -n` permanent 删除（1.12.0）。

**Architecture:** exact critical 豁免 + PrivilegeBackend exact allow + plan/apply arm64 门控；apply 仿 system-services（无 unload）。

**Tech Stack:** Rust / vole-core / macOS `uname` / `sudo -n`。

## Global Constraints

- 版本：**1.12.0**；规则 **517 → 518**；不 bump schema
- 放行面：**仅** exact `…/rosetta_update_bundle`（live 或 `VOLE_TEST_SYSTEM_LIBRARY` 映射）
- **禁止**放宽 `/Library/Apple/**` critical；**禁止**交互 sudo / 桌面 SMAppService
- arm64：运行时 `uname -m == arm64`（`VOLE_TEST_FORCE_UNAME_M` 可注入）
- 提权删除 **permanent**；plan 不 sudo
- PR：security-review；**默认不打 tag**

---

## File Structure

| 文件 | 职责 |
|---|---|
| **Modify** `crates/vole-core/src/safety/critical.rs` | `is_rosetta_update_bundle` |
| **Modify** `crates/vole-core/src/safety/validate.rs` | critical 前 early-ok |
| **Modify** `crates/vole-core/src/privilege/mod.rs` | exact allow + `is_arm64_host`（或旁侧小模块） |
| **Modify** `crates/vole-core/src/ops/plan.rs` | `rosetta-2-cache` 非 arm64 → 零候选 |
| **Modify** `crates/vole-core/src/ops/apply_plan.rs` | rule 特权流水线 |
| **Modify** `data/rules/user-devtools.toml` | 规则（紧邻 `rosetta-2-user-cache`） |
| **Modify** coverage / README / Cargo / Formula / releases / findings | 发版 |

**常量**（建议放 `privilege` 或小 `rosetta` 模块再 re-export）：

```rust
pub const ROSETTA_CACHE_RULE_ID: &str = "rosetta-2-cache";
pub const ROSETTA_UPDATE_BUNDLE_LIVE: &str =
    "/Library/Apple/usr/share/rosetta/rosetta_update_bundle";
```

---

### Task 1: Critical 豁免 + validate

**Files:** `safety/critical.rs`、`safety/validate.rs`

- [ ] **Step 1: Failing tests**（`critical.rs`）

```rust
#[test]
fn rosetta_update_bundle_exact_only() {
    assert!(is_rosetta_update_bundle(
        "/Library/Apple/usr/share/rosetta/rosetta_update_bundle"
    ));
    assert!(is_rosetta_update_bundle(
        "/Library/Apple/usr/share/rosetta/rosetta_update_bundle/"
    ));
    assert!(!is_rosetta_update_bundle(
        "/Library/Apple/usr/share/rosetta"
    ));
    assert!(!is_rosetta_update_bundle(
        "/Library/Apple/usr/share/rosetta/rosetta_update_bundle/extra"
    ));
    assert!(!is_rosetta_update_bundle("/Library/Apple/other"));
    // critical 仍认整树，但 exact 走独立谓词
    assert!(is_critical_deletion_path(
        "/Library/Apple/usr/share/rosetta/rosetta_update_bundle"
    ));
}
```

另：在 `validate` 单测（若已有模式）断言 exact path `validate_path_for_deletion` Ok（需临时关闭 protect 干扰；或仅测 critical early path）。

- [ ] **Step 2:** `cargo test -p vole-core --lib safety::critical` — FAIL

- [ ] **Step 3: Implement**

```rust
pub fn is_rosetta_update_bundle(path: &str) -> bool {
    let path = normalize_policy_path(path);
    if path == ROSETTA_UPDATE_BUNDLE_LIVE {
        return true;
    }
    if let Some(base) = std::env::var_os("VOLE_TEST_SYSTEM_LIBRARY") {
        let mapped = PathBuf::from(base)
            .join("Apple/usr/share/rosetta/rosetta_update_bundle");
        if let Some(s) = mapped.to_str() {
            return path == normalize_policy_path(s);
        }
    }
    false
}
```

`validate.rs`：在 `is_coresymbolicationd_cache` 旁：

```rust
if is_rosetta_update_bundle(&policy_path) {
    return Ok(());
}
```

- [ ] **Step 4:** tests PASS

- [ ] **Step 5: Commit** `feat(safety): exact carve-out for Rosetta update bundle`

---

### Task 2: Privilege exact allow + arm64 门控

**Files:** `privilege/mod.rs`（可拆 `host.rs`）

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn allowlist_accepts_rosetta_exact() {
    assert!(path_allowed_for_privilege(Path::new(
        "/Library/Apple/usr/share/rosetta/rosetta_update_bundle"
    )));
    assert!(!path_allowed_for_privilege(Path::new(
        "/Library/Apple/usr/share/rosetta"
    )));
    assert!(!path_allowed_for_privilege(Path::new(
        "/Library/Apple/usr/share/rosetta/rosetta_update_bundle/x"
    )));
}

#[test]
fn arm64_host_respects_force_env() {
    // 用 test_env::lock 或串行；设 VOLE_TEST_FORCE_UNAME_M
    std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "arm64");
    assert!(is_arm64_host());
    std::env::set_var("VOLE_TEST_FORCE_UNAME_M", "x86_64");
    assert!(!is_arm64_host());
    std::env::remove_var("VOLE_TEST_FORCE_UNAME_M");
}
```

- [ ] **Step 2:** FAIL

- [ ] **Step 3: Implement**

`path_allowed_for_privilege`：先判 `is_rosetta_update_bundle(s)`（Path→str）为 true 则允许；否则现有三树逻辑。

`is_arm64_host`：读 `VOLE_TEST_FORCE_UNAME_M`，否则 `Command::new("uname").arg("-m")`。

- [ ] **Step 4:** PASS + 三树旧测仍绿

- [ ] **Step 5: Commit** `feat(privilege): allow Rosetta update bundle + arm64 gate`

---

### Task 3: 规则 + plan/apply 接线

**Files:** `user-devtools.toml`、`plan.rs`、`apply_plan.rs`（必要时 `lib` re-export 常量）

规则（紧邻 `rosetta-2-user-cache`）：

```toml
[[rule]]
id = "rosetta-2-cache"
category = "user-devtools"
label = "Rosetta 2 cache"
platform = ["macos"]
paths = ["/Library/Apple/usr/share/rosetta/rosetta_update_bundle"]
impact = "Rosetta update bundle; safe to rebuild"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "all"
```

- [ ] **Step 1:** plan：处理 `ROSETTA_CACHE_RULE_ID` 时若 `!is_arm64_host()` 跳过展开（零候选）。单测：force x86 → 无该 path；force arm64 + fixture 文件 → 入选（`VOLE_TEST_SYSTEM_LIBRARY` 映射时规则 path 或测试用 remap——若 TOML 写死 live path，plan 测试可临时写 live 或在 expand 中识别 remapped：

  **注意**：fixture 下 live `/Library/...` 通常不存在。二选一（计划写死选 **B**）：
  - **B（推荐）**：plan 对 `rosetta-2-cache` 在 `VOLE_TEST_SYSTEM_LIBRARY` 设定时，将候选路径解析为 mapped exact（与 privilege/critical 一致），再 `exists` 入选。
  - A：仅测 unit，不做集成。

- [ ] **Step 2:** apply：仿 `SYSTEM_SERVICES_RULE_ID` 分支，但对 Rosetta：
  1. `!is_arm64_host()` → skip
  2. `!path_allowed_for_privilege` → skip
  3. `!probe` → `NeedsPrivilege`
  4. identity verify
  5. **不** unload
  6. `needs_sudo: true` permanent

  单测：RecordingPrivilege 删除 mapped 文件；NoPrivilege → NeedsPrivilege。

- [ ] **Step 3:** `rg -c '^id = ' data/rules/*.toml` → **518**

- [ ] **Step 4: Commit** `feat: wire rosetta-2-cache plan/apply with sudo -n`

---

### Task 4: Coverage / docs / 1.12.0

- coverage：落地句加入 Rosetta；unported 去掉 Rosetta，保留交互提权 / 桌面
- 单测：`!unported.contains("Rosetta")`；`unported` 仍可提交互/桌面
- README 517→518；Cargo/Formula **1.12.0**
- `docs/releases/v1.12.0.md`、`docs/findings/2026-08-rosetta-library-cache.md`
- `cargo test -p vole-core`
- Commit：`chore: release 1.12.0 rosetta-2-cache`（不打 tag）

---

## Spec coverage

| Spec | Task |
|---|---|
| critical exact | 1 |
| privilege + arm64 | 2 |
| TOML + plan/apply | 3 |
| coverage / 1.12.0 | 4 |
| security-review | PR 阶段 |
