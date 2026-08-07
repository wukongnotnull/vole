# Time Machine Failed Backups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vole clean` 可清理陈旧（≥48h）的 Time Machine `*.inProgress` 失败中备份（HFS + 已挂载 bundle），apply 用 `tmutil delete`；发版 **1.28.0**。

**Architecture:** 新模块 `vole-core::tmbackup`（`TmDeps` 可注入）；规则 `tm-failed-backups` 经 `plan.rs` / `apply_plan.rs` 接线；**不**走 `PrivilegeBackend::remove_permanent` / `sudo rm`。门控与 Mole `clean_time_machine_failed_backups` 对齐。

**Tech Stack:** Rust / macOS / `tmutil` / `hdiutil` / `df` / 既有 plan→apply

## Global Constraints

- 版本：**1.28.0**；规则 **532 → 533**；**不 bump** `schema_version`
- 删除：**仅** `tmutil delete <path>`；失败 skip，不自动 sudo
- 安全窗：**48** 小时；mtime 不可读 → keep
- Running / status unknown → 整规则零候选（fail-closed）
- 非目标：本地快照删/报告、Updates/Install Data、网络卷、桌面 Helper
- 合并：`gh pr merge --merge`；security-review 必过；默不打 tag
- 全程中文进度；task-level commit

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/tmbackup/mod.rs` | NEW：常量、TmDeps、gate、选入、allowlist、delete |
| `crates/vole-core/src/lib.rs` | `pub mod tmbackup;` |
| `crates/vole-core/src/ops/plan.rs` | rule 分支 + PlanNotice（可选）+ 单测 |
| `crates/vole-core/src/ops/apply_plan.rs` | apply 分支（tmutil delete）+ 单测 |
| `crates/vole-core/src/ops/coverage.rs` | 落地 / 去掉 TM 失败备份未移植 |
| `data/rules/user-devtools.toml` | 新规则 |
| `Cargo.toml` / Formula / README / releases / findings | 1.28.0 |

---

## Task 1: `tmbackup` 模块 — deps / gate / shape

**Files:**
- Create: `crates/vole-core/src/tmbackup/mod.rs`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub const TM_FAILED_BACKUPS_RULE_ID: &str = "tm-failed-backups";`
  - `pub const TM_BACKUP_SAFE_HOURS: u64 = 48;`
  - `pub enum TmRunningState { Running, Idle, Unknown }`
  - `pub trait TmDeps: Send + Sync { fn tmutil_exists(&self) -> bool; fn auto_backup_configured(&self) -> bool; fn destination_configured(&self) -> bool; fn running_state(&self) -> TmRunningState; fn volumes_root(&self) -> PathBuf; fn fs_type(&self, vol: &Path) -> String; fn bundle_mount_point(&self, bundle: &Path) -> Option<PathBuf>; fn path_mtime(&self, path: &Path) -> Option<SystemTime>; fn dir_size_bytes(&self, path: &Path) -> u64; fn delete_backup(&self, path: &Path) -> Result<(), String>; }`
  - `pub struct LiveTmDeps;`
  - `pub fn gates_allow_scan(deps: &dyn TmDeps) -> bool` — steps 1–5；Running/Unknown → false
  - `pub fn is_tm_inprogress_dir_name(name: &str) -> bool`
  - `pub fn path_allowed_for_tm_delete(path: &Path) -> bool` — 形状闸口（可先只测 backupdb 叶形）

- [ ] **Step 1: RED** — 在 `tmbackup/mod.rs`（可先空 `mod` 让测放同文件 `#[cfg(test)]`）：

```rust
#[test]
fn gates_block_when_running() {
    let deps = FakeTmDeps { running: TmRunningState::Running, ..happy() };
    assert!(!gates_allow_scan(&deps));
}

#[test]
fn gates_block_when_unknown() { /* Unknown → false */ }

#[test]
fn gates_allow_when_idle_and_configured() {
    assert!(gates_allow_scan(&happy()));
}

#[test]
fn inprogress_name_matches() {
    assert!(is_tm_inprogress_dir_name("2024-01-01-120000.inProgress"));
    assert!(is_tm_inprogress_dir_name("x.inprogress"));
    assert!(!is_tm_inprogress_dir_name("2024-01-01-120000"));
}
```

- [ ] **Step 2: 跑测 RED**

```bash
cargo test -p vole-core gates_block_when_running -- --nocapture
```

Expected: FAIL（模块/函数不存在）

- [ ] **Step 3: GREEN** — 实现 `FakeTmDeps`（测试用，字段可调）、`gates_allow_scan`、命名谓词；`LiveTmDeps` 可 stub 到 Task 2（本任务至少 Fake + gates）。

