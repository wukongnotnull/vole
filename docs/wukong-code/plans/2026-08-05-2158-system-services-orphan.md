# System Services Orphan（可读子集）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vole clean` 交付 `/Library/{LaunchDaemons,LaunchAgents,PrivilegedHelperTools}` 可读子集的 orphaned system services 扫描（无 sudo、fail-closed），apply 走 `NeedsPrivilege` + 响亮提示；发版 **1.5.0**。

**Architecture:** 新模块 `vole-core::sysorphan`（扫描根注入、plist Program 解析、fail-closed 存在性探测、package-managed 排除、known_protect、PHT 判定），经 custom handler `orphaned_system_services` 接入既有 plan→apply；新增 `CustomDegrade::SystemLibraryInaccessible` → `PlanNotice` → `Skipped(NeedsPrivilege)`；clean apply 补 `APPLY_PERMISSION_WARN` 接线。零 `schema_version` 变更。

**Tech Stack:** Rust 1.97.1、`vole-core`（已有 `plist = "1.10"` 依赖）、Mole 钉版 `third_party/mole-1.48.1/lib/clean/apps.sh` `clean_orphaned_system_services`。

**Design basis:** `docs/wukong-code/specs/2026-08-05-2150-system-services-orphan-design.md`（含审阅修订 1–6）。

## Global Constraints

- **禁止 sudo**（find / plist 读取 / 存在性探测一律当前用户）。
- 存在性 fail-closed（spec §4.3）：仅「每级祖先可进入 + 终点 `ENOENT`」才算缺失；`EACCES`/`EPERM` → 视为存在。
- 判定顺序以 spec §5.1/§5.2 为唯一权威；mdfind 只在 `_system_service_app_exists` / `bundle_has_installed_app` 同形处出现。
- 复用 `OrphanDeps`（mdfind/spotlight/installed scan 注入）；CI 禁真 mdfind/pgrep。
- 扫描根可注入：`SystemServiceRoots`（默认三个真实 `/Library` 路径）；测试/conformance 经 env `VOLE_TEST_SYSTEM_LIBRARY`（指向 fake `/Library`）覆盖。
- 整规则降级 reason = **`NeedsPrivilege`**（不是 `TccDenied`；不新增 `SkipReason` 变体）。
- **已知保守取舍（写死，不修）**：plan 期 `validate_path_for_deletion` / 数据保护 pattern 命中的候选按既有语义跳过；**不**做 Mole `MOLE_UNINSTALL_MODE=1` 等价绕过。
- 发现优先契约：coverage / release 文案必须写明「系统路径候选当前无法由 Vole 删除（apply 会 NeedsPrivilege），完整删除用 Mole 或未来提权能力」。
- 每个 Task 至少一次 commit；最终包版本 **1.5.0**。

---

## File Structure

```
crates/vole-core/src/sysorphan/
  mod.rs        # NEW：SystemServiceRoots + roots_from_env + re-exports
  probe.rs      # NEW：fail-closed 存在性探测 + package-managed 判定
  plist.rs      # NEW：读 ProgramArguments:0 / Program（plist crate）
  protect.rs    # NEW：known_protect_patterns + _system_service_app_exists 同形
  select.rs     # NEW：三树扫描 + §5 判定 → Vec<PathBuf> / SysOrphanScanError

crates/vole-core/src/lib.rs                    # + pub mod sysorphan
crates/vole-core/src/rules/custom_handlers.rs  # + "orphaned_system_services" + CustomDegrade::SystemLibraryInaccessible
crates/vole-core/src/ops/plan.rs               # + PlanNotice::SystemServicesInaccessible + label + degrade→Skipped(NeedsPrivilege)
crates/vole-core/src/ops/coverage.rs           # coverage 句更新 + SYSTEM_SERVICES_WARN + notices helper 扩展
crates/vole-cli/src/clean.rs                   # apply 补 APPLY_PERMISSION_WARN（human + --json）

data/rules/zzz-orphaned.toml                   # + [[rules]] orphaned-system-services（paths=三根，handler）
Cargo.toml / Cargo.lock                        # 1.5.0
README.md / docs/releases/v1.5.0.md
docs/findings/2026-08-system-services-orphan.md
```

---

### Task 1: `sysorphan` 探测与 plist 解析（probe.rs / plist.rs）

**Interfaces:**
- `pub enum BinaryPresence { Missing, PresentOrUnknowable }`
- `pub fn probe_binary_presence(path: &Path) -> BinaryPresence`（spec §4.3 三条规则）
- `pub fn is_package_managed_binary(path: &Path) -> bool`（spec §5.1 步骤 6 列表）
- `pub fn read_launchd_program(plist_path: &Path) -> Option<PathBuf>`（先 `ProgramArguments[0]` 后 `Program`；读/解析失败或非绝对路径 → `None`）

- [ ] **Step 1: 写失败测试**（`probe.rs` / `plist.rs` 各自 `#[cfg(test)]`）
  - `probe`: 终点 `ENOENT` 且祖先可进入 → `Missing`；祖先 `chmod 0o000` → `PresentOrUnknowable`（测试后恢复权限清理）；存在文件 → `PresentOrUnknowable`
  - `is_package_managed_binary`: `/opt/homebrew/bin/x`、`/usr/libexec/foo`、`/opt/homebrew/opt/pkg/bin/x` → true；`/Library/PrivilegedHelperTools/x`、`/Applications/A.app/x` → false
  - `read_launchd_program`: fixture plist 有 `ProgramArguments` → 取 [0]；只有 `Program` → 取之；相对路径 / 空 / 损坏 plist / 不可读 → `None`
