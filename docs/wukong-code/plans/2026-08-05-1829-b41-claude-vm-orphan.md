# B4.1：Claude VM Orphan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在既有 `orphaned-app-data` 主路径上交付 Claude Desktop workspace VM orphan 清理（对齐 Mole `is_claude_vm_bundle_orphaned`），发版 **1.4.0**。

**Architecture:** 扩展 `vole-core::orphan`：新增 Claude VM 年龄 env、路径识别、专用 judge、select 合并；`OrphanDeps` 增加可注入的 Claude 进程探针；apply 重判按路径分流。规则仍为 `orphaned-app-data` / handler `orphaned_app_data`。零 `schema_version` 变更。

**Tech Stack:** Rust 1.97.1、`vole-core`、Mole 钉版 `third_party/mole-1.48.1/lib/clean/apps.sh`。

**Design basis:** B4 spec §4.4 + 本会话批准方案（同规则扩展、1.4.0 MINOR）。不混淆 `ai-agents` 的 `claude-code` / `claude-code-vm` keep-N。

## Global Constraints

- 许可证：GPL-3.0-only；仅 macOS；无 sudo。
- 规则 id / handler 不变：`orphaned-app-data` / `orphaned_app_data`。
- Claude 扫描：仅 `$HOME/Library/Application Support/Claude` 下 depth≤3 的 `*.bundle` 目录（对齐 Mole `find -maxdepth 3`）；**不**泛扫整个 Application Support。
- 规则 glob **不支持 `**`**（见 `rules/glob.rs`）→ TOML 用三层浅模式，或 select 内自举 walk（二选一写死在 Task 2；推荐自举 walk 以免依赖三行 glob）。
- env：`MOLE_CLAUDE_VM_ORPHAN_AGE_DAYS` 默认 **7**；解析失败或空 → **7**（不沿用 `MOLE_ORPHAN_AGE_DAYS` 的「非法→30」）。
- 判定目标 bundle id 固定：`com.anthropic.claudefordesktop`。
- Spotlight / mdfind fail-closed 与 B4.0 相同；复用同一 `OrphanDeps::mdfind_bundle`。
- Claude 进程：`pgrep -x Claude` 等价；必须经 `OrphanDeps` 注入，CI 禁真 pgrep。
- 零尺寸跳过；whitelist + `validate_path_for_deletion` + Trash 漏斗不变。
- 不做：system services orphan、Containers stubs、其它 Application Support 扫。
- 每个 Task 至少一次 commit；包版本最终 **1.4.0**。

---

## File Structure

```
crates/vole-core/src/orphan/
  mod.rs          # + CLAUDE constants / re-exports
  deps.rs         # + OrphanDeps::claude_desktop_running
  judge.rs        # + claude vm age + is_claude_vm_bundle_orphaned + path helpers + label
  select.rs       # + merge Claude VM candidates into select_orphaned_paths
  claude.rs       # NEW（可选）：walk Claude Support + select_claude_vm；若保持 select.rs 内联也可

crates/vole-core/src/rules/custom_handlers.rs   # 传 claude age（若需要）
crates/vole-core/src/ops/apply_plan.rs          # recheck 分流
crates/vole-core/src/ops/plan.rs               # orphan_label 已覆盖 Claude
crates/vole-core/src/ops/coverage.rs           # 去掉「Claude VM orphan」未移植

data/rules/zzz-orphaned.toml                   # impact 文案更新（paths 可不加若用自举 walk）
Cargo.toml / crates/*/Cargo.toml               # 1.4.0
docs/releases/v1.4.0.md
README.md
docs/findings/2026-08-b41-claude-vm-orphan.md  # 安全/验收短文
docs/wukong-code/specs/2026-08-05-1642-b4-orphaned-app-data-design.md  # 状态注 B4.1 已实现
```

**推荐扫描方式（写死）：** select 在 `Library/Caches` FDA 闸通过后，若 `home/Library/Application Support/Claude` 存在，自行 `max_depth=3` 枚举 `*.bundle` 目录并并入结果。**不**依赖 TOML `**`；也不必强加三行浅 glob（避免双通道重复）。TOML `paths` 保持 B4.0 三根；impact 文案注明含 Claude VM。

---

### Task 1: Claude VM age + path helpers + judge

**Files:**
- Modify: `crates/vole-core/src/orphan/mod.rs`
- Modify: `crates/vole-core/src/orphan/judge.rs`
- Modify: `crates/vole-core/src/orphan/deps.rs`（本 Task 只加 trait 方法 stub + Fake 字段；Live 实现可本 Task 一并完成）

