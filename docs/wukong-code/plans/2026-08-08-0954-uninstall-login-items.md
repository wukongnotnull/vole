# Uninstall Login Items Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vole uninstall` 在 plan→apply 上交付 Login Items 按名移除 + LoginItems helper `launchctl bootout`；发版 **1.34.0**。

**Architecture:** 新模块 `vole-core::login_items`（`LoginItemDeps` 可注入）负责发现、rule_id 编解码与真实/假动作；`uninstall_plan` 为每个合格 app 追加侧车条目；`uninstall_apply` 解析后调 deps（fail-soft / NeedsPrivilege），**不**对该侧车 `mole_delete`。保护与 sibling/`guard_login` 名冲突守卫对齐 Mole。

**Tech Stack:** Rust / macOS / `osascript` + `launchctl` / 既有 uninstall plan→apply

## Global Constraints

- 版本：**1.34.0**；**不 bump** `schema_version`
- 仅 W2a②（login items）；**不**做 LaunchDaemons / `/Library`（W2a③）、clean/optimize/status
- 保护：`should_protect_from_uninstall` / official uninstaller / `UninstallPathProtection` / TOCTOU 全程有效
- 同 bundle sibling → 不发 helper 侧车；display name 与 survivor 冲突 → 不发 `login-item:name`
- apply 再检 sibling/名冲突；TCC → NeedsPrivilege skip，不阻塞其余条目
- 测：FakeLoginItemDeps；不下真 osascript/launchctl
- 合并：`gh pr merge --merge --delete-branch`；security-review；默不打 tag
- 冲突文件窄改：Cargo.toml / Formula / coverage / README / findings
- 全程中文；task-level commit
- 权威设计：`docs/wukong-code/specs/2026-08-08-0953-uninstall-login-items-design.md`

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/login_items/mod.rs` | NEW：发现、编解码、LoginItemDeps、Live/Fake |
| `crates/vole-core/src/lib.rs` | `pub mod login_items;` |
| `crates/vole-core/src/ops/uninstall_plan.rs` | 侧车条目 + coverage_note |
| `crates/vole-core/src/ops/uninstall_apply.rs` | login 分支 + 注入 deps |
| `crates/vole-core/src/ops/coverage.rs` | uninstall 长尾诚实句 |
| `README.md` / `docs/releases/v1.34.0.md` / findings | 版本与长尾 |
| `Cargo.toml` / `Formula/vole.rb` | 1.34.0 |

---

### Task 1: `login_items` 编解码 + discover 纯函数

**Files:**
- Create: `crates/vole-core/src/login_items/mod.rs`
- Modify: `crates/vole-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub const LOGIN_ITEM_NAME_PREFIX: &str = "uninstall:login-item:name:";`
  - `pub const LOGIN_HELPER_PREFIX: &str = "uninstall:login-helper:";`
  - `pub fn encode_login_item_name_rule_id(display_name: &str) -> String`
  - `pub fn parse_login_item_name_rule_id(rule_id: &str) -> Option<String>` — 百分号解码
  - `pub fn encode_login_helper_rule_id(bundle_id: &str) -> String`
  - `pub fn parse_login_helper_rule_id(rule_id: &str) -> Option<String>`
  - `pub fn percent_encode_token(s: &str) -> String` / `percent_decode_token`
  - `pub fn discover_login_item_helper_bundle_ids(app_path: &Path) -> Vec<(PathBuf, String)>` — 扫 `Contents/Library/LoginItems/*.app`，读 Info.plist CFBundleIdentifier（可用 `protection::read_bundle_id`），仅 reverse-DNS、排除 `com.apple.*`
  - `pub fn login_name_collides(display_name: &str, sibling_display_names: &[String]) -> bool` — 大小写不敏感相等

- [ ] **Step 1: RED** — 写测试（可先 stub）：

```rust
#[test]
fn name_rule_id_roundtrip_with_spaces() {
    let id = encode_login_item_name_rule_id("Foo Bar");
    assert!(id.starts_with("uninstall:login-item:name:"));
    assert_eq!(parse_login_item_name_rule_id(&id).as_deref(), Some("Foo Bar"));
}

#[test]
fn helper_rule_id_roundtrip() {
    let id = encode_login_helper_rule_id("com.example.helper");
    assert_eq!(id, "uninstall:login-helper:com.example.helper");
    assert_eq!(
        parse_login_helper_rule_id(&id).as_deref(),
        Some("com.example.helper")
    );
    assert!(parse_login_helper_rule_id("uninstall:com.example").is_none());
}

#[test]
fn discover_helpers_and_skips_apple() {
    // tempfile: App.app/Contents/Library/LoginItems/{Good,Evil}.app + Info.plist
    // Good → com.example.good；Evil → com.apple.Evil
    // assert only Good
}

#[test]
fn login_name_collides_case_insensitive() {
    assert!(login_name_collides("Foo", &["foo".into()]));
    assert!(!login_name_collides("Foo", &["Foo Beta".into()]));
}
```

- [ ] **Step 2: 跑测 RED** — `cargo test -p vole-core name_rule_id_roundtrip -- --nocapture` → FAIL

- [ ] **Step 3: GREEN** — 实现编解码（空格/非 ASCII 用 `%XX`；`:` 也编码避免与前缀冲突）、discover、collides；`lib.rs` 注册模块

- [ ] **Step 4: 跑测 GREEN**

```bash
cargo test -p vole-core -- name_rule_id helper_rule_id discover_helpers login_name_collides -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/login_items/mod.rs crates/vole-core/src/lib.rs
git commit -m "feat(login_items): add rule_id codec and helper discovery"
```

---

### Task 2: `LoginItemDeps` + Live + Fake

**Files:**
- Modify: `crates/vole-core/src/login_items/mod.rs`

**Interfaces:**
- Produces:
  - `pub enum LoginItemError { NeedsPrivilege, Failed(String) }`
  - `pub trait LoginItemDeps: Send + Sync { fn remove_login_item(&self, display_name: &str) -> Result<(), LoginItemError>; fn bootout_helper(&self, uid: u32, helper_bundle_id: &str) -> Result<(), LoginItemError>; }`
  - `pub struct LiveLoginItemDeps;` — `osascript`（System Events 按名删，名转义 `\`/`"`）；`launchctl bootout gui/{uid}/{id}`；超时有界（参考 brew_cask / optimize 现有 timeout 模式，默认 ~15–30s）
  - `#[cfg(test)] pub struct FakeLoginItemDeps` — 记录 `removed_names` / `booted_helpers`；可配置返回 Err

- [ ] **Step 1: RED**

```rust
#[test]
fn fake_remove_and_bootout_record_calls() {
    let fake = FakeLoginItemDeps::default();
    fake.remove_login_item("Foo").unwrap();
    fake.bootout_helper(501, "com.example.h").unwrap();
    assert_eq!(fake.removed_names.lock().unwrap().as_slice(), ["Foo"]);
    assert_eq!(
        fake.booted_helpers.lock().unwrap().as_slice(),
        &[(501, "com.example.h".into())]
    );
}

#[test]
fn live_skips_apple_namespace_in_bootout_guard() {
    // 若 Live 内有前置校验：直接 bootout_helper(..., "com.apple.x") → Ok(()) 且不 spawn
    // 或纯函数 assert！is_bootout_allowed("com.apple.x") == false
}
```

- [ ] **Step 2–4: RED→GREEN→测**

- [ ] **Step 5: Commit** — `feat(login_items): add LoginItemDeps with live and fake`

---

### Task 3: plan 侧车接线

**Files:**
- Modify: `crates/vole-core/src/ops/uninstall_plan.rs`

**改动：**
- 对每个合格 app：在本体 entry 之后（leftovers 之前或之后均可，推荐 **本体后、leftovers 前**）：
  1. 收集 sibling 显示名（读 sibling 路径的 display_name / stem）
  2. 若不 `login_name_collides` → 侧车 `uninstall:login-item:name:…`，label `Login Item: {name}`，size 0
  3. 若不 `siblings.has_siblings()` → discover helpers → 每条 `uninstall:login-helper:{id}`，label `LoginItems helper: {id}`，size 0
- coverage_note：`login_items={n}`；Long-tail 改为 `system LaunchDaemons, /Library sudo paths`（去掉 login items）
- 测试可继续用现有 fixture 模式；需要时 `build_uninstall_plan_with_brew` 旁可选不注入 login deps（发现为纯 FS）

- [ ] **Step 1: RED**

```rust
#[test]
fn plan_emits_login_item_and_helper_sidecar() {
    // fixture app + LoginItems helper；无 sibling
    // assert 存在 name 与 helper rule_id
}

#[test]
fn plan_skips_helper_when_sibling() {
    // 两份同 bundle；有 LoginItems → 无 helper 条目；name 是否发出取决于名冲突
}

#[test]
fn plan_skips_login_name_when_display_collides() {
    // sibling display 同名 → 无 login-item:name
}

#[test]
fn coverage_note_drops_login_items_long_tail() {
    let note = /* plan */.coverage_note.unwrap();
    assert!(!note.to_lowercase().contains("login items"));
    assert!(note.contains("LaunchDaemons") || note.contains("/Library"));
}
```

- [ ] **Step 2–4: RED→GREEN→测**

- [ ] **Step 5: Commit** — `feat(uninstall): plan login item sidecar entries`

---

### Task 4: apply 接线

**Files:**
- Modify: `crates/vole-core/src/ops/uninstall_apply.rs`

**改动：**
- `UninstallApplyContext` 增 `login_items: Option<&'a dyn LoginItemDeps>`（None→Live）
- 在 brew 分支之前（或之后独立匹配）：
  - `parse_login_item_name_rule_id` → 再检名冲突（需从 plan 同批或现场扫 sibling；推荐：apply 时对 entry.path 父 Applications + 同目录其它 `.app` 做与 plan 相同的 sibling/名冲突检测；路径简单策略：用 `find_bundle_siblings` + 读 display names）→ `remove_login_item`；NeedsPrivilege→skip 计数；成功 succeeded；**不** mole_delete
  - `parse_login_helper_rule_id` → 再检 has_siblings；非法/`com.apple.*` skip；PathVanished 仍允许 bootout；失败 soft-skip
