# Install macOS*.app Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vole clean` 可安全清理过期的 `/Applications/Install macOS*.app`（SWU fail-closed + 大版本 keep + age≥14 + 运行中跳过；特权永久删），发版 **1.27.0**。

**Architecture:** 在 `vole-core::privilege` 增 `install-macos-apps` 规则（同 GPU Metal 刀法）：可注入 apps root / SWU plist / 当前大版本；`plan.rs` 走 `*_plan_candidates()`；`apply_plan.rs` 重判门控后 `ensure_privilege_ready` + `sudo -n` permanent。永不删 `/Library/Updates`、`/macOS Install Data`。

**Tech Stack:** Rust / macOS / `plist` crate / 既有 `PrivilegeBackend` / `ProcessProbe`（`pgrep -f`）

## Global Constraints

- 版本：**1.27.0**（MINOR）；规则 **531 → 532**；**不 bump** `schema_version`
- 删除：特权 **permanent** + `sudo -n`；plan **零** sudo
- SWU：仅显式 `RecommendedUpdates == []` 才清理；否则整规则零候选
- 非目标：`$HOME/Applications`、analyze 大文件提示、Updates / Install Data、SMAppService
- 合并：`gh pr merge --merge`（禁止 squash）；**默认不打 tag**
- 全程中文进度；task-level commit；PR 前 **security-review**

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/privilege/mod.rs` | 常量、谓词、SWU/age/version、`install_macos_apps_plan_candidates`、allowlist 接线 |
| `crates/vole-core/src/ops/plan.rs` | rule_id 分支调用 candidates + plan 单测 |
| `crates/vole-core/src/ops/apply_plan.rs` | apply 重判分支 + 单测 |
| `crates/vole-core/src/ops/coverage.rs` | 落地句 / 去掉未移植 Install macOS |
| `data/rules/user-devtools.toml` | 新规则 TOML |
| `Cargo.toml` / `Formula/vole.rb` / `README.md` | 版本与规则数 |
| `docs/releases/v1.27.0.md` + findings | 发版说明 |
|（已有）`docs/wukong-code/specs/2026-08-07-2335-install-macos-apps-design.md` | 设计依据 |

---

## Task 1: SWU probe + path 谓词 + allowlist

**Files:**
- Modify: `crates/vole-core/src/privilege/mod.rs`

**Interfaces:**
- Produces:
  - `pub const INSTALL_MACOS_APPS_RULE_ID: &str = "install-macos-apps";`
  - `pub const INSTALL_MACOS_APP_AGE_DAYS: u32 = 14;`
  - `pub fn applications_root() -> PathBuf` — `VOLE_TEST_APPLICATIONS` 或 `/Applications`
  - `pub fn software_update_plist_path() -> PathBuf` — `VOLE_TEST_SOFTWARE_UPDATE_PLIST` 或 `/Library/Preferences/com.apple.SoftwareUpdate.plist`
  - `pub fn current_macos_major() -> Option<String>` — `VOLE_TEST_MACOS_MAJOR` 或 `sw_vers -productVersion` 主版本
  - `pub fn software_update_pending_or_unknown(plist: &Path) -> bool` — `true` = **阻塞**清理
  - `pub fn is_install_macos_app_bundle(path: &Path, apps_root: &Path) -> bool`
  - allowlist：`path_allowed_for_privilege` 接受生产形状 `/Applications/Install macOS*.app` **或** 测试根下同形（用 `applications_root()` 前缀）

- [ ] **Step 1: RED** — 在 `privilege/mod.rs` 的 `#[cfg(test)] mod tests` 增加：