`gates_allow_scan`：
1. `!tmutil_exists` → false  
2. `!auto_backup_configured` → false  
3. `!destination_configured` → false  
4. `!volumes_root.is_dir()` → false  
5. `running_state != Idle` → false  
6. else true  

- [ ] **Step 4: 跑测 GREEN**

```bash
cargo test -p vole-core -- gates_block gates_allow inprogress_name -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/tmbackup/mod.rs crates/vole-core/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(tmbackup): TmDeps gates and inProgress name predicate

EOF
)"
```

---

## Task 2: select candidates + plan 接线 + TOML

**Files:**
- Modify: `crates/vole-core/src/tmbackup/mod.rs`
- Modify: `crates/vole-core/src/ops/plan.rs`
- Modify: `data/rules/user-devtools.toml`
- Modify: `crates/vole-core/src/ops/plan.rs` — `PlanNotice::TimeMachineBusy`（可选；**不**改 vole-proto schema）

**Interfaces:**
- Consumes: Task 1
- Produces:
  - `pub fn select_tm_failed_backups(deps: &dyn TmDeps, now: SystemTime) -> Vec<PathBuf>`
  - `pub fn tm_failed_backups_plan_candidates() -> Vec<PathBuf>` — LiveTmDeps + now
  - `path_allowed_for_tm_delete` 完整化：含 `{vol}/Backups.backupdb/.../*.inProgress` 深度约束

选入（`gates_allow_scan` 已过之后）：
1. 列 volumes_root 下非 symlink 目录；跳过名为 `MacintoshHD` 等本机别名（对齐 Mole 对 `/Volumes/MacintoshHD`）；有 `Backups.backupdb` 或 `.MobileBackups`  
2. `fs_type` ∈ {nfs,smbfs,afpfs,cifs,webdav,unknown} → skip 卷  
3. 若存在 `Backups.backupdb`：walk maxdepth 3 目录，名 inProgress  
4. 对 `*.backupbundle`/`*.sparsebundle`：`bundle_mount_point` → Some 则同样 walk  
5. mtime/`hours_old`：`now.duration_since(mtime).as_secs()/3600 >= 48`；mtime None → skip；size==0 → skip  

- [ ] **Step 1: RED**

```rust
#[test]
fn select_finds_stale_backupdb_inprogress() {
    // tempfile: Volumes/VolA/Backups.backupdb/Host/x.inProgress
    // FakeTmDeps volumes_root=...; Idle; fs_type=apfs; mtime=now-49h; size>0
    assert_eq!(select_tm_failed_backups(&deps, now).len(), 1);
}

#[test]
fn select_skips_younger_than_48h() { /* 1h → empty */ }

#[test]
fn select_skips_network_fs() { /* fs_type=nfs → empty */ }
```

- [ ] **Step 2: RED run**

```bash
cargo test -p vole-core select_finds_stale -- --nocapture
```

- [ ] **Step 3: GREEN** — 实现 select；`plan.rs`：

```rust
} else if rule.id == crate::tmbackup::TM_FAILED_BACKUPS_RULE_ID {
    crate::tmbackup::tm_failed_backups_plan_candidates()
}
```

若 `!gates` 且原因为 Running/Unknown：可 push `PlanNotice::TimeMachineBusy`（需在构建 plan 时知状态——`select` 可返回 `SelectOutcome { paths, busy: bool }` 或 plan 内先 `gates` 再 select）。**推荐**：

```rust
pub struct TmSelectResult {
    pub paths: Vec<PathBuf>,
    pub skipped_busy: bool, // Running or Unknown
}
```

plan 循环：`skipped_busy` → notices。coverage 可后续用。

TOML：

```toml
[[rule]]
id = "tm-failed-backups"
category = "user-devtools"
label = "Incomplete Time Machine backups"
platform = ["macos"]
paths = ["/Volumes"]
impact = "Stale *.inProgress under Backups.backupdb / mounted backup bundles (≥48h); tmutil delete"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "all"
```

plan 测：`plan_tm_failed_backups_enters_under_test_volumes`（需 Live 路径走 Fake——因此 `tm_failed_backups_plan_candidates` 在测试用 env `VOLE_TEST_TM_FAKE=1` 过重；**更好**：plan 测直接设测依赖困难，则 privilege 式只测 `select_*` + 轻量 plan 测用 `std::env` 勾 `LiveTmDeps` 读 `VOLE_TEST_VOLUMES` 并对真实 `tmutil` skip——**强制**：`LiveTmDeps` 在 `VOLE_TEST_VOLUMES` 设置时，`running_state=Idle`、`auto/destination=true`、`tmutil_exists=true`、`fs_type=apfs`（测试捷径写进 Live 或 TestTmDeps 经 thread_local——YAGNI：plan 测可只断言 rule 存在于 catalog；选入以 unit 为准）。

