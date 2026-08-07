# Uninstall Brew Cask Linkage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vole uninstall` 对 Homebrew Cask 管理的应用走 `brew uninstall --cask [--zap]` 联动；发版 **1.30.0**。

**Architecture:** 新模块 `vole-core::brew_cask`（`BrewDeps` 可注入）负责检测与卸载；`uninstall_plan` 把本体 `rule_id` 编码为 `uninstall:brew-cask:{zap|nozap}:{token}`；`uninstall_apply` 解析后调 brew，失败且 cask 仍登记则 skip、仅 cask 已卸才回退 `mole_delete_verified`。保护层不绕过。

**Tech Stack:** Rust / macOS / Homebrew CLI / 既有 uninstall plan→apply

## Global Constraints

- 版本：**1.30.0**；**不 bump** `schema_version`
- 仅 W2a①（brew cask）；**不**做 login items / LaunchDaemons / W1 / W2b / W2c
- 保护：`should_protect_from_uninstall` / official uninstaller / `UninstallPathProtection` / TOCTOU 全程有效
- sibling → **nozap**；无 sibling → **zap**
- brew 失败且 `is_cask_installed==true/unknown` → **禁止** mole_delete app
- 测：FakeBrewDeps；不下真 brew；可缩短超时
- 合并：`gh pr merge --merge --delete-branch`；security-review；默不打 tag
- 冲突文件窄改：Cargo.toml / Formula / coverage
- 全程中文；task-level commit

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/brew_cask/mod.rs` | NEW：检测、rule_id 编解码、BrewDeps、uninstall |
| `crates/vole-core/src/lib.rs` | `pub mod brew_cask;` |
| `crates/vole-core/src/ops/uninstall_plan.rs` | detect + rule_id + coverage_note |
| `crates/vole-core/src/ops/uninstall_apply.rs` | brew 分支 + 回退策略 |
| `crates/vole-core/src/ops/coverage.rs` | uninstall 长尾诚实句 |
| `README.md` / `docs/releases/v1.30.0.md` / findings | 版本与长尾 |
| `Cargo.toml` / `Formula/vole.rb` | 1.30.0 |

---

### Task 1: `brew_cask` 模块 — token / 编解码 / Caskroom 抽取

**Files:**
- Create: `crates/vole-core/src/brew_cask/mod.rs`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub const BREW_CASK_RULE_PREFIX: &str = "uninstall:brew-cask:";`
  - `pub enum ZapMode { Zap, NoZap }`
  - `pub fn is_valid_cask_token(token: &str) -> bool` — `^[a-z0-9][a-z0-9-]*$`
  - `pub fn extract_cask_token_from_caskroom_path(path: &Path) -> Option<String>`
  - `pub fn encode_brew_cask_rule_id(mode: ZapMode, token: &str) -> String` — `uninstall:brew-cask:zap|nozap:{token}`
  - `pub fn parse_brew_cask_rule_id(rule_id: &str) -> Option<(ZapMode, String)>`

