# B4：Orphaned App Data Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `vole clean` plan→apply 上交付用户域 orphaned app data（Caches / Logs / Saved Application State），发版 **1.3.0**。

**Architecture:** 新建可注入的 `vole-core::orphan` 库（安装扫描 + judge + 过滤）；经 custom handler `orphaned_app_data` 接入既有 clean plan；apply 侧对 `rule_id == "orphaned-app-data"` **新增**完整 judge 重判钩子。零 `schema_version` 变更。默认废纸篓。

**Tech Stack:** Rust 1.97.1、`vole-core` / `vole-cli`、`mdfind`/`lsappinfo`/`mdutil` 经 `std::process::Command`（超时模式对齐 `scan/du.rs`）、Mole 钉版 `third_party/mole-1.48.1`。

**Design:** [`docs/wukong-code/specs/2026-08-05-1642-b4-orphaned-app-data-design.md`](../specs/2026-08-05-1642-b4-orphaned-app-data-design.md)

## Global Constraints

- 许可证：GPL-3.0-only。
- 平台：仅 macOS；**无 sudo / 真提权**。
- 扫描根仅三处：`$HOME/Library/{Caches,Logs,Saved Application State}`；前缀仅 `com.*`/`org.*`/`net.*`/`io.*`。
- **永不**扫 Containers / Group Containers / LaunchAgents / Application Scripts / `/Library/**`。
- env：`MOLE_ORPHAN_AGE_DAYS`（默认 30；解析失败或 <7 → 30）。
- mdfind：Spotlight 不可用 / 超时 / 非零退出 → fail-closed（视为仍安装）；每次 plan 调用上限 **64**。
- 安装扫描：进程内单次，**无磁盘缓存**；目录权限失败 → 跳过整个 orphan 判定。
- 迭代上限：每资源根 **100**（对齐 `MOLE_MAX_ORPHAN_ITERATIONS`）。
- 规则文件名 `zzz-orphaned.toml` 保证加载序最后；同路径去重「具名胜 orphaned」。
- fixture / CI：**注入假 deps**，禁止真 `/Applications`、禁止真 mdfind/lsappinfo。
- 每个 Task 至少一次 commit；合并 main 前 CI 绿。
- 本计划 **不含** B4.1 Claude VM、system services orphan、SwiftUI。

---

## File Structure

```
crates/vole-core/src/orphan/
  mod.rs              # NEW：模块导出、RULE_ID 常量、age clamp、label helper
  deps.rs             # NEW：OrphanDeps trait + LiveOrphanDeps + FakeOrphanDeps
  installed.rs        # NEW：扫描 .app / agents；合并运行中 bundle
  judge.rs            # NEW：is_bundle_orphaned
  select.rs           # NEW：从 PathEntry 过滤出 orphan 路径

crates/vole-core/src/lib.rs                          # + pub mod orphan
crates/vole-core/src/rules/custom_handlers.rs        # 注册 orphaned_app_data
crates/vole-core/src/ops/mod.rs                      # Orchestrator 持有 orphan_deps
crates/vole-core/src/ops/plan.rs                     # 传 deps；orphaned 用 per-path label
crates/vole-core/src/ops/apply_plan.rs               # apply 重判钩子
crates/vole-core/src/ops/coverage.rs                 # 文案：用户域 orphaned 已落地

data/rules/zzz-orphaned.toml                         # NEW：规则 orphaned-app-data
tests/fixtures/orphaned/                             # NEW：假 HOME 布局说明 / 若需 json
docs/findings/2026-08-b4-orphaned-security-review.md
docs/releases/v1.3.0.md
README.md
Cargo.toml / Formula/vole.rb                         # bump 1.3.0（发版 task）
```

---

### Task 1: Age clamp + 敏感族 + 系统 deny + bundle 抽取

**Files:**
- Create: `crates/vole-core/src/orphan/mod.rs`
- Create: `crates/vole-core/src/orphan/judge.rs`（先放纯函数；deps 在 Task 2）
- Modify: `crates/vole-core/src/lib.rs`（`pub mod orphan;`）