**最低要求**：select 单测充分；plan 接线有编译路径；可选 integration plan 测用注入 env：`VOLE_TEST_VOLUMES` + `VOLE_TEST_TM_IDLE=1` 让 Live 短路探针。

- [ ] **Step 4: GREEN run**

```bash
cargo test -p vole-core -- select_finds select_skips tm_failed -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/tmbackup crates/vole-core/src/ops/plan.rs data/rules/user-devtools.toml
git commit -m "$(cat <<'EOF'
feat(clean): plan candidates for stale Time Machine inProgress backups

EOF
)"
```

---

## Task 3: apply — `tmutil delete` + allowlist

**Files:**
- Modify: `crates/vole-core/src/ops/apply_plan.rs`
- Modify: `crates/vole-core/src/tmbackup/mod.rs`（`delete` + recheck helpers）

**Interfaces:**
- Consumes: `TM_FAILED_BACKUPS_RULE_ID`, `path_allowed_for_tm_delete`, `TM_BACKUP_SAFE_HOURS`, `TmDeps::delete_backup` / age helpers
- Produces: apply 分支

Apply 顺序：
1. `path_allowed_for_tm_delete` + `is_dir`；否 → PathVanished skip  
2. Live（或 ctx 可扩展）`running_state`：非 Idle → skip（可用 PathVanished 或现有；推荐 **不新增** SkipReason；busy → PathVanished 或复用 Timeout——**指定 PathVanished**；人读靠 notice）  
3. age &lt; 48h / mtime bad → skip  
4. `deps.delete_backup(&path)` → Ok 成功；Err → skip + deletion_log  
5. **禁止** ensure_privilege_ready / mole_delete  

测试用 `FakeTmDeps { deleted: Mutex<Vec<PathBuf>> }`。

- [ ] **Step 1: RED**

```rust
#[test]
fn apply_tm_failed_deletes_when_idle() { /* fixture + Fake delete 记录 */ }

#[test]
fn apply_tm_failed_skips_when_running() { /* 零 delete */ }

#[test]
fn apply_tm_failed_rejects_off_shape() { /* /tmp/x.inProgress + rule_id → 零 delete */ }
```

**接线注意**：apply 需持有 `&dyn TmDeps`。选项：
- **A.** `ApplyPlanContext` 新增 `pub tm_deps: Option<&'a dyn TmDeps>`，默认 `LiveTmDeps`  
- **B.** apply 分支内直接 `LiveTmDeps`，测试用 `std::env` 短路  

**选 A**（与 orphan_deps 一致）。

- [ ] **Step 2: RED run**

```bash
cargo test -p vole-core apply_tm_failed -- --nocapture
```

- [ ] **Step 3: GREEN** — context 字段 + 分支 + Fake 测

- [ ] **Step 4: GREEN run**

```bash
cargo test -p vole-core apply_tm_failed path_allowed_for_tm -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/apply_plan.rs crates/vole-core/src/tmbackup
git commit -m "$(cat <<'EOF'
feat(apply): tmutil delete for tm-failed-backups

EOF
)"
```

---

## Task 4: coverage / 1.28.0 / PR

**Files:**
- `coverage.rs`、`Cargo.toml`、`Formula/vole.rb`、`README.md`（532→533）
- Create: `docs/releases/v1.28.0.md`、`docs/findings/2026-08-tm-failed-backups.md`

coverage：
- 落地：`Time Machine 失败中备份（≥48h inProgress + tmutil delete）`
- 仍未移植：本地快照报告、桌面 SMAppService  

- [ ] **Step 1–3:** 改文案 → 测通过 → bump 版本 → findings（写明 fail-closed / 禁止 sudo rm）

```bash
cargo test -p vole-core -- tmbackup apply_tm_failed coverage_note select_finds -- --nocapture
cargo fmt --all -- --check
```

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
chore(release): bump 1.28.0 for Time Machine failed backups

EOF
)"
```

- [ ] **Step 5: PR + security-review + CI + `gh pr merge --merge --delete-branch`**

---

## Spec coverage (self-review)

| Spec | Task |
|---|---|
| gates fail-closed | T1 |
| 48h / mtime keep | T2–T3 |
| HFS + mounted bundles | T2 |
| tmutil delete only | T3 |
| plan 零 delete | T2（无 delete 调用） |
| coverage / 1.28.0 / 533 | T4 |
| security-review | T4 |

无 TBD；接口名前后一致：`TM_FAILED_BACKUPS_RULE_ID`、`select_tm_failed_backups` / `TmSelectResult`。
