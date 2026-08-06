# System Services Sudo Apply Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `orphaned-system-services` 接通 CLI `sudo -n` permanent 真删（1.10.0）；plan 扫描不变。

**Architecture:** 新建 `vole-core::privilege`（`PrivilegeBackend` + `SudoNoninteractive` + `NoPrivilege`）；`mole_delete` 的 `needs_sudo` 走 Backend；`apply_plan` 去掉 system-services 硬 skip，改为形状重验 → probe → unload 尽力而为 → permanent 删除。

**Tech Stack:** Rust / macOS / `std::process::Command` / vole-core。

## Global Constraints

- 版本意图：**1.10.0**；规则数 **516 不变**；**不 bump** `schema_version`
- 仅 CLI `sudo -n`；**禁止**交互密码；桌面 SMAppService **不落地**
- 提权删除写死 **permanent**；参数分列；禁 `sh -c`
- 允许前缀仅：`/Library/LaunchDaemons/`、`/Library/LaunchAgents/`、`/Library/PrivilegedHelperTools/`
- `VOLE_TEST_NO_AUTH=1` / 测试默认 `NoPrivilege` 永不调真 sudo
- plan **不**用 sudo 扩扫描
- PR：**security-review 必过**

---

## File Structure

| 文件 | 职责 |
|---|---|
| **Create** `crates/vole-core/src/privilege/mod.rs` | trait、错误、allowlist、`path_allowed_for_privilege` |
| **Create** `crates/vole-core/src/privilege/sudo.rs` | `SudoNoninteractive`、`NoPrivilege`、测试用 `RecordingPrivilege` |
| **Modify** `crates/vole-core/src/lib.rs` | `pub mod privilege;` |
| **Modify** `crates/vole-core/src/delete/mole_delete.rs` | `needs_sudo` 走 Backend |
| **Modify** `crates/vole-core/src/ops/apply_plan.rs` | 去硬 skip；sysorphan 真删流水线；注入 Backend |
| **Modify** `crates/vole-core/src/ops/coverage.rs` | WARN + coverage 文案 |
| **Modify** README / Cargo.toml / Formula / releases / findings | 发版 |

---

### Task 1: `privilege` 模块（trait + allowlist + backends）

**Files:**
- Create: `crates/vole-core/src/privilege/mod.rs`
- Create: `crates/vole-core/src/privilege/sudo.rs`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub enum PrivilegeError { Unavailable, Refused, CommandFailed(String) }`
  - `pub trait PrivilegeBackend: Send + Sync`
  - `fn probe_noninteractive(&self) -> bool`
  - `fn remove_permanent(&self, path: &Path) -> Result<(), PrivilegeError>`
  - `fn launchctl_unload(&self, plist: &Path) -> Result<(), PrivilegeError>`
  - `pub fn path_allowed_for_privilege(path: &Path) -> bool`
  - `pub struct NoPrivilege;`
  - `pub struct SudoNoninteractive;`
  - `pub struct RecordingPrivilege`（测试）

- [ ] **Step 1: Write the failing tests**（`privilege/mod.rs` 的 `#[cfg(test)]`）

```rust
#[test]
fn allowlist_accepts_three_roots_only() {
    assert!(path_allowed_for_privilege(Path::new(
        "/Library/LaunchDaemons/com.example.plist"
    )));
    assert!(path_allowed_for_privilege(Path::new(
        "/Library/LaunchAgents/com.example.plist"
    )));
    assert!(path_allowed_for_privilege(Path::new(
        "/Library/PrivilegedHelperTools/com.example.helper"
    )));
    assert!(!path_allowed_for_privilege(Path::new("/Library/Caches/foo")));
    assert!(!path_allowed_for_privilege(Path::new(
        "/Library/LaunchDaemons/../Preferences/com.apple.plist"
    )));
    assert!(!path_allowed_for_privilege(Path::new("LaunchDaemons/x")));
}

#[test]
fn no_privilege_probe_false_and_refuses_remove() {
    let b = NoPrivilege;
    assert!(!b.probe_noninteractive());
    assert!(matches!(
        b.remove_permanent(Path::new("/Library/LaunchDaemons/x.plist")),
        Err(PrivilegeError::Unavailable)
    ));
}

#[test]
fn recording_backend_remove_requires_allowlist() {
    let b = RecordingPrivilege::allowing();
    assert!(matches!(
        b.remove_permanent(Path::new("/tmp/evil")),
        Err(PrivilegeError::Refused)
    ));
    assert!(b.removed.lock().unwrap().is_empty());
    b.remove_permanent(Path::new("/Library/LaunchDaemons/com.x.plist"))
        .unwrap();
    assert_eq!(b.removed.lock().unwrap().len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p vole-core --lib privilege::
```