**Interfaces:**
- Produces:
  ```rust
  pub const ORPHANED_RULE_ID: &str = "orphaned-app-data";
  pub const DEFAULT_ORPHAN_AGE_DAYS: u32 = 30;
  pub const MIN_ORPHAN_AGE_DAYS: u32 = 7;
  pub const MAX_ORPHAN_ITERATIONS: usize = 100;
  pub const MAX_MDFIND_CALLS: usize = 64;

  pub fn orphan_age_days_from_env() -> u32; // 读 MOLE_ORPHAN_AGE_DAYS
  pub fn orphan_age_days_from_raw(raw: Option<&str>) -> u32; // 可测
  pub fn is_sensitive_orphan_bundle(bundle_id: &str) -> bool;
  pub fn is_system_component_bundle(bundle_id: &str) -> bool;
  pub fn bundle_id_from_orphan_path(path: &Path) -> Option<String>;
  pub fn matches_orphan_name_prefix(name: &str) -> bool; // com/org/net/io
  pub fn orphan_label(path: &Path) -> String; // "Orphaned Caches: com.foo"
  pub fn resource_kind_label(path: &Path) -> &'static str; // Caches|Logs|States
  ```

- [ ] **Step 1: Write failing tests** in `judge.rs` `#[cfg(test)]`

```rust
#[test]
fn age_clamp_rejects_zero_and_garbage() {
    assert_eq!(orphan_age_days_from_raw(None), 30);
    assert_eq!(orphan_age_days_from_raw(Some("0")), 30);
    assert_eq!(orphan_age_days_from_raw(Some("6")), 30);
    assert_eq!(orphan_age_days_from_raw(Some("7")), 7);
    assert_eq!(orphan_age_days_from_raw(Some("30")), 30);
    assert_eq!(orphan_age_days_from_raw(Some("nope")), 30);
    assert_eq!(orphan_age_days_from_raw(Some("-1")), 30);
}

#[test]
fn sensitive_and_system_denylists() {
    assert!(is_sensitive_orphan_bundle("com.1password.1password"));
    assert!(is_sensitive_orphan_bundle("com.apple.keychain"));
    assert!(is_sensitive_orphan_bundle("org.gpg.agent"));
    assert!(is_system_component_bundle("finder"));
    assert!(is_system_component_bundle("safari"));
    assert!(!is_sensitive_orphan_bundle("com.example.cache"));
}

#[test]
fn bundle_id_and_prefix_from_path() {
    let p = Path::new("/tmp/Library/Caches/com.example.app");
    assert_eq!(bundle_id_from_orphan_path(p).as_deref(), Some("com.example.app"));
    assert!(matches_orphan_name_prefix("com.example.app"));
    assert!(!matches_orphan_name_prefix("dev.orbstack.OrbStack"));
    let s = Path::new("/tmp/Library/Saved Application State/com.foo.savedState");
    assert_eq!(bundle_id_from_orphan_path(s).as_deref(), Some("com.foo"));
    assert_eq!(orphan_label(p), "Orphaned Caches: com.example.app");
}
```

- [ ] **Step 2: Run** `cargo test -p vole-core orphan_age --lib`  
  Expected: FAIL（模块不存在）

- [ ] **Step 3: Implement** 纯函数；敏感族 glob 对齐 Mole：
  `*1password*` / `*keychain*` / `*bitwarden*` / `*lastpass*` / `*keepass*` / `*dashlane*` / `*enpass*` / `*ssh*` / `*gpg*` / `*gnupg*` / `com.apple.keychain*`（大小写不敏感匹配用 `to_ascii_lowercase` + 简单 contains / 前缀）。系统 deny：`loginwindow|dock|systempreferences|systemsettings|settings|controlcenter|finder|safari`。

- [ ] **Step 4: Run tests** — Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/orphan crates/vole-core/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(orphan): add age clamp, denylists, and path helpers