- [ ] **Step 1: RED** — 创建 `brew_cask/mod.rs`，先放测试（函数先可不存在让编译挂，或 stub `todo!`）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn token_validation() {
        assert!(is_valid_cask_token("visual-studio-code"));
        assert!(is_valid_cask_token("iterm2"));
        assert!(!is_valid_cask_token("Visual-Studio"));
        assert!(!is_valid_cask_token(""));
        assert!(!is_valid_cask_token("-bad"));
    }

    #[test]
    fn extract_token_from_caskroom() {
        assert_eq!(
            extract_cask_token_from_caskroom_path(Path::new(
                "/opt/homebrew/Caskroom/iterm2/3.5.0/iTerm.app"
            ))
            .as_deref(),
            Some("iterm2")
        );
        assert_eq!(
            extract_cask_token_from_caskroom_path(Path::new(
                "/usr/local/Caskroom/foo-bar/1.0/Foo.app"
            ))
            .as_deref(),
            Some("foo-bar")
        );
        assert!(extract_cask_token_from_caskroom_path(Path::new(
            "/Applications/Foo.app"
        ))
        .is_none());
    }

    #[test]
    fn rule_id_roundtrip() {
        let id = encode_brew_cask_rule_id(ZapMode::Zap, "iterm2");
        assert_eq!(id, "uninstall:brew-cask:zap:iterm2");
        assert_eq!(
            parse_brew_cask_rule_id(&id),
            Some((ZapMode::Zap, "iterm2".into()))
        );
        let id2 = encode_brew_cask_rule_id(ZapMode::NoZap, "iterm2");
        assert_eq!(
            parse_brew_cask_rule_id(&id2),
            Some((ZapMode::NoZap, "iterm2".into()))
        );
        assert!(parse_brew_cask_rule_id("uninstall:com.example").is_none());
    }
}
```

- [ ] **Step 2: 跑测 RED**

```bash
cargo test -p vole-core token_validation -- --nocapture
```

Expected: FAIL（模块/符号不存在）

- [ ] **Step 3: GREEN** — 实现校验、抽取、编解码；`lib.rs` 加 `pub mod brew_cask;`

抽取：路径字符串前缀 `/opt/homebrew/Caskroom/` 或 `/usr/local/Caskroom/`；去掉前缀后第一段为 token，且 `is_valid_cask_token`。

- [ ] **Step 4: 跑测 GREEN**

```bash
cargo test -p vole-core -- token_validation extract_token rule_id_roundtrip -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/brew_cask/mod.rs crates/vole-core/src/lib.rs
git commit -m "feat(brew_cask): add token parse and rule_id codec"
```

---

### Task 2: `BrewDeps` + Stage1/3 检测 + Fake

**Files:**
- Modify: `crates/vole-core/src/brew_cask/mod.rs`

**Interfaces:**
- Consumes: Task 1 抽取/校验
- Produces:
  - `pub enum CaskInstallState { Installed, NotInstalled, Unknown }`
  - `pub trait BrewDeps: Send + Sync { fn brew_available(&self) -> bool; fn list_casks(&self) -> Result<Vec<String>, ()>; fn cask_info(&self, token: &str) -> Result<String, ()>; fn is_cask_installed(&self, token: &str) -> CaskInstallState; fn uninstall_cask(&self, token: &str, mode: ZapMode, app_path: Option<&Path>) -> Result<(), String>; fn resolve_path(&self, path: &Path) -> Option<PathBuf>; fn read_symlink(&self, path: &Path) -> Option<PathBuf>; fn find_caskroom_apps(&self, app_bundle_name: &str) -> Vec<PathBuf>; }`
  - `pub struct LiveBrewDeps;`（可先 stub resolve/readlink/find；uninstall 真调用放到 Task 3）
  - `pub fn detect_cask_name(deps: &dyn BrewDeps, app_path: &Path) -> Option<String>`

检测顺序（design §5）：

1. `!brew_available` → None  
2. Stage1：`resolve_path(app)` → `extract_cask_token_from_caskroom_path`  
3. Stage2：`find_caskroom_apps(basename)` → 抽 token 去重；恰 1 个且 `list_casks` 含之且 `cask_info` 含路径或 `/Applications/{name}` 或 basename → Some；否则跳过本 stage  
4. Stage3：`read_symlink` → extract  
5. Stage4：`list_casks` 中与 `basename.strip_suffix(.app).to_ascii_lowercase()` 精确（大小写不敏感）匹配一项；`cask_info` 交叉验证  

- [ ] **Step 1: RED**

```rust
#[test]
fn detect_stage1_resolved_caskroom() {
    let app = PathBuf::from("/Applications/Foo.app");
    let deps = FakeBrewDeps {
        available: true,
        resolve: Some(PathBuf::from("/opt/homebrew/Caskroom/foo/1.0/Foo.app")),
        ..FakeBrewDeps::empty()
    };
    assert_eq!(detect_cask_name(&deps, &app).as_deref(), Some("foo"));
}

#[test]
fn detect_none_when_brew_missing() {
    let deps = FakeBrewDeps { available: false, ..FakeBrewDeps::empty() };
    assert!(detect_cask_name(&deps, Path::new("/Applications/Foo.app")).is_none());
}

#[test]
fn detect_stage2_ambiguous_tokens_none() {
    let deps = FakeBrewDeps {
        available: true,
        find_hits: vec![
            PathBuf::from("/opt/homebrew/Caskroom/a/1/Foo.app"),
            PathBuf::from("/opt/homebrew/Caskroom/b/1/Foo.app"),
        ],
        ..FakeBrewDeps::empty()
    };
    assert!(detect_cask_name(&deps, Path::new("/Applications/Foo.app")).is_none());
}
```

- [ ] **Step 2: 跑测 RED** — `cargo test -p vole-core detect_stage1 -- --nocapture` → FAIL

- [ ] **Step 3: GREEN** — 实现 `FakeBrewDeps`（test-only 或 `#[cfg(test)]`）、`detect_cask_name`；`LiveBrewDeps`：`brew_available`=`Command which brew`；`resolve_path`=`fs::canonicalize`；`read_symlink`=`fs::read_link`（相对链按 dirname 拼）；`find_caskroom_apps` 扫两 Caskroom maxdepth 3 同名；`list_casks`/`cask_info`/`is_cask_installed` 调 brew（env 三件套）；`uninstall_cask` 可先 `Err("todo")` 留给 Task 3