```rust
#[test]
fn swu_empty_array_not_pending() {
    let _guard = crate::test_env::lock();
    let dir = tempfile::tempdir().unwrap();
    let plist = dir.path().join("com.apple.SoftwareUpdate.plist");
    // 写入含 RecommendedUpdates = [] 的 XML/binary plist（可用 plist::Value）
    assert!(!software_update_pending_or_unknown(&plist));
}

#[test]
fn swu_missing_file_is_pending() {
    let missing = PathBuf::from("/tmp/vole-no-such-swu-plist-xyz");
    assert!(software_update_pending_or_unknown(&missing));
}

#[test]
fn swu_nonempty_recommended_is_pending() {
    // RecommendedUpdates 含至少一项 → true
}

#[test]
fn is_install_macos_app_bundle_shape() {
    let root = Path::new("/Applications");
    assert!(is_install_macos_app_bundle(
        Path::new("/Applications/Install macOS Sequoia.app"),
        root
    ));
    assert!(!is_install_macos_app_bundle(
        Path::new("/Applications/Safari.app"),
        root
    ));
    assert!(!is_install_macos_app_bundle(
        Path::new("/tmp/Install macOS Sequoia.app"),
        root
    ));
}

#[test]
fn allowlist_accepts_install_macos_under_apps_root() {
    let _guard = crate::test_env::lock();
    let dir = tempfile::tempdir().unwrap();
    let apps = dir.path().join("Applications");
    std::fs::create_dir_all(&apps).unwrap();
    let app = apps.join("Install macOS Fixtures.app");
    std::fs::create_dir_all(&app).unwrap();
    std::env::set_var("VOLE_TEST_APPLICATIONS", &apps);
    assert!(path_allowed_for_privilege(&app));
    assert!(!path_allowed_for_privilege(Path::new(
        "/tmp/Install macOS Evil.app"
    )));
    std::env::remove_var("VOLE_TEST_APPLICATIONS");
}
```

- [ ] **Step 2: 跑测确认 RED**

```bash
cargo test -p vole-core swu_empty_array_not_pending -- --nocapture
```

Expected: FAIL（函数未定义）

- [ ] **Step 3: GREEN** — 实现：

`software_update_pending_or_unknown`：
1. `!plist.is_file()` → `true`
2. `plist::Value::from_file` 失败 → `true`
3. 取 `RecommendedUpdates`；缺失或非 Array → `true`
4. Array 长度为 0 → `false`；否则 `true`

`is_install_macos_app_bundle`：
- `path` 绝对、canonical 前缀等于 `apps_root`（normalize 两者）
- 文件名匹配：以 `Install macOS` 开头且以 `.app` 结尾（`OsStr`/`str`）
- 禁止 `..` 组件；根必须是目录时由 callers 再查 `is_dir` / symlink

`applications_root` / `software_update_plist_path` / `current_macos_major`：读 env；生产默认如上。`current_macos_major` 用 `Command::new("sw_vers").arg("-productVersion")`，取 `.` 前段；失败 → `None`。

allowlist 分支：若 `is_install_macos_app_bundle(path, &applications_root())` → 允许（与其他 OR 并列）。

- [ ] **Step 4: 跑测 GREEN**

```bash
cargo test -p vole-core swu_ allowlist_accepts_install_macos is_install_macos -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/privilege/mod.rs
git commit -m "$(cat <<'EOF'
feat(privilege): SWU fail-closed probe and Install macOS*.app allowlist

EOF
)"
```

---

## Task 2: select candidates（age / version / running）+ TOML + plan 接线

**Files:**
- Modify: `crates/vole-core/src/privilege/mod.rs`
- Modify: `crates/vole-core/src/ops/plan.rs`
- Modify: `data/rules/user-devtools.toml`（文件末尾 `gpu-metal-caches` 附近追加）

**Interfaces:**
- Consumes: Task 1 全部
- Produces:
  - `pub fn installer_matches_current_macos_major(app: &Path, current_major: Option<&str>) -> bool`
  - `pub fn installer_age_days(app: &Path, now: SystemTime) -> Option<u64>` — 根 mtime；失败 → `None`（保守：不选入）
  - `pub fn install_macos_apps_plan_candidates() -> Vec<PathBuf>`
  - 内部可测：`install_macos_apps_plan_candidates_with(apps_root, swu_plist, current_major, now, running: &dyn ProcessProbe) -> Vec<PathBuf>`

门控顺序（写死）：
1. 若 `software_update_pending_or_unknown(swu_plist)` → **立即返回空 Vec**
2. 读 `apps_root` 下匹配 `Install macOS*.app` 的**目录**条目（跳过非 dir；根为 symlink → 跳过）
3. 对每项：`ProcessProbe::cmdline_substring_running(app.display())` 非 Idle（Running/Unknown）→ skip
4. `installer_matches_current_macos_major`：读 `app/Contents/Info.plist` 的 `DTPlatformVersion` 主版本；与 `current_major` 均 `Some` 且相等 → keep/skip
5. age：`installer_age_days < 14` → skip；`None` → skip
6. 否则 push