EOF
)"
```

---

### Task 2: `OrphanDeps` + 安装集合（可注入）

**Files:**
- Create: `crates/vole-core/src/orphan/deps.rs`
- Create: `crates/vole-core/src/orphan/installed.rs`
- Modify: `crates/vole-core/src/orphan/mod.rs`

**Interfaces:**
- Produces:
  ```rust
  pub trait OrphanDeps: Send + Sync {
      fn spotlight_available(&self) -> bool;
      /// Ok(true)=found, Ok(false)=empty+available, Err=timeout/fail → caller fail-closed
      fn mdfind_bundle(&self, bundle_id: &str) -> Result<bool, ()>;
      fn scan_installed_bundle_ids(&self, home: &Path) -> Result<HashSet<String>, ()>;
  }

  pub struct LiveOrphanDeps { /* mdfind call counter interior */ }
  impl LiveOrphanDeps {
      pub fn new() -> Self;
      pub fn mdfind_calls(&self) -> usize;
  }

  pub struct FakeOrphanDeps {
      pub spotlight: bool,
      pub installed: HashSet<String>,
      pub mdfind: HashMap<String, Result<bool, ()>>,
  }

  /// 合并：目录扫描 + LaunchAgents basename（Live 实现内完成；Fake 直接给 installed 全集）
  pub fn default_app_scan_roots(home: &Path) -> Vec<PathBuf>;
  ```
- Live 扫描根：`/Applications`、`/System/Applications`、`$HOME/Applications`、`/opt/homebrew/Caskroom`、`/usr/local/Caskroom`、`$HOME/Library/Application Support/Setapp/Applications`；maxdepth 3 找 `*.app`，读 `Contents/Info.plist` CFBundleIdentifier（复用 `read_bundle_id`）。
- Live 另并：`~/Library/LaunchAgents` + `/Library/LaunchAgents` 的 `*.plist` basename（strip `.plist`）；运行中优先 `lsappinfo list` 抽 `CFBundleIdentifier`（超时 10s，失败则跳过该源，不整批失败）。
- Live mdfind：`mdfind "kMDItemCFBundleIdentifier == 'id'"`，超时 10s（`du.rs` 线程+轮询模式）；`spotlight_available`：`mdutil -s /` 输出含 `disabled` → false，命令失败 → false。
- mdfind 调用计数：进程内；达 `MAX_MDFIND_CALLS` 后 `mdfind_bundle` 返回 `Err(())`。

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn fake_deps_installed_set_used_as_is() {
    let mut installed = HashSet::new();
    installed.insert("com.keep.me".into());
    let deps = FakeOrphanDeps {
        spotlight: true,
        installed,
        mdfind: HashMap::new(),
    };
    assert!(deps.scan_installed_bundle_ids(Path::new("/tmp")).unwrap().contains("com.keep.me"));
}

#[test]
fn live_mdfind_counter_caps_at_64() {
    // 用 Fake 模拟：连续 65 次查询，第 65 次 Err——在 Live 包装测试里用可注入 counter 单元测
    // 见 select 层测试 Task 3：mdfind_calls_exhausted_means_not_orphan
}
```

实现 Fake 先绿；Live 的 mdfind 上限在 `LiveOrphanDeps::mdfind_bundle` 内用 `AtomicUsize` 测：

```rust
#[test]
fn live_mdfind_budget_returns_err_after_cap() {
    let deps = LiveOrphanDeps::new_for_test_with_mdfind(|_| Ok(false));
    for _ in 0..MAX_MDFIND_CALLS {
        assert!(deps.mdfind_bundle("com.x.y").is_ok());
    }
    assert!(deps.mdfind_bundle("com.x.z").is_err());
}
```

（若不便 mock Command，把计数逻辑抽成 `MdfindBudget` 纯结构单测。）

- [ ] **Step 2–4:** RED → 实现 → GREEN

- [ ] **Step 5: Commit** `feat(orphan): injectable OrphanDeps and installed-set scan`

---

### Task 3: `is_bundle_orphaned` + `select_orphaned_paths`

**Files:**
- Modify: `crates/vole-core/src/orphan/judge.rs`
- Create: `crates/vole-core/src/orphan/select.rs`
- Modify: `crates/vole-core/src/orphan/mod.rs`