- [ ] **Step 4: GREEN 测** — `cargo test -p vole-core -- detect_stage detect_none -- --nocapture`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(brew_cask): multi-stage cask detection with BrewDeps"
```

---

### Task 3: `LiveBrewDeps::uninstall_cask` + 超时启发式

**Files:**
- Modify: `crates/vole-core/src/brew_cask/mod.rs`

**Interfaces:**
- Produces:
  - `pub fn brew_uninstall_timeout_secs(app_path: Option<&Path>, size_bytes: u64) -> u64` — 默认 300；`size_bytes > 15GiB` → 900；`> 5GiB` → 600
  - `LiveBrewDeps::uninstall_cask`：`Command::new("brew").args(["uninstall","--cask"]).args(zap?).arg(token)`，env `HOMEBREW_NO_ENV_HINTS=1 HOMEBREW_NO_AUTO_UPDATE=1 NONINTERACTIVE=1`；可用 `wait_timeout` 或现有超时手段（查 crate 内类似；无则 `Command` + 线程 join 超时，超时当失败）

- [ ] **Step 1: RED**

```rust
#[test]
fn timeout_scales_with_size() {
    assert_eq!(brew_uninstall_timeout_secs(None, 0), 300);
    assert_eq!(brew_uninstall_timeout_secs(None, 6 * 1024 * 1024 * 1024), 600);
    assert_eq!(brew_uninstall_timeout_secs(None, 16 * 1024 * 1024 * 1024), 900);
}

#[test]
fn fake_uninstall_records_zap_flag() {
    let mut deps = FakeBrewDeps::empty();
    deps.available = true;
    deps.uninstall_cask("foo", ZapMode::Zap, None).unwrap();
    assert!(deps.last_uninstall.as_ref().unwrap().zap);
    deps.uninstall_cask("foo", ZapMode::NoZap, None).unwrap();
    assert!(!deps.last_uninstall.as_ref().unwrap().zap);
}
```

（若 Fake 已在 Task 2，本测调整字段名对齐实现。）

- [ ] **Step 2–4: RED→GREEN→测** `timeout_scales` + Fake 记录

- [ ] **Step 5: Commit** — `feat(brew_cask): implement brew uninstall with timeout`

---

### Task 4: plan 接线

**Files:**
- Modify: `crates/vole-core/src/ops/uninstall_plan.rs`

**Interfaces:**
- Consumes: `detect_cask_name`, `encode_brew_cask_rule_id`, `ZapMode`, `LiveBrewDeps`
- 改动：`build_uninstall_plan` 在写出本体 entry 前 detect；sibling → NoZap else Zap；`rule_id` 用 brew 编码否则 `uninstall:{bundle_id}`；label 可选加 `[Brew:{token}]`  
- coverage_note：`Long-tail not covered (use Mole): login items, system LaunchDaemons, /Library sudo paths.`（去掉 brew cask zap）；可含 `brew_cask={n}`

可选：为可测性给 `build_uninstall_plan` 增加 `&dyn BrewDeps` 参数，或 `build_uninstall_plan_with_brew(...)`；CLI 仍调带 `LiveBrewDeps` 的包装。**推荐** `build_uninstall_plan_with_deps(..., brew: &dyn BrewDeps)`，原 `build_uninstall_plan` 转为调 Live。

- [ ] **Step 1: RED**

```rust
#[test]
fn plan_marks_brew_cask_with_zap() {
    // fixture Foo.app；FakeBrewDeps stage1 → token foo；无 sibling
    // assert rule_id == "uninstall:brew-cask:zap:foo"
}

#[test]
fn plan_marks_nozap_when_sibling() {
    // 两份同 bundle_id app；Fake 返回 token；assert nozap
}