版本 keep：缺 plist / 缺键 → **不 keep**（继续年龄）。

- [ ] **Step 1: RED** — privilege 单测：

```rust
#[test]
fn select_skips_all_when_swu_pending() { /* SWU 非空 → 即便有过期 installer → empty */ }

#[test]
fn select_includes_stale_unrelated_major_when_swu_clear() {
    // VOLE_TEST_APPLICATIONS + SWU [] + VOLE_TEST_MACOS_MAJOR=15
    // 建 Install macOS Old.app，DTPlatformVersion=14.x，mtime 旧 20 天
    // StubProcessProbe idle → len==1
}

#[test]
fn select_keeps_matching_major() { /* major=15, DT=15 → empty */ }

#[test]
fn select_keeps_young() { /* mtime now → empty */ }
```

对 mtime：`filetime` 若项目已有则用；否则 `std::process::Command` `touch -t`；或依赖注入 `now` 相对回拨 metadata（macOS `filetime` crate 检查 Cargo.toml——若无则在测试里用 `libc::utimes` / `std::fs::File` 不足以设过去时，优先加测试用 `filetime` 若已在 dev-deps，否则用「把 now 设成未来」即 `now = mtime + 20 days` 注入，**不必改真实 mtime**）。

推荐：**注入 `now: SystemTime`**，fixture 用「现在」创建 → `now = SystemTime::now() + Duration::from_secs(20*86400)` 即视为已 20 天。

Running：用 `process_guard::RecordingProcessProbe`（或现有 stub）把 app path 放进 `cmdline_running`。

- [ ] **Step 2: 跑测 RED**

```bash
cargo test -p vole-core select_skips_all_when_swu -- --nocapture
```

- [ ] **Step 3: GREEN** — 实现 helpers + `install_macos_apps_plan_candidates` / `_with`。

`DTPlatformVersion`：`plist::Value::from_file(app.join("Contents/Info.plist"))` → 字典 string；主版本 `split('.').next()`。

`plan.rs`：在 GPU metal 分支旁：

```rust
} else if rule.id == crate::privilege::INSTALL_MACOS_APPS_RULE_ID {
    crate::privilege::install_macos_apps_plan_candidates()
} else {
```

TOML：

```toml
[[rule]]
id = "install-macos-apps"
category = "user-devtools"
label = "Old macOS installer apps"
platform = ["macos"]
paths = ["/Applications"]
impact = "Install macOS*.app ≥14d, SWU clear, not current major, not running; privileged permanent delete"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "all"
```

plan 测：`plan_install_macos_apps_enters_under_test_applications`（设 env、清 env、断言 rule_id + path）。

- [ ] **Step 4: 跑测 GREEN**

```bash
cargo test -p vole-core select_ install_macos plan_install_macos -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/privilege/mod.rs crates/vole-core/src/ops/plan.rs data/rules/user-devtools.toml
git commit -m "$(cat <<'EOF'
feat(clean): plan candidates for Install macOS*.app with Mole gates

EOF
)"
```

---

## Task 3: apply 重判 + 特权永久删

**Files:**
- Modify: `crates/vole-core/src/ops/apply_plan.rs`

**Interfaces:**
- Consumes: Task 1–2 谓词 / age / version / SWU / allowlist / `INSTALL_MACOS_APPS_RULE_ID`
- Produces: apply 分支（无新公开 API）

Apply 顺序（对齐 GPU metal）：
1. `is_install_macos_app_bundle` + `is_dir` + allowlist；否 → skip PathVanished
2. `software_update_pending_or_unknown(&software_update_plist_path())` → skip（PathVanished 或既有语义；推荐 PathVanished 与其它 recheck 一致，**不要** NeedsPrivilege）
3. running：`LiveProcessProbe` 或 ctx 若已有 probe 则用之；非 Idle → skip AppRunning（若 SkipReason 有）或 PathVanished
4. `installer_matches_current_macos_major(&path, current_macos_major().as_deref())` → skip
5. age &lt; 14（用 `SystemTime::now()`）→ skip
6. `ensure_privilege_ready` → else NeedsPrivilege
7. `verify_plan_entry` → mole_delete_verified permanent + needs_sudo