- 保护 app 测试不回归

- [ ] **Step 1: RED**

```rust
#[test]
fn apply_login_item_calls_remove() { /* Fake */ }

#[test]
fn apply_login_helper_calls_bootout() { /* Fake */ }

#[test]
fn apply_login_item_needs_privilege_skips_loudly() {
    // Fake returns NeedsPrivilege；assert SkipReason::NeedsPrivilege；同 plan 普通 leftover/app 仍可 succeeded
}

#[test]
fn apply_skips_bootout_when_sibling_present() { /* ... */ }
```

- [ ] **Step 2–4: RED→GREEN→测**

- [ ] **Step 5: Commit** — `feat(uninstall): apply login item and helper actions`

---

### Task 5: 版本 / coverage / README / findings / release note

**Files:**
- `Cargo.toml` workspace version → **1.34.0**
- `Formula/vole.rb` version
- `crates/vole-core/src/ops/coverage.rs` — 诚实句加入 login items 已落地；仍写 LaunchDaemons/`/Library` 若适用
- `README.md` — 窄改诚实句
- `docs/findings/2026-07-v2-m1-uninstall.md` — ② ✅ **1.34.0**
- Create: `docs/releases/v1.34.0.md`

- [ ] **Step 1:** 改文案与版本
- [ ] **Step 2:** `cargo test -p vole-core -- uninstall login_items`；`cargo fmt`；相关 CLI 测若有则跑
- [ ] **Step 3: Commit** — `chore: release 1.34.0 uninstall login items`

---

### Task 6: PR + CI + merge

- [ ] Push `feat/uninstall-login-items`
- [ ] `gh pr create`（Summary + Test plan）
- [ ] 触发/等待 CI；需要时 security-review subagent
- [ ] CI 绿 → `gh pr merge --merge --delete-branch`
- [ ] **不在本 PR**改 0119 路线图（另开 follow-up）

---

## 验收清单

- [ ] 侧车 plan + Fake apply 单测绿
- [ ] sibling / 名冲突 / `com.apple.*` 守卫
- [ ] NeedsPrivilege 不阻塞整规则
- [ ] 1.34.0；无 W2a③ / clean / optimize 误改
- [ ] PR 已 merge-commit 合入