**Interfaces:**
- Produces:
  - `pub const CLAUDE_DESKTOP_BUNDLE_ID: &str = "com.anthropic.claudefordesktop";`
  - `pub const DEFAULT_CLAUDE_VM_ORPHAN_AGE_DAYS: u32 = 7;`
  - `pub fn claude_vm_orphan_age_days_from_env() -> u32`
  - `pub fn claude_vm_orphan_age_days_from_raw(raw: Option<&str>) -> u32`
  - `pub fn is_claude_vm_bundle_path(path: &Path, home: &Path) -> bool`
  - `impl OrphanJudge { pub fn is_claude_vm_bundle_orphaned(&self, path: &Path, mtime: SystemTime) -> bool }`
  - `OrphanDeps::claude_desktop_running(&self) -> bool`
  - `FakeOrphanDeps { pub claude_running: bool, ... }`（默认 `false`）
  - `orphan_label`：Claude VM 路径 → `"Orphaned Claude workspace VM"`

- [ ] **Step 1: Write failing tests in `judge.rs`**

```rust
#[test]
fn claude_vm_age_defaults_and_invalid() {
    assert_eq!(claude_vm_orphan_age_days_from_raw(None), 7);
    assert_eq!(claude_vm_orphan_age_days_from_raw(Some("")), 7);
    assert_eq!(claude_vm_orphan_age_days_from_raw(Some("nope")), 7);
    assert_eq!(claude_vm_orphan_age_days_from_raw(Some("14")), 14);
    assert_eq!(claude_vm_orphan_age_days_from_raw(Some("0")), 7); // 0 视为非法 → 7
}

#[test]
fn is_claude_vm_bundle_path_only_under_claude_support() {
    let home = Path::new("/Users/t");
    assert!(is_claude_vm_bundle_path(
        Path::new("/Users/t/Library/Application Support/Claude/vm_bundles/x.bundle"),
        home,
    ));
    assert!(!is_claude_vm_bundle_path(
        Path::new("/Users/t/Library/Caches/com.foo.bar"),
        home,
    ));
    assert!(!is_claude_vm_bundle_path(
        Path::new("/Users/t/Library/Application Support/Other/x.bundle"),
        home,
    ));
}

#[test]
fn claude_vm_judge_skips_when_running_or_installed_or_young() {
    // FakeOrphanDeps { claude_running: true } → false
    // installed contains CLAUDE_DESKTOP_BUNDLE_ID → false
    // mtime < age_days → false
    // spotlight off → false
    // mdfind Ok(true) → false
    // mdfind Err → false
    // else → true
}
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test -p vole-core orphan::judge --lib`
Expected: FAIL（符号未定义 / 断言失败）

- [ ] **Step 3: Implement**

`claude_vm_orphan_age_days_from_raw`:
```rust
pub fn claude_vm_orphan_age_days_from_raw(raw: Option<&str>) -> u32 {
    let Some(s) = raw.filter(|s| !s.trim().is_empty()) else {
        return DEFAULT_CLAUDE_VM_ORPHAN_AGE_DAYS;
    };
    match s.trim().parse::<u32>() {
        Ok(n) if n >= 1 => n,
        _ => DEFAULT_CLAUDE_VM_ORPHAN_AGE_DAYS,
    }
}
```

`is_claude_vm_bundle_path`: path 规范化后 `starts_with(home.join("Library/Application Support/Claude"))`，且 `extension == Some("bundle")`（或文件名以 `.bundle` 结尾）。

`is_claude_vm_bundle_orphaned`（对齐 Mole）：
1. `deps.claude_desktop_running()` → `false`
2. `installed.contains(CLAUDE_DESKTOP_BUNDLE_ID)` → `false`
3. age < threshold（用 **Claude** `age_days`，由调用方传入 judge 的字段；select/apply 对 Claude 分支设 Claude age）→ `false`
4. mdfind/Spotlight 对 `CLAUDE_DESKTOP_BUNDLE_ID` fail-closed（同 `is_bundle_orphaned`）
5. 否则 `true`

**Age 字段设计：** 不要复用 `OrphanJudge.age_days` 混用 30/7。二选一（推荐 A）：
- **A.** 新增方法签名自带 `age_days: u32` 参数：`is_claude_vm_bundle_orphaned(&self, path, mtime, age_days)`
- B. 另构 `ClaudeVmJudge`

选 **A**。

`orphan_label`: 若 path 含 `/Application Support/Claude/` 且以 `.bundle` 结尾 → 固定英文 label（对齐 Mole safe_clean 文案）。