**Interfaces:**
- Consumes: Task 1 helpers、`OrphanDeps`、`ProtectionCatalog` / `should_protect_data`、`is_reverse_dns_bundle_id`
- Produces:
  ```rust
  pub struct OrphanJudge<'a> {
      pub catalog: &'a ProtectionCatalog,
      pub deps: &'a dyn OrphanDeps,
      pub installed: &'a HashSet<String>,
      pub age_days: u32,
      pub now: SystemTime,
  }

  impl OrphanJudge<'_> {
      /// true = 可标为 orphan（可删）
      pub fn is_bundle_orphaned(&self, bundle_id: &str, path: &Path) -> bool;
  }

  pub fn select_orphaned_paths(
      entries: &[PathEntry],
      home: &Path,
      catalog: &ProtectionCatalog,
      deps: &dyn OrphanDeps,
      age_days: u32,
      now: SystemTime,
  ) -> Result<Vec<PathBuf>, OrphanScanError>;

  pub enum OrphanScanError {
      LibraryInaccessible, // 读 home/Library/Caches 失败
  }
  ```

判定顺序（任一步「否」→ return false）：
1. `should_protect_data` → false  
2. `is_sensitive_orphan_bundle` → false  
3. `installed.contains` → false  
4. `is_system_component_bundle` → false  
5. mtime 距今 < age_days → false（用 `PathEntry.mtime` 或 `fs::metadata`）  
6. 若 reverse-DNS：`!spotlight_available` → false；`mdfind` Err → false；`Ok(true)` → false；`Ok(false)` → 继续  
7. true

`select_orphaned_paths`：
- 先 `fs::read_dir(home.join("Library/Caches"))` 失败 → `Err(LibraryInaccessible)`  
- `deps.scan_installed_bundle_ids` 失败 → 同 Err（不把空集当零安装）  
- 对 entries：跳过非三根前缀路径；`matches_orphan_name_prefix`；零尺寸（metadata len==0 且空目录用 `du`/简易：目录无子项或 size 0）跳过；每根迭代计数 >100 break；过 judge 的 push

- [ ] **Step 1: Write failing tests**（全 FakeDeps）

```rust
#[test]
fn orphan_when_old_and_not_installed() {
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join("Library/Caches/com.gone.app");
    fs::create_dir_all(&cache).unwrap();
    let old = SystemTime::now() - Duration::from_secs(40 * 86400);
    filetime_or_manual_mtime(&cache, old); // 若无 filetime crate：在测试里用 PathEntry 直接喂 mtime，judge 优先用传入 mtime

    let deps = FakeOrphanDeps {
        spotlight: true,
        installed: HashSet::new(),
        mdfind: HashMap::from([("com.gone.app".into(), Ok(false))]),
    };
    let catalog = ProtectionCatalog::embedded();
    let installed = deps.scan_installed_bundle_ids(home.path()).unwrap();
    let judge = OrphanJudge { catalog: &catalog, deps: &deps, installed: &installed, age_days: 30, now: SystemTime::now() };
    assert!(judge.is_bundle_orphaned("com.gone.app", &cache));
}

#[test]
fn not_orphan_when_spotlight_disabled() {
    // spotlight: false → 即使 mdfind 会 Ok(false) 也不标 orphan
}

#[test]
fn not_orphan_when_mdfind_errors() {
    // mdfind: Err(()) → false
}

#[test]
fn not_orphan_when_installed_or_fresh_mtime() { /* ... */ }

#[test]
fn select_skips_dev_prefix_and_respects_iteration_cap() { /* ... */ }
```

mtime：优先让 `is_bundle_orphaned` 接受 `mtime: SystemTime` 参数（从 `PathEntry` 传入），避免测试依赖 `filetime` crate。

- [ ] **Step 2–4:** RED → 实现 → GREEN

- [ ] **Step 5: Commit** `feat(orphan): judge pipeline and path selection with fail-closed mdfind`

---

### Task 4: Custom handler + TOML 规则（加载序最后）

**Files:**
- Modify: `crates/vole-core/src/rules/custom_handlers.rs`
- Create: `data/rules/zzz-orphaned.toml`
- Modify: `crates/vole-core/src/rules/custom_handlers.rs` 的 `select_custom` 签名——增加 `orphan_deps: &dyn OrphanDeps`（其它 handler 忽略）
- Modify: 所有 `select_custom(` 调用点（`ops/plan.rs` 等）

**Interfaces:**
- TOML：
  ```toml
  [[rule]]
  id = "orphaned-app-data"
  category = "orphaned"
  label = "Orphaned app data"
  platform = ["macos"]
  paths = [
    "~/Library/Caches/*",
    "~/Library/Logs/*",
    "~/Library/Saved Application State/*",
  ]
  impact = "卸载后残留且 ≥30 天未改动的用户域缓存/日志/Saved State（启发式）"
  disabled = false
  last_verified = "2026-08"

  [rule.strategy]
  kind = "custom"
  handler = "orphaned_app_data"
  ```