- [ ] **Step 2: run `cargo test -p vole-core sysorphan --lib` → FAIL**
- [ ] **Step 3: 实现三个函数 + `mod.rs` 骨架 + `lib.rs` 挂载**
- [ ] **Step 4: 测试转绿；`cargo fmt`；commit**

### Task 2: known_protect + 三树扫描判定（protect.rs / select.rs）

**Interfaces:**
- `pub struct SystemServiceRoots { pub launch_daemons: PathBuf, pub launch_agents: PathBuf, pub privileged_helpers: PathBuf }`
  - `SystemServiceRoots::live()`；`roots_from_env()`（`VOLE_TEST_SYSTEM_LIBRARY` 覆盖）
- `protect.rs`: `KNOWN_PROTECT_PATTERNS: &[(&str, &str)]`（Mole 列表逐条移植，glob 语义）+ `system_service_app_exists(bundle_id, app_paths, deps, budget) -> bool`（含 mdfind fail-closed：超时/不可用 → true）
- `select.rs`: `pub fn select_system_service_orphans(roots, deps, budget, protection…) -> Result<Vec<PathBuf>, SysOrphanScanError>`
  - `SysOrphanScanError::AllRootsInaccessible`（三树皆不可列/权限零可读）
  - §5.1 顺序写死；§5.2 PHT `-type f` + 扩展名黑名单 + `^(com|org|net|io)\.` + `bundle_has_installed_app` 同形（installed set ∪ mdfind fail-closed）

- [ ] **Step 1: 写失败测试**（temp 目录造 fake 三树 + `FakeOrphanDeps`）
  - 可读 plist、Program 缺失、非 package-managed、无 protect → 入选
  - `com.apple.*` → 不入选；读失败（chmod 000 plist）→ 不入选
  - Program 缺失但 `/opt/homebrew/bin/*` → 不入选
  - Program 存在且位于 fake PHT、父 app 未安装 → **入选**（#1082）；父 app 已安装（installed set 命中）→ 不入选
  - protect pattern 命中且 fake app 目录存在 → 不入选；`homebrew.mxcl.*` → 无条件不入选
  - PHT：`.app` 目录 / `.json` / 非 reverse-DNS 前缀 → 不入选；合法 helper 且未安装 → 入选
  - 三树全 chmod 000 → `Err(AllRootsInaccessible)`；单树不可读 → Ok + 其余树正常
- [ ] **Step 2: run → FAIL**
- [ ] **Step 3: 实现 protect.rs / select.rs**
- [ ] **Step 4: 绿；fmt；commit**

### Task 3: handler + plan/coverage 接线

**Interfaces:**
- `CustomDegrade::SystemLibraryInaccessible`
- `PlanNotice::SystemServicesInaccessible`
- handler `"orphaned_system_services"`（roots_from_env + `ProtectionCatalog::embedded()` + live OrphanDeps 由调用方注入路径不变）
- plan.rs：degrade → emit `Skipped { rule_id, reason: NeedsPrivilege }` + notice；label：`Orphaned LaunchDaemon/LaunchAgent/PrivilegedHelper: <bundle_id>`
- coverage.rs：
  - `SYSTEM_SERVICES_WARN` 中文常量（无 sudo、结果可能不全、删除用 Mole/sudo；**不**提 FDA）
  - `coverage_with_orphan_notices` 扩展（或并列 helper）拼接新 notice
  - 全局 coverage 句：system services orphan（可读子集）→ 已落地；仍未移植改为「真 sudo 删除、Containers stubs」

- [ ] **Step 1: 失败测试**：handler 降级映射；notice → coverage 文本含 `SYSTEM_SERVICES_WARN`；coverage 句断言更新（现有 `coverage.rs` 测试同步改）
- [ ] **Step 2: run → FAIL**
- [ ] **Step 3: 实现 + `data/rules/zzz-orphaned.toml` 增 `orphaned-system-services`（category orphaned、paths 三根、handler、impact 文案含发现优先契约）**
- [ ] **Step 4: `cargo test -p vole-core --lib` 全绿；fmt；commit**

### Task 4: clean apply 权限响亮提示接线（CLI）

- [ ] `clean.rs` `run_apply`/`write_apply_output`：`--json` → `coverage_note = coverage_with_apply_permission_hint(...)`；human `print_human_report` → `report_has_permission_skips` 时 `eprintln!(APPLY_PERMISSION_WARN)`（与 uninstall/optimize 1.4.2 同形）
- [ ] `cargo test -p vole-core -p vole-cli`；clippy 两包；fmt；commit

### Task 5: 文档 + 1.5.0 发版

- [ ] README（规则数 512→513、对比表、coverage 叙述）、`docs/releases/v1.5.0.md`（含发现优先契约声明）、findings `docs/findings/2026-08-system-services-orphan.md`
- [ ] `Cargo.toml` → 1.5.0 + `cargo check` 刷 lock（macOS 本机可 build）
- [ ] 全量 `cargo test -p vole-core -p vole-cli` + clippy + fmt
- [ ] push → PR → **security-review**（系统路径扫描）→ CI 绿 → review → merge
- [ ] tag `v1.5.0` → Release workflow → `scripts/update-homebrew-formula.sh 1.5.0` → Formula + findings commit

## 验收对照（spec §9）

| 验收 | 落点 |
|---|---|
| plan 列出候选 | Task 2/3 + fixture 测试 |
| 不可读不静默 | Task 2 `AllRootsInaccessible` + Task 3 notice/Skipped |
| apply reason + 提示 + 契约文案 | Task 4 + Task 5 文档 |
| 审阅必测 | Task 1/2 Step 1 清单 |
| coverage 句更新 | Task 3 |
| 1.5.0 发版 | Task 5 |