`OrphanDeps` 增加：
```rust
fn claude_desktop_running(&self) -> bool;
```
`FakeOrphanDeps.claude_running: bool`（Default false）。
`LiveOrphanDeps`：`pgrep -x Claude`（超时同 PROBE_TIMEOUT；失败视为未运行？→ **fail-closed：探针失败视为 running=true**，避免误删）。写死：超时/非零 → `true`（保守）。

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test -p vole-core orphan::judge --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/orphan/
git commit -m "$(cat <<'EOF'
feat(orphan): Claude VM age, path helpers, and judge

EOF
)"
```

---

### Task 2: Select — 枚举并合并 Claude VM 候选

**Files:**
- Modify: `crates/vole-core/src/orphan/select.rs`
- Modify: `crates/vole-core/src/orphan/mod.rs`（re-export 如需要）
- Modify: `crates/vole-core/src/rules/custom_handlers.rs`（调用时传入 Claude age 逻辑已在 select 内读 env）

**Interfaces:**
- Consumes: Task 1 helpers + `OrphanDeps`
- Produces: `select_orphaned_paths` 返回值可含 Claude VM 路径；单测锁定

- [ ] **Step 1: Write failing select tests**

在临时 home：
- 建 `Library/Caches`（可读，过 FDA 闸）
- 建 `Library/Application Support/Claude/vm_bundles/old.bundle/rootfs.img`，mtime 设为 10 天前
- `FakeOrphanDeps { spotlight: true, mdfind: {claude_id → Ok(false)}, claude_running: false, installed: {} }`
- `select_orphaned_paths(..., orphan_age=30, now)` 应 **包含** `old.bundle`
- `claude_running: true` → 不包含
- installed 含 `com.anthropic.claudefordesktop` → 不包含
- mtime 1 天前 + claude age 7 → 不包含
- `Library/Application Support/Other/x.bundle` → 永不包含
- depth>3 的 `.bundle` → 不包含（例如 Claude/a/b/c/d.bundle）

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p vole-core orphan::select --lib`
Expected: FAIL

- [ ] **Step 3: Implement walk + merge**

在 `select_orphaned_paths` 末尾（或中段，在 installed 扫描成功后）：

```rust
let claude_age = claude_vm_orphan_age_days_from_env(); // 测试可加参数；见下
selected.extend(select_claude_vm_bundles(home, &judge_base, claude_age, now, deps, &installed)?);
```

为保持可测，扩展签名（推荐）：

```rust
pub fn select_orphaned_paths(
    entries: &[PathEntry],
    home: &Path,
    catalog: &ProtectionCatalog,
    deps: &dyn OrphanDeps,
    age_days: u32,
    claude_age_days: u32,
    now: SystemTime,
) -> Result<Vec<PathBuf>, OrphanScanError>
```

更新 `custom_handlers::orphaned_app_data` 传入 `claude_vm_orphan_age_days_from_env()`。更新所有既有 select 调用点（plan 测试、select 测试）。

`select_claude_vm_bundles`：
- root = `home.join("Library/Application Support/Claude")`
- 若不存在 → 空
- BFS/DFS，相对 depth≤3，目录名 `*.bundle`：
  - 零尺寸跳过
  - whitelist：经 `crate::whitelist`（对齐 Mole；若 B4.0 select 未显式查 whitelist，检查 plan/apply 路径——B4 把 whitelist 放在 judge 或外层；读现码：若 judge 无 whitelist，确认 `ops/plan` 去重层是否调用。**对齐 Mole：select 阶段 skip whitelist。** 查 `is_path_whitelisted` 在 vole 的等价 API，若 plan 管线稍后有统一过滤则 select 可不查；否则在 Claude 分支调用与其它 clean 一致的 whitelist。）
- `judge.is_claude_vm_bundle_orphaned(path, mtime, claude_age_days)`
- 迭代上限：Claude 根最多 `MAX_ORPHAN_ITERATIONS` 个 bundle 候选检查

Whitelist：查 `crate::whitelist::is_whitelisted`（或现有名）；B4.0 若未在 select 调，本 Task 对 Claude **显式调用**（Mole 对 Claude 做了；对常规 orphan 也在 clean 循环里做——常规侧若缺失属已知差异，本 Task 至少 Claude 对齐）。

- [ ] **Step 4: Run — expect PASS**

Run: `cargo test -p vole-core orphan::select orphan::judge --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/orphan/ crates/vole-core/src/rules/custom_handlers.rs
git commit -m "$(cat <<'EOF'
feat(orphan): select Claude Desktop workspace VM bundles

EOF
)"
```

---

### Task 3: Apply 重判分流 + plan label

**Files:**
- Modify: `crates/vole-core/src/ops/apply_plan.rs`（`recheck_orphaned_entry`）
- Modify: `crates/vole-core/src/ops/plan.rs`（若 label 已由 `orphan_label` 覆盖则仅加测试）
- Modify: `crates/vole-core/src/ops/apply_plan.rs` 既有 orphan 测试