- `select_custom` 分支：
  ```rust
  "orphaned_app_data" => {
      match select_orphaned_paths(entries, home, &ProtectionCatalog::embedded(), orphan_deps, orphan_age_days_from_env(), SystemTime::now()) {
          Ok(paths) => paths,
          Err(_) => Vec::new(), // FDA/不可访问：plan 无候选；coverage/日志另任务
      }
  }
  ```

- [ ] **Step 1: Write failing test** — 加载规则目录后 orphaned 规则存在且为**最后一条** enabled：

```rust
#[test]
fn orphaned_rule_loads_last_among_enabled() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/rules");
    let rules = load_rules_from_dir(&dir).unwrap();
    let enabled: Vec<_> = rules.iter().filter(|r| !r.disabled).collect();
    let last = enabled.last().expect("rules");
    assert_eq!(last.id, "orphaned-app-data");
    assert_eq!(last.strategy.handler.as_deref(), Some("orphaned_app_data"));
}
```

- [ ] **Step 2: Write failing test** — handler 过滤（Fake deps 经 `select_custom` 传入）

- [ ] **Step 3–5:** 实现 TOML + 接线 → GREEN → Commit  
  `feat(clean): wire orphaned-app-data custom rule (last in load order)`

---

### Task 5: Orchestrator 注入 deps + plan per-path label + 去重顺序单测

**Files:**
- Modify: `crates/vole-core/src/ops/mod.rs`（`Orchestrator` 加 `orphan_deps: Arc<dyn OrphanDeps>`；构造器默认 `LiveOrphanDeps`；`with_orphan_deps`）
- Modify: `crates/vole-core/src/ops/plan.rs`（调用 `select_custom(..., self.orphan_deps.as_ref())`；`label`：若 `rule.id == ORPHANED_RULE_ID` 用 `orphan_label(&path)`）
- Test: `plan.rs` 或 `orphan` 集成测

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn plan_orphaned_uses_per_path_label_and_loses_dedup_to_named_rule() {
    let _guard = test_env::lock();
    let home = scratch("orphan-plan");
    // 1) 建旧 cache com.example.app
    // 2) rules: 先 specific-cache（paths 精确该目录），再 orphaned-app-data
    // 3) FakeOrphanDeps: 未安装 + spotlight + mdfind Ok(false)
    // 4) build plan → 仅一条，rule_id == specific-cache（具名胜）
}

#[test]
fn plan_orphaned_selects_when_only_orphaned_matches() {
    // 无具名规则；假 deps；期望 rule_id orphaned-app-data，label 含 "Orphaned Caches:"
}
```

- [ ] **Step 2–4:** 实现 → GREEN

- [ ] **Step 5: Commit** `feat(clean): inject OrphanDeps into plan; per-path orphan labels`

---

### Task 6: Apply 重判钩子

**Files:**
- Modify: `crates/vole-core/src/ops/apply_plan.rs`
- Modify: `ApplyPlanContext` / `ApplyPlanOptions` 或 `apply_proto_plan` 签名以传入 `&dyn OrphanDeps`（默认 Live；测试 Fake）

**Interfaces:**
- 在 guards 检查之后、`verify_plan_entry_for_apply` 之前（或之后、删除之前）：
  ```rust
  if entry.rule_id == ORPHANED_RULE_ID {
      if !recheck_orphaned_entry(entry, ctx) {
          // skip + SkipReason::PathVanished 或新增语义：复用 PathVanished / 不 bump proto——用现有 SkipReason::PathVanished 表示「重判未通过」并在 oplog 留 rule_id；若需更清晰可仅人读 log，不改 enum
          continue;
      }
  }
  ```
- `recheck_orphaned_entry`：重建 installed 集合；用入口 path 的当前 mtime；跑完整 `is_bundle_orphaned`；白名单已在 `mole_delete_verified` 内。

- [ ] **Step 1: Failing test**

```rust
#[test]
fn apply_skips_orphaned_when_rejudge_fails_after_plan() {
    // plan 含 orphaned 条目；FakeDeps 在 apply 时把该 bundle 标为 installed
    // → skipped >= 1，路径仍存在
}
```

- [ ] **Step 2–4:** 实现 → GREEN

- [ ] **Step 5: Commit** `feat(clean): re-judge orphaned-app-data entries at apply`

---

### Task 7: coverage_note + FDA 降级提示

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs`
- Modify: `crates/vole-core/src/ops/plan.rs` 或 clean CLI：当 orphan select 返回 `LibraryInaccessible` 时，在 `coverage_note` 或 stderr/stream 提示（对齐 Mole「No permission」）。最小：coverage 追加一句；或 `StreamEvent::Skipped { rule_id: orphaned, reason: TccDenied }`。