Expected: FAIL（模块不存在）

- [ ] **Step 3: Implement minimal code**

`path_allowed_for_privilege`：`path.is_absolute()`；components 不含 `..`；字符串前缀为三树之一（带尾 `/` 边界，防止 `/Library/LaunchDaemonsEvil`）。

`NoPrivilege`：probe=false；remove/unload → `Unavailable`。

`SudoNoninteractive`：

```rust
fn probe_noninteractive(&self) -> bool {
    if crate::delete::test_no_auth() {
        return false;
    }
    std::process::Command::new("sudo")
        .args(["-n", "true"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn remove_permanent(&self, path: &Path) -> Result<(), PrivilegeError> {
    if crate::delete::test_no_auth() {
        return Err(PrivilegeError::Unavailable);
    }
    if !path_allowed_for_privilege(path) {
        return Err(PrivilegeError::Refused);
    }
    let status = std::process::Command::new("sudo")
        .args(["-n", "/bin/rm", "-rf", "--"])
        .arg(path)
        .status()
        .map_err(|e| PrivilegeError::CommandFailed(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(PrivilegeError::CommandFailed(format!("rm exit {status}")))
    }
}
```

`launchctl_unload`：`sudo -n /bin/launchctl unload -- <plist>`（或 `launchctl` PATH）；失败 → `CommandFailed`。

`RecordingPrivilege::allowing()`：probe=true；remove 仅 allowlist 通过时 push 到 `Mutex<Vec<PathBuf>>`。

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p vole-core --lib privilege::
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/privilege crates/vole-core/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(privilege): add PrivilegeBackend with sudo -n and allowlist

EOF
)"
```

---

### Task 2: `mole_delete` 接通 `needs_sudo`

**Files:**
- Modify: `crates/vole-core/src/delete/mole_delete.rs`

**Interfaces:**
- Consumes: `PrivilegeBackend`
- Produces: `MoleDeleteOptions { privilege: Option<&'a dyn PrivilegeBackend>, ... }`；新增 `MoleDeleteError::SudoUnavailable`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn needs_sudo_with_no_privilege_errors_unavailable() {
    // options.needs_sudo=true, privilege=Some(&NoPrivilege)
    // expect Err(SudoUnavailable) or SudoBlockedTestMode under TEST_NO_AUTH
}

#[test]
fn needs_sudo_with_recording_backend_calls_remove() {
    let backend = RecordingPrivilege::allowing();
    // path = "/Library/LaunchDaemons/com.x.plist"
    // needs_sudo=true, privilege=Some(&backend), dry_run=false
    // （identity 可 None 若 API 允许；否则构造最小 identity）
    // expect Ok + backend.removed contains path
}
```

- [ ] **Step 2: Run — expect FAIL**（仍 sudo-not-implemented）

- [ ] **Step 3: Replace needs_sudo 分支**

```rust
if options.needs_sudo {
    if test_no_auth() {
        deletion_log.log(mode_label, &size, "sudo-blocked-test-mode", path);
        return Err(MoleDeleteError::SudoBlockedTestMode);
    }
    let backend = options.privilege.unwrap_or(&NoPrivilege);
    if !backend.probe_noninteractive() {
        deletion_log.log(mode_label, "unknown", "sudo-unavailable", path);
        return Err(MoleDeleteError::SudoUnavailable);
    }
    if options.dry_run {
        deletion_log.log(mode_label, &size_field, "dry-run-sudo", path);
        return Ok(DeleteOutcome { bytes });
    }
    backend
        .remove_permanent(Path::new(path))
        .map_err(|e| /* map */)?;
    deletion_log.log(mode_label, &size_field, "ok-sudo", path);
    oplog.log("REMOVED", Path::new(path), Some("sudo-permanent")).ok();
    return Ok(DeleteOutcome { bytes });
}
```

更新所有 `MoleDeleteOptions { ... }` 构造处加 `privilege: None`（或测试显式传入）。

- [ ] **Step 4:**

```bash
cargo test -p vole-core --lib delete::
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/delete/mole_delete.rs
git commit -m "$(cat <<'EOF'
feat(delete): wire needs_sudo through PrivilegeBackend

