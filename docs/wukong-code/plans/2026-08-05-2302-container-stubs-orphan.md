# Containers Stubs Orphan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vole clean` 交付 Mole 同形 CleanMyMac allowlist 的 container stub 清理（plan 入选 + apply 专用 unlink+rmdir carve-out）；发版 **1.6.0**。

**Architecture:** 新模块 `vole-core::stubs`（allowlist、stub 形状判定、app 存在探测、专用删除）；custom handler `orphaned_container_stubs`；plan.rs 对该 `rule_id` 豁免 `validate_path_for_deletion`（保留白名单 + identity + 路径形状检查）；apply_plan.rs 早分支专用删除（对齐 `SYSTEM_SERVICES_RULE_ID` 分流位置，但执行 carve-out 而非硬 skip）。零 `schema_version` 变更。

**Tech Stack:** Rust 1.97.1、`vole-core`、Mole 钉版 `third_party/mole-1.48.1/lib/clean/apps.sh`（`clean_orphaned_container_stubs` / `_remove_verified_container_stub`）。

**Design basis:** `docs/wukong-code/specs/2026-08-05-2253-container-stubs-orphan-design.md`（含审阅修订：protect 豁免、carve-out 强制、TeamID mdfind 跳过）。

## Global Constraints

- Allowlist 写死两条（Mole 1.48.1）：`("com.macpaw.CleanMyMac*", "/Applications/CleanMyMac X.app")`、`("*.com.macpaw.CleanMyMac*", "/Applications/CleanMyMac X.app")`；扩表须另开 design。
- stub 定义写死：非 symlink 目录 + 唯一子项 `.com.apple.containermanagerd.metadata.plist`（普通文件）。
- **豁免面最小化**：只有 `rule_id == orphaned-container-stubs` 跳过 `validate_path_for_deletion` / `verify_plan_entry_for_apply`；豁免路径必须先通过「位于 `home/Library/Containers/<单层名>`」形状校验。
- apply **禁止** trash / `mole_delete_verified` / `rm -r`；忽略 `--permanent`；重验失败 → `Skipped(PathVanished)`。
- app 存在探测：canonical path → `$HOME/Applications` → `/Applications/Setapp` → `$HOME/Library/Application Support/Setapp/Applications`；reverse-DNS 才走 mdfind（fail-closed：Spotlight 不可用/Err → 视为仍安装）；TeamID 前缀（非 reverse-DNS）**不**调 mdfind。
- Containers 根不可列 → `CustomDegrade`（新 variant）→ `Skipped(TccDenied)` + `PlanNotice` + 中文 warn（提 FDA）。
- `protection.toml` 与全局 `should_protect_path` **不改**。
- 每个 Task 至少一次 commit；最终包版本 **1.6.0**；PR 前 security-review。

---

## File Structure

```
crates/vole-core/src/stubs/
  mod.rs        # NEW：CONTAINER_STUB_RULE_ID、allowlist 常量、label、re-exports
  select.rs     # NEW：扫描 + 判定 → Vec<PathBuf> / StubScanError::ContainersInaccessible
  remove.rs     # NEW：remove_verified_container_stub（重验 + unlink + rmdir）

crates/vole-core/src/lib.rs                    # + pub mod stubs
crates/vole-core/src/rules/custom_handlers.rs  # + handler + CustomDegrade::ContainersInaccessible
crates/vole-core/src/ops/plan.rs               # + notice + label + validate 豁免（形状校验替代）
crates/vole-core/src/ops/apply_plan.rs         # + carve-out 早分支
crates/vole-core/src/ops/coverage.rs           # + CONTAINER_STUBS_WARN + coverage 句更新

data/rules/zzz-orphaned.toml                   # + [[rule]] orphaned-container-stubs
Cargo.toml / Cargo.lock                        # 1.6.0
README.md / docs/releases/v1.6.0.md
docs/findings/2026-08-container-stubs-orphan.md
```

---

### Task 1: `stubs::select` — allowlist 扫描与 stub 判定

**Interfaces:**
- `pub const CONTAINER_STUB_RULE_ID: &str = "orphaned-container-stubs";`
- `pub const CONTAINER_STUB_METADATA: &str = ".com.apple.containermanagerd.metadata.plist";`
- `pub const STUB_ALLOWLIST: &[(&str, &str)]`（两条）
- `pub fn container_stub_label(path: &Path) -> String` → `Orphaned container stub: <basename>`
- `pub fn is_verified_stub_dir(dir: &Path) -> bool`（非 symlink + 唯一 metadata；select 与 apply 共用）
- `pub enum StubScanError { ContainersInaccessible }`
- `pub fn select_container_stubs(home: &Path, deps: &dyn OrphanDeps) -> Result<Vec<PathBuf>, StubScanError>`

- [ ] **Step 1: 失败测试**（temp home 下造 `Library/Containers`）
  - 纯 stub `com.macpaw.CleanMyMac4` → 入选
  - stub + `Data/` 子目录 → 不入选
  - symlink 目录 → 不入选
  - metadata 缺失 / 是目录 → 不入选
  - `S8EX82NJP6.com.macpaw.CleanMyMac4` stub → 入选（TeamID glob）且 **FakeOrphanDeps 断言 mdfind 未被调用**（`mdfind: HashMap` 留空 + spotlight=false 也不影响：非 reverse-DNS 跳过）
  - allowlist 外 `com.example.app` stub → 不入选
  - fake `/Applications/CleanMyMac X.app` 存在（经 home 注入的探测路径注入 or 直接建 `$home/Applications/CleanMyMac X.app`）→ 不入选
  - reverse-DNS id 且 spotlight=false → 视为仍安装 → 不入选
  - `Library/Containers` 不存在 → `Ok(vec![])`；存在但 chmod 000 → `Err(ContainersInaccessible)`（测试后恢复权限）