#[test]
fn coverage_note_drops_brew_cask_long_tail() {
    let plan = /* ... */;
    let note = plan.coverage_note.unwrap();
    assert!(!note.contains("brew cask"));
    assert!(note.contains("login items"));
}
```

- [ ] **Step 2–4: RED→实现→GREEN**

- [ ] **Step 5: Commit** — `feat(uninstall): tag brew cask apps in plan`

---

### Task 5: apply 接线

**Files:**
- Modify: `crates/vole-core/src/ops/uninstall_apply.rs`
- 若需注入：`UninstallApplyContext` 增 `brew: Option<&dyn BrewDeps>`（默认 Live）；或内部固定 Live + 测用 `apply_uninstall_proto_plan` 扩展参数

**行为（对 `parse_brew_cask_rule_id(entry.rule_id)` 为 Some 时）：**

1. 照常 `verify_plan_entry_for_apply`  
2. `deps.uninstall_cask(token, mode, Some(path))`  
3. Ok → succeeded（不重复 mole_delete，即使文件偶发残留：Mole 以 cask_gone && app_gone 为准；此处若 Ok 即 succeeded；若实现返回 Ok 但 app 仍在，可再验 `is_cask_installed`）  
4. Err：查 `is_cask_installed`：  
   - `NotInstalled` → 对该 path `mole_delete_verified`（回退）  
   - `Installed` | `Unknown` → skipped（记 PathVanished 或专用；**不得** delete）

非 brew rule_id：现有逻辑不变；仍要求 `rule_id.starts_with("uninstall:")`。

- [ ] **Step 1: RED**

```rust
#[test]
fn apply_brew_cask_calls_uninstall_zap() { /* Fake 记录 zap；app fixture 可预先「删掉」让成功路径简单 */ }

#[test]
fn apply_brew_fail_still_installed_skips_delete() {
    // uninstall Err；is_cask_installed=Installed；RecordingTrash 零调用
}

#[test]
fn apply_brew_fail_cask_gone_falls_back_delete() {
    // uninstall Err；NotInstalled；mole_delete 路径被走到（或 Trash 一次）
}
```

复用现有 apply 测夹具（见文件尾部测试）。

- [ ] **Step 2–4: RED→GREEN**

- [ ] **Step 5: Commit** — `feat(uninstall): apply brew cask uninstall with safe fallback`

---

### Task 6: coverage / README / 版本 1.30.0 / findings

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs`（「已落地」句加 uninstall brew cask 联动；长尾若曾写 brew 则删）
- Modify: `README.md` 成熟度行
- Create: `docs/releases/v1.30.0.md`
- Modify: `docs/findings/2026-07-v2-m1-uninstall.md`（① 完成）
- Modify: `Cargo.toml` workspace.version `1.30.0`
- Modify: `Formula/vole.rb` version / url 中的版本号（sha 可留占位，与既有发版一致）

- [ ] **Step 1:** 更新 coverage 文案；单测若断言「仍未移植」列表，确认不含错误项

- [ ] **Step 2:**

```bash
cargo test -p vole-core coverage -- --nocapture
rg -n 'brew cask zap' crates/vole-core/src/ops/uninstall_plan.rs # 应无
```

- [ ] **Step 3:** bump 版本与 release 笔记（要点：brew cask plan/apply；长尾剩 login items / LaunchDaemons）

- [ ] **Step 4: Commit** — `chore(release): bump to 1.30.0 for uninstall brew cask`

---

### Task 7: 全量验证 + PR

- [ ] **Step 1:**

```bash
cargo fmt --all -- --check
cargo test -p vole-core -- brew_cask uninstall
cargo test -p vole-cli --test uninstall_cli
# macOS 本地可再：
cargo clippy -p vole-core -p vole-cli -- -D warnings
```

- [ ] **Step 2:** 开 PR（base main）；body 含 Summary / Test plan；请求 security-review

- [ ] **Step 3:** CI 绿 + review 齐 → `gh pr merge --merge --delete-branch`；否则开好 PR 回报

---

## Spec coverage (self-review)

| Spec 项 | Task |
|---|---|
| 四级检测 | T2 |
| rule_id zap/nozap | T1/T4 |
| sibling → nozap | T4 |
| brew uninstall + env + timeout | T3 |
| 失败回退策略 | T5 |
| 保护不绕过 | T4/T5（沿用既有校验） |
| coverage/README/findings | T6 |
| 1.30.0 | T6 |
| 不做 ②③ | 无对应 task（显式禁） |

## Execution Handoff

Plan complete → **默认 Inline Execution**（用户习惯优先 inline；不征询）。下一步：`using-git-worktrees` 建 `feat/uninstall-brew-cask`，再 `executing-plans` 逐 task 落地。