**禁止**任何路径字符串包含删除 `/Library/Updates` 或 `/macOS Install Data`。

- [ ] **Step 1: RED**

```rust
#[test]
fn apply_install_macos_removes_when_probe_ok() {
    // fixture app + SWU [] + major mismatch + now 远未来等价年龄
    // RecordingPrivilege::allowing()
    // 断言 remove 记录含该 path；目录被删或 Recording 记 remove
}

#[test]
fn apply_install_macos_skips_when_swu_pending() {
    // 即使 Recording allowing，也不 remove
}

#[test]
fn apply_install_macos_rejects_off_apps_root() {
    // rule_id 正确但 path=/tmp/... → skip，零 remove
}
```

- [ ] **Step 2: 跑测 RED**

```bash
cargo test -p vole-core apply_install_macos -- --nocapture
```

- [ ] **Step 3: GREEN** — 在 `apply_plan.rs` 加入与 `GPU_METAL_CACHES_RULE_ID` 同结构的分支；import 新常量与函数。

运行中探测：优先复用 `ctx` 已有 process probe；若无字段则局部 `crate::rules::process_guard::LiveProcessProbe`（与 guards 一致 fail-closed：Unknown → skip）。

- [ ] **Step 4: 跑测 GREEN**

```bash
cargo test -p vole-core apply_install_macos privilege::tests::allowlist_accepts_install -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/apply_plan.rs
git commit -m "$(cat <<'EOF'
feat(apply): privilege permanent delete for Install macOS*.app

EOF
)"
```

---

## Task 4: coverage / 1.27.0 文档 + PR merge

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs`
- Modify: `Cargo.toml`、`Cargo.lock`、`Formula/vole.rb`、`README.md`（531→532，成熟度 1.27.0）
- Create: `docs/releases/v1.27.0.md`
- Create: `docs/findings/2026-08-install-macos-apps.md`
- Modify: spec 状态行改为「已实现」可选

coverage：
- **已落地**：`Install macOS*.app（age≥14 + SWU fail-closed + 当前大版本 keep）`
- **仍未移植**：仅 `桌面 SMAppService / 特权助手`（去掉 Install macOS）
- 更新 `coverage_note_mentions_mole_and_count`：仍要求提到未移植桌面；**不得**再要求 Install macOS 在 unported

- [ ] **Step 1: RED** — 改 coverage 测期望后先跑失败（若文案未改）

- [ ] **Step 2: GREEN** — 改文案 + 版本 bump + release/findings

findings 要点：macOS 27 beta SWU fail-closed 动机；永不碰 Updates/Install Data。

- [ ] **Step 3: 全量特权相关测 + fmt**

```bash
cargo test -p vole-core install_macos swu_ plan_install apply_install coverage_note -- --nocapture
cargo fmt --all -- --check
```

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock Formula/vole.rb README.md \
  crates/vole-core/src/ops/coverage.rs \
  docs/releases/v1.27.0.md docs/findings/2026-08-install-macos-apps.md
git commit -m "$(cat <<'EOF'
chore(release): bump 1.27.0 for Install macOS*.app cleanup

EOF
)"
```

- [ ] **Step 5: PR + security-review + CI + merge commit**

```bash
git push -u origin HEAD
gh pr create --title "feat(clean): Install macOS*.app cleanup (1.27.0)" --body "..."
# security-review subagent on branch changes
# wait CI green
gh pr merge <N> --merge --delete-branch
```

---

## Spec coverage (self-review)

| Spec 要求 | Task |
|---|---|
| SWU fail-closed | T1 + T2/T3 recheck |
| age ≥ 14 | T2 + T3 |
| 大版本 keep | T2 + T3 |
| 运行中跳过 | T2 + T3 |
| 特权 permanent sudo -n | T3 |
| `VOLE_TEST_APPLICATIONS` | T1–T3 |
| 永不 Updates / Install Data | T3 禁区 + findings |
| coverage / 1.27.0 / 532 | T4 |
| security-review + merge commit | T4 |
| 非目标 analyze / HOME/Applications | 无任务（刻意不做） |

无 TBD / 类型名前后一致：`INSTALL_MACOS_APPS_RULE_ID`、`software_update_pending_or_unknown`、`install_macos_apps_plan_candidates`。