- [ ] **Step 2: run `cargo test -p vole-core stubs --lib` → FAIL**
- [ ] **Step 3: 实现 select.rs / mod.rs + lib.rs 挂载**（canonical app 探测表进函数内；`/Applications/*` 变体探测对齐 Mole `_container_stub_app_exists`）
- [ ] **Step 4: 绿；fmt；commit**

### Task 2: `stubs::remove` — carve-out 删除

**Interfaces:**
- `pub enum StubRemoveError { NotAStub, MetadataUnlink, RmdirFailed }`
- `pub fn remove_verified_container_stub(dir: &Path) -> Result<(), StubRemoveError>`
  - 重验 `is_verified_stub_dir`
  - `fs::remove_file(dir/CONTAINER_STUB_METADATA)`
  - `fs::remove_dir(dir)`（非空即失败；**不**递归）

- [ ] **Step 1: 失败测试**
  - stub → Ok 且目录消失
  - 删除前塞入额外文件 → `NotAStub`，metadata 与目录都保留
  - metadata unlink 后 rmdir 前塞文件（用只读目录或直接模拟：先手工 unlink 再造文件）→ `RmdirFailed` 且目录保留
  - symlink → `NotAStub`
- [ ] **Step 2: run → FAIL**
- [ ] **Step 3: 实现 remove.rs**
- [ ] **Step 4: 绿；fmt；commit**

### Task 3: handler + plan 豁免 + coverage

**Interfaces:**
- `CustomDegrade::ContainersInaccessible`
- handler `"orphaned_container_stubs"`（home + orphan_deps → select）
- plan.rs：
  - degrade → `Skipped(TccDenied)` + `PlanNotice::ContainersInaccessible`
  - label 分支 `CONTAINER_STUB_RULE_ID` → `container_stub_label`
  - **validate 豁免**：`rule_id == CONTAINER_STUB_RULE_ID` 时不调 `validate_path_for_deletion`，改为形状校验 `is_container_stub_candidate_path(path, home)`（必须 == `home/Library/Containers/<name>` 单层、无 `..`）；失败按原样 emit skip
- coverage.rs：`CONTAINER_STUBS_WARN`（FDA 指引，风格对齐 `ORPHAN_LIBRARY_WARN`）；`coverage_with_orphan_notices` 拼接；全局句改「container stubs（CleanMyMac allowlist）已落地」，仍未移植列表去掉 Containers stubs、保留真 sudo / Group Containers 泛清理

- [ ] **Step 1: 失败测试**：handler degrade 映射；plan 集成测试（VOLE_TEST_HOME + fixture stub → plan 含候选，证明未死于 validate）；coverage 断言更新（现有测试同步改）
- [ ] **Step 2: run → FAIL**
- [ ] **Step 3: 实现 + `data/rules/zzz-orphaned.toml` 增 `[[rule]] orphaned-container-stubs`（paths = `~/Library/Containers`，handler；impact 写明 allowlist + 非 trash carve-out）+ load.rs 末位规则测试同步**
- [ ] **Step 4: `cargo test -p vole-core --lib` 全绿；fmt；commit**

### Task 4: apply carve-out 早分支

- plan_verify / mole_delete 不动；`apply_plan.rs` 在 `SYSTEM_SERVICES_RULE_ID` 分支后加：

```text
rule_id == CONTAINER_STUB_RULE_ID →
  verify_plan_entry(path, identity) 失败 → Skipped(PathVanished)
  remove_verified_container_stub(dir) Ok → succeeded（bytes 0）
  Err(NotAStub/...) → Skipped(PathVanished)
  绝不落入 mole_delete_verified
```

- [ ] **Step 1: 失败测试**（apply_plan.rs tests）
  - stub plan → apply succeeded=1，目录消失，trash 未被调用（FakeTrash calls 为空）
  - apply 前塞 `Data/` → skipped=1（PathVanished），目录保留
  - `--permanent` opts → 行为相同（仍 carve-out）
- [ ] **Step 2: run → FAIL**
- [ ] **Step 3: 实现分支**
- [ ] **Step 4: `cargo test -p vole-core --lib` + clippy 两包全绿；fmt；commit**

### Task 5: 文档 + 1.6.0 发版

- [ ] README（规则 513→514、对比句去 Containers stubs、版本 1.6.0）、`docs/releases/v1.6.0.md`（allowlist + carve-out + 忽略 --permanent 说明）、findings
- [ ] `Cargo.toml` → 1.6.0 + `cargo check` 刷 lock
- [ ] 全量 `cargo test -p vole-core -p vole-cli` + clippy + fmt
- [ ] push → PR → **security-review**（豁免面 + carve-out）→ 修复回执 → CI 绿 → review → merge
- [ ] tag `v1.6.0` → Release workflow → `scripts/update-homebrew-formula.sh 1.6.0` → Formula + release findings commit

## 验收对照（spec §11）

| 验收 | 落点 |
|---|---|
| plan 列出 `com.macpaw.*` stub（未死于 validate） | Task 3 集成测试 |
| apply 真删 stub（非 trash）、非 stub 不删 | Task 2/4 |
| 其它 Containers 路径保护不变 | Task 3 豁免面最小 + 既有 property 测试 |
| coverage / README 更新 | Task 3/5 |
| 1.6.0 发版 | Task 5 |