EOF
)"
```

---

### Task 3: `apply_plan` 去掉硬 skip + sysorphan 流水线

**Files:**
- Modify: `crates/vole-core/src/ops/apply_plan.rs`
- Modify: CLI apply 调用处传入 `&SudoNoninteractive`（查找 `apply_plan(` / `ApplyCtx`）

**Interfaces:**
- `ApplyCtx` / `run_apply` 增加 `privilege: &'a dyn PrivilegeBackend`
- system-services：allowlist → identity → probe → unload → `needs_sudo=true` + Permanent

- [ ] **Step 1: Rewrite tests**

将 `apply_hard_skips_system_services_rule` 拆为：

1. `apply_system_services_skips_when_probe_fails`：注入 `NoPrivilege`；plan 路径 `/Library/LaunchDaemons/com.example.orphan.plist`；assert NeedsPrivilege；Recording 不被调用
2. `apply_system_services_deletes_when_probe_ok`：注入 `RecordingPrivilege::allowing()`；同路径；断言 succeeded 与 `removed`；**不要**依赖真实 `/Library` 文件存在——若 identity/stat 失败，对本 rule 在 probe 通过后允许「仅 Backend 删除记录」：identity 验证失败则 skip PathVanished。为使测试稳定，`plan_entry` 使用 scratch 文件的 meta，但 **policy path** 字段……  

**稳定测法（写死）：** apply 对 `SYSTEM_SERVICES_RULE_ID` **先** `path_allowed_for_privilege`；再 `verify_plan_entry`（需要真实文件）。因此测试应：

```text
不可在无 root 下创建 /Library/... 文件。
→ 生产 allowlist 不变。
→ 测试专用：对 RecordingPrivilege 测「流水线调用顺序」用单元测抽出来的 `apply_system_service_entry(...)` 辅助函数，接收已通过的 PathBuf（绝对 allowlist 字符串）并 mock stat/identity。
```

若抽取成本过高：**折中** — 删除硬 skip 后，旧测改为「NoPrivilege → NeedsPrivilege」（路径可仍为 scratch 绝对路径，因 allowlist 失败 → 映射 PathVanished 或 NeedsPrivilege）；另加 **纯函数测** `path_allowed` + Task 2 Backend 测覆盖删除。并加注释：真 `/Library` 删除依赖手工/ignored 测。

**最低验收测（必须）：**

- 硬 skip 代码块不存在（NoPrivilege + 合法 allowlist plan 路径 + 跳过 identity 时能进 probe）
- 实现时：合法 allowlist 路径若 `verify_plan_entry` 因文件不存在失败 → PathVanished（可接受）；用 `RecordingPrivilege` + 临时文件 **无法**通过 allowlist。  

故：**Task 3 测试策略定稿：**

1. 保留/改写：任意 system-services 条目 + `NoPrivilege` → `NeedsPrivilege`（即使路径是 scratch，只要 rule_id 匹配且我们在 allowlist 失败前先 probe…… **顺序必须为 allowlist 最先**）。scratch 路径 → allowlist 失败 → `PathVanished`（不是旧的无条件 NeedsPrivilege）。更新断言为 PathVanished **或** 在测里把 `entry.path` 设为 `/Library/LaunchDaemons/com.example.orphan.plist` 且 **跳过** identity（对缺失文件：`verify` 失败 → PathVanished）。要测 NeedsPrivilege：路径 allowlist OK + identity 跳过/成功 + NoPrivilege。

2. **抽出** `fn try_apply_system_service(path, backend, ...) -> ApplyResult` 单测：mock 掉 fs，只测 probe false/true 分支。实现者按此抽取。

- [ ] **Step 2–4: 实现、测试、Commit**

```bash
git commit -m "$(cat <<'EOF'
feat(apply): delete system-services orphans via sudo -n

Remove hard-skip; allowlist, probe, unload, permanent remove via backend.

EOF
)"
```

---

### Task 4: Coverage / WARN / 发版 1.10.0

**Files:**
- `crates/vole-core/src/ops/coverage.rs`
- `README.md`、`Cargo.toml`、`Cargo.lock`、`Formula/vole.rb`
- Create: `docs/releases/v1.10.0.md`、`docs/findings/2026-08-system-services-sudo-apply.md`

- [ ] **Step 1: 文案**

`SYSTEM_SERVICES_WARN`：扫描仍无 sudo；apply 在非交互 sudo 可用时永久删除，否则 skip（可先 `sudo -v`）。

`coverage_note`：system services 含「sudo -n apply 真删」；仍未移植改为 Rosetta、claude pending-uploads、交互提权/桌面助手；**删除**「真 sudo 删除」整项。

单测同步。

- [ ] **Step 2:**

```bash
cargo test -p vole-core --lib ops::coverage
cargo test -p vole-core
```

- [ ] **Step 3: version 1.10.0 + docs → Commit**

```bash
git commit -m "$(cat <<'EOF'
chore: release 1.10.0 system-services sudo -n apply

EOF
)"
```

---

## Spec coverage checklist

| Spec | Task |
|---|---|
| PrivilegeBackend / allowlist / sudo -n | 1 |
| mole_delete needs_sudo | 2 |
| apply 去硬 skip + 流水线 | 3 |
| coverage / 1.10.0 | 4 |
| plan 不扩面 / 桌面不落地 | Constraints |
| security-review | PR 时 |