**文案目标：**
- 标明用户域 orphaned **已落地**
- 仍未移植：system services / Containers / Claude VM orphan / sudo 路径
- **禁止**再写「orphaned apps … 仍未移植」笼统句

- [ ] **Step 1:** 改测试 `coverage_note_mentions_mole_and_count`：断言含「orphaned」且含「已落地」或等价；断言**不含**「orphaned apps、sudo…仍未移植」旧句式

- [ ] **Step 2–4:** 改文案 → GREEN → Commit  
  `docs(clean): update coverage_note for shipped user-domain orphaned`

---

### Task 8: Fixture / CLI 回归 + 安全 findings

**Files:**
- Create: `docs/findings/2026-08-b4-orphaned-security-review.md`（勾选设计 §7）
- Modify: `crates/vole-core/src/clean_fixture.rs` 或新增 orphan fixture 测试（若现有 verify 会加载全量规则：确保 Fake/注入路径不触网）
- 确认 CI `cargo test --workspace` 与 `conformance-plan-only`：**LiveOrphanDeps 在无 FDA 的 runner 上** select 返回空或 LibraryInaccessible，不得 panic；不得依赖真实 orphan 命中数

- [ ] **Step 1:** 写 security-review findings，对照设计 §7 逐条勾选（实现后未勾的标「待 Task 9 发版前」）

- [ ] **Step 2:** 跑  
  `cargo test -p vole-core --lib`  
  `cargo test -p vole-cli --test '*'`（若有）  
  Expected: PASS

- [ ] **Step 3: Commit** `test(orphan): security review findings and CI-safe coverage`

---

### Task 9: 文档 + 版本 bump 1.3.0（发版准备）

**Files:**
- Create: `docs/releases/v1.3.0.md`
- Modify: `README.md`（特性 / Mole 对比：用户域 orphaned 已支持；诚实写清未做）
- Modify: `Cargo.toml` workspace `version = "1.3.0"`
- Modify: `docs/findings/2026-07-v1-closeout.md` B4 → ✅（指向本 findings）
- Modify: design 状态 → 已实现（发版后）
- **不**在本 task 打 tag / 改 Formula sha（tag 由 release 流水线；Formula 在资产就绪后另 commit）

- [ ] **Step 1:** 写 release notes（亮点：orphaned-app-data；安全闸口；非目标列表）

- [ ] **Step 2:** README 对齐

- [ ] **Step 3:** bump version

- [ ] **Step 4:** `cargo test -p vole-core coverage_note --lib` + `cargo fmt` + 相关测

- [ ] **Step 5: Commit** `chore(release): prepare v1.3.0 orphaned app data`

发版（人工/后续）：`git tag v1.3.0 && git push origin v1.3.0` → 等 Release → `scripts/update-homebrew-formula.sh`。

---

## Spec coverage（自审）

| 设计节 | Task |
|---|---|
| §4.3 扫描根 / NEVER | T3/T4 + findings |
| §5 判定 1–8 | T1–T3、T6 |
| §5.1 预算 / 无磁盘缓存 | T2 |
| §6.1 plan 接线 / 顺序 | T4–T5 |
| §6.2 apply 重判 | T6 |
| §6.3 可注入 | T2/T5 |
| §7 清单 | T8 |
| coverage / README / 1.3.0 | T7/T9 |
| B4.1 Claude VM | **不做**（约束） |

## Placeholder scan

无 TBD；标签方案写死为 plan.rs 对 `ORPHANED_RULE_ID` 调 `orphan_label`；SkipReason 不 bump proto。