**Interfaces:**
- Consumes: `is_claude_vm_bundle_path`, `is_claude_vm_bundle_orphaned`, Claude age env
- Produces: apply 对 Claude VM plan 条目走 Claude 重判；非 Claude 仍 `bundle_id_from_orphan_path` + `is_bundle_orphaned`

- [ ] **Step 1: Write failing apply tests**

用 tempfile home + `FakeOrphanDeps`：
1. plan entry path = Claude `*.bundle`，`rule_id=orphaned-app-data`，deps 显示 Claude 已安装 → recheck / apply **skip**
2. 同 path，deps 未安装 + 年龄够 + mdfind false → recheck **pass**
3. 常规 Cache orphan 回归仍 pass

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p vole-core apply_plan --lib`
Expected: FAIL on new cases（当前 `bundle_id_from_orphan_path("claudevm.bundle")` 为 `None` → recheck 恒 false）

- [ ] **Step 3: Implement `recheck_orphaned_entry`**

```rust
fn recheck_orphaned_entry(...) -> bool {
    let home = dirs_home();
    let Ok(installed) = deps.scan_installed_bundle_ids(home.as_path()) else {
        return false;
    };
    let judge = OrphanJudge { catalog: protection.catalog(), deps, installed: &installed, age_days: orphan_age_days_from_env(), now };
    if is_claude_vm_bundle_path(&entry.path, home.as_path()) {
        return judge.is_claude_vm_bundle_orphaned(
            &entry.path,
            entry.mtime,
            claude_vm_orphan_age_days_from_env(),
        );
    }
    let Some(bundle_id) = bundle_id_from_orphan_path(&entry.path) else {
        return false;
    };
    judge.is_bundle_orphaned(&bundle_id, &entry.path, entry.mtime)
}
```

- [ ] **Step 4: Run — expect PASS**

Run: `cargo test -p vole-core --lib orphan:: apply_plan`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/apply_plan.rs crates/vole-core/src/ops/plan.rs
git commit -m "$(cat <<'EOF'
feat(clean): apply recheck for Claude VM orphan entries

EOF
)"
```

---

### Task 4: 文案、规则 impact、安全 findings、版本 1.4.0

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs` + 其测试
- Modify: `data/rules/zzz-orphaned.toml`（impact 提到 Claude VM；`last_verified`）
- Modify: `README.md`（与 Mole 对比 / 特性：Claude VM orphan 已做）
- Create: `docs/releases/v1.4.0.md`
- Create: `docs/findings/2026-08-b41-claude-vm-orphan.md`
- Modify: workspace + crate `Cargo.toml` version → `1.4.0`
- Modify: B4 design status 行注「B4.1 已实现 → 1.4.0」
- **不**改 `Formula/vole.rb`（发版 tag 后再 `update-homebrew-formula.sh`）

- [ ] **Step 1: coverage 测试先改期望**

`coverage_note` 不再含「Claude VM orphan」；仍可列 system services / Containers / sudo。

- [ ] **Step 2: 改文案与版本**

`docs/releases/v1.4.0.md` 要点：
- Claude Desktop workspace VM orphan（`MOLE_CLAUDE_VM_ORPHAN_AGE_DAYS`，默认 7）
- 仍未做：system services orphan、Containers stubs、真 sudo

Findings 勾选：进程 fail-closed、mdfind fail-closed、depth≤3、与 ai-agents keep-N 隔离、apply 重判、注入测试。

- [ ] **Step 3: Run**

Run: `cargo test -p vole-core --lib`  
Run: `cargo fmt --all -- --check`  
（本机 macOS 上）`cargo clippy -p vole-core --all-targets -- -D warnings`（若环境允许）

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vole-core data/rules/zzz-orphaned.toml README.md docs/ Cargo.toml crates/*/Cargo.toml
git commit -m "$(cat <<'EOF'
chore(release): prepare v1.4.0 Claude VM orphan

EOF
)"
```

---

## Self-Review

1. **Spec coverage:** B4 §4.4 Claude 路径 / 年龄 env / 对齐 `is_claude_vm_bundle_orphaned` → Tasks 1–2；apply 重判 → Task 3；coverage/README/版本 → Task 4。
2. **Placeholders:** 无 TBD；glob `**` 限制已用自举 walk 解决。
3. **Types:** `select_orphaned_paths` 新增 `claude_age_days` 参数，Task 2/3/handlers 一致；`OrphanDeps::claude_desktop_running` Task 1 定义。

---

## Execution Handoff

Plan complete and saved to `docs/wukong-code/plans/2026-08-05-1829-b41-claude-vm-orphan.md`.

按仓库惯例默认 **Inline Execution**（executing-plans）。
