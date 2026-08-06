# Protected Group Container Logs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在保护层放行 Group Containers 下可再生 Logs 叶，使 `group-container-caches` 能清理受保护容器 Logs 与 bundle 命名日志文件（1.9.0）。

**Architecture:** 在 `protection/path.rs` 新增 `is_group_container_logs_path`；步骤 3 命中时设 `container_cache = true`（跳过 data_protected 早退）；步骤 6/7 经 `is_explicit_clean_cache_path` 放行顶层 `/Logs/`。不动 handler / TOML / apply 旁路。Caches/tmp 对 data_protected 仍拦。

**Tech Stack:** Rust / macOS / vole-core / 既有 protection 单测。

## Global Constraints

- 版本意图：**1.9.0**（SemVer MINOR）；规则数 **516 不变**；**不 bump** `schema_version`
- **禁止**修改：`groupcaches` handler 的 protected 语义、`data/rules/**`、`apply_plan.rs` rule_id 旁路、`skip_protection`、`protection.toml`、`is_container_cache_or_tmp` Data 沙盒语义
- 放行形状写死：`$HOME/Library/Group Containers/<id>/{Logs,Library/Logs}/<leaf>`（相对容器根 depth 1 的 `Logs`，或 depth 2 的 `Library/Logs`）
- **仍拦**：同容器 Caches/tmp；`…/<id>/Other/Logs/…`；Notes（步骤 1）；OrbStack（顶部）；非 Group Containers
- PR：**security-review 必过**

---

## File Structure

| 文件 | 职责 |
|---|---|
| **Modify** `crates/vole-core/src/protection/path.rs` | `is_group_container_logs_path` + 步骤 3/explicit 接入 + 单测 |
| **Modify** `crates/vole-core/src/ops/plan.rs` | 翻转 `plan_protected_macpaw_logs_*`：改为入选并可 assert 路径 |
| **Modify** `crates/vole-core/src/ops/coverage.rs` | coverage 文案去掉「受保护容器的组容器缓存」未移植；单测同步 |
| **Modify** `README.md`、`Cargo.toml`、`docs/releases/v1.9.0.md`、`docs/findings/2026-08-protected-group-container-logs.md`、`Formula/vole.rb` | 发版 |

**不做：** handler / TOML / apply carve-out。

---

### Task 1: `is_group_container_logs_path` + 接入保护层

**Files:**
- Modify: `crates/vole-core/src/protection/path.rs`

**Interfaces:**
- Produces: `fn is_group_container_logs_path(path: &str) -> bool`（`path` 模块私有即可）
- Consumes: 现有 `should_protect_path` / `is_explicit_clean_cache_path` 结构

- [ ] **Step 1: Write the failing tests**（追加到 `path.rs` 的 `#[cfg(test)] mod tests`）

```rust
#[test]
fn group_container_logs_allows_data_protected_leaves() {
    let c = cat();
    let home = "/Users/t";
    assert!(!should_protect_path(
        &format!("{home}/Library/Group Containers/com.macpaw.CleanMyMac/Logs/x.log"),
        &c,
        ProtectionMode::Cleanup
    ));
    assert!(!should_protect_path(
        &format!("{home}/Library/Group Containers/com.macpaw.CleanMyMac/Library/Logs/x.log"),
        &c,
        ProtectionMode::Cleanup
    ));
}

#[test]
fn group_container_logs_still_protects_caches_tmp_for_data_protected() {
    let c = cat();
    let home = "/Users/t";
    let base = format!("{home}/Library/Group Containers/com.macpaw.CleanMyMac");
    for rel in ["Caches/x", "Library/Caches/x", "tmp/x", "Library/tmp/x"] {
        assert!(
            should_protect_path(&format!("{base}/{rel}"), &c, ProtectionMode::Cleanup),
            "must still protect {rel}"
        );
    }
}

#[test]
fn group_container_logs_allows_bundle_named_leaf() {
    let c = cat();
    let home = "/Users/t";
    assert!(!should_protect_path(
        &format!(
            "{home}/Library/Group Containers/group.com.docker.docker/Logs/com.docker.helper.log"
        ),
        &c,
        ProtectionMode::Cleanup
    ));
}

#[test]
fn group_container_logs_notes_and_illegal_depth_stay_protected() {
    let c = cat();
    let home = "/Users/t";
    assert!(should_protect_path(
        &format!("{home}/Library/Group Containers/group.com.apple.notes/Logs/x.log"),
        &c,
        ProtectionMode::Cleanup
    ));
    assert!(should_protect_path(
        &format!("{home}/Library/Group Containers/com.macpaw.CleanMyMac/Other/Logs/x.log"),
        &c,
        ProtectionMode::Cleanup
    ));
}

#[test]
fn group_container_logs_does_not_weaken_non_group_container_paths() {
    let c = cat();
    let home = "/Users/t";
    // Application Support 等非 Group Containers 路径行为保持保护
    assert!(should_protect_path(
        &format!("{home}/Library/Application Support/com.macpaw.CleanMyMac/data"),
        &c,
        ProtectionMode::Cleanup
    ));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p vole-core --lib protection::path::tests::group_container_logs_allows_data_protected_leaves \
  protection::path::tests::group_container_logs_allows_bundle_named_leaf
```

Expected: FAIL（macpaw Logs / docker bundle 命名叶仍被保护）

- [ ] **Step 3: Implement helper + wire**

在 `is_container_cache_or_tmp` 之后加入：

```rust
/// Group Containers 下可再生 Logs 路径（1.9.0 Cleanup 形状豁免）。
/// 仅相对容器根的 `Logs/<leaf>` 或 `Library/Logs/<leaf>`。
fn is_group_container_logs_path(path: &str) -> bool {
    const MARKER: &str = "/Library/Group Containers/";
    let Some(rest) = path.split(MARKER).nth(1) else {
        return false;
    };
    let mut parts = rest.split('/');
    let Some(id) = parts.next() else {
        return false;
    };
    if id.is_empty() {
        return false;
    }
    match (parts.next(), parts.next()) {
        (Some("Logs"), Some(leaf)) if !leaf.is_empty() => true,
        (Some("Library"), Some("Logs")) => matches!(parts.next(), Some(leaf) if !leaf.is_empty()),
        _ => false,
    }
}
```

**步骤 3** 改为：

```rust
let mut container_cache = false;
if let Some(bundle_id) = extract_container_bundle_id(path) {
    if is_container_cache_or_tmp(path) || is_group_container_logs_path(path) {
        container_cache = true;
    } else if mode == ProtectionMode::Cleanup && should_protect_data(&bundle_id, catalog) {
        return true;
    }
}
```

**`is_explicit_clean_cache_path`** 在 `/Library/Logs/` 分支之后加入：

```rust
if is_group_container_logs_path(path) {
    return true;
}
```

（`container_cache=true` 已使步骤 7 短路；explicit 扩展是纵深，覆盖 extract 失败等边角。）

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p vole-core --lib protection::path::tests
```

Expected: PASS（含新测与既有测）

另跑：

```bash
cargo test -p vole-core --lib safety
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/protection/path.rs
git commit -m "$(cat <<'EOF'
feat(protection): allow Group Containers Logs leaves for Cleanup

Unblock data_protected container Logs and bundle-named log files under
Group Containers via shape allowlist; keep Caches/tmp blocked.

EOF
)"
```

---

### Task 2: Plan 集成断言翻转（macpaw Logs 入选）

**Files:**
- Modify: `crates/vole-core/src/ops/plan.rs`（测试 `plan_protected_macpaw_logs_skipped_by_protection_gate`）

**Interfaces:**
- Consumes: Task 1 的保护层行为
- Produces: 集成测证明 plan 入选

- [ ] **Step 1: Rewrite the expectation test**

将 `plan_protected_macpaw_logs_skipped_by_protection_gate` **改名为** `plan_protected_macpaw_logs_enter_plan`，内容改为：

```rust
#[test]
fn plan_protected_macpaw_logs_enter_plan() {
    let _guard = test_env::lock();
    let home = scratch("gcc-prot-enter");
    let logs = home.join("Library/Group Containers/com.macpaw.CleanMyMac/Logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(logs.join("x.log"), b"x").unwrap();
    // Caches 叶：handler 因 protected 不提；即便存在也不得入 plan
    let caches = home.join("Library/Group Containers/com.macpaw.CleanMyMac/Library/Caches");
    fs::create_dir_all(&caches).unwrap();
    fs::write(caches.join("y"), b"y").unwrap();
    std::env::set_var("HOME", &home);

    let (tx, _rx) = unbounded();
    let orch = Orchestrator::new(crate::cancel::CancelToken::new(), Some(tx));
    let plan = orch
        .build_plan(&[group_container_cache_rule()], &AppProtection::new(), &[])
        .unwrap();

    assert_eq!(plan.entries.len(), 1);
    assert!(plan.entries[0].path.ends_with("Logs/x.log"));
    assert_eq!(plan.entries[0].rule_id, "group-container-caches");
    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}
```

可选追加（同文件）：

```rust
#[test]
fn plan_bundle_named_group_container_log_enters() {
    let _guard = test_env::lock();
    let home = scratch("gcc-bundle-log");
    let logs = home.join("Library/Group Containers/group.com.docker.docker/Logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(logs.join("com.docker.helper.log"), b"x").unwrap();
    std::env::set_var("HOME", &home);

    let (tx, _rx) = unbounded();
    let orch = Orchestrator::new(crate::cancel::CancelToken::new(), Some(tx));
    let plan = orch
        .build_plan(&[group_container_cache_rule()], &AppProtection::new(), &[])
        .unwrap();

    assert_eq!(plan.entries.len(), 1);
    assert!(plan.entries[0]
        .path
        .ends_with("Logs/com.docker.helper.log"));
    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&home);
}
```

- [ ] **Step 2: Run tests**

Run:

```bash
cargo test -p vole-core --lib ops::plan::tests::plan_protected_macpaw_logs_enter_plan \
  ops::plan::tests::plan_bundle_named_group_container_log_enters
```

Expected: PASS（Task 1 已落地后）

- [ ] **Step 3: Commit**

```bash
git add crates/vole-core/src/ops/plan.rs
git commit -m "$(cat <<'EOF'
test(plan): expect protected Group Containers Logs in plan

Flip macpaw Logs expectation from ProtectedPath skip to selected entry;
add bundle-named log fixture.

EOF
)"
```

---

### Task 3: Coverage / README / 发版 1.9.0

**Files:**
- Modify: `crates/vole-core/src/ops/coverage.rs`
- Modify: `README.md`
- Modify: `Cargo.toml`（workspace / 根 `version = "1.9.0"`）
- Create: `docs/releases/v1.9.0.md`
- Create: `docs/findings/2026-08-protected-group-container-logs.md`
- Modify: `Formula/vole.rb`（version → 1.9.0；sha256 暂留占位或沿用发版流水线后填，与 1.7.0/1.8.0 惯例一致）

**Interfaces:**
- Produces: 用户可见 coverage / 发版文档

- [ ] **Step 1: Update coverage_note + tests**

`coverage_note` 中：

- `Group Containers logs/caches（Mole 同形，受保护容器与 bundle 命名文件除外）` → `Group Containers logs/caches（含受保护容器 Logs / bundle 命名日志）`
- `仍未移植：真 sudo 删除、受保护容器的组容器缓存、其它 sudo/系统路径（如 Rosetta `/Library`、claude pending-uploads）。` → `仍未移植：真 sudo 删除、其它 sudo/系统路径（如 Rosetta `/Library`、claude pending-uploads）。`

单测（现有 `unported.contains("受保护容器的组容器缓存")`）改为：

```rust
assert!(
    !unported.contains("受保护容器的组容器缓存"),
    "protected group container caches must not remain unported"
);
assert!(
    !note.contains("受保护容器与 bundle 命名文件除外"),
    "partial Group Containers caveat must be removed"
);
```

- [ ] **Step 2: Run coverage tests**

```bash
cargo test -p vole-core --lib ops::coverage
```

Expected: PASS

- [ ] **Step 3: README + version + release docs**

- `README.md`：把「受保护组容器缓存完整对齐」从「请继续用 Mole」要点中去掉或改为已落地；规则数保持 **516**；如有版本徽章依赖 tag 则无需改数字行（现为动态 badge）
- `Cargo.toml`：`version = "1.9.0"`（及成员 crate 若继承则只改 workspace）
- `docs/releases/v1.9.0.md`：对齐 `v1.8.0.md` 结构；亮点写保护层 Logs 形状豁免；非目标写 Caches/tmp / 真 sudo
- `docs/findings/2026-08-protected-group-container-logs.md`：贴 design §4.2 矩阵 + 实现落点
- `Formula/vole.rb`：`version "1.9.0"`；url 换 v1.9.0；sha256 若尚无资产则写注释/沿用流水线后补（**禁止**瞎编 hash）

- [ ] **Step 4: Full vole-core test（macOS）**

```bash
cargo test -p vole-core
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vole-core/src/ops/coverage.rs README.md Cargo.toml \
  docs/releases/v1.9.0.md docs/findings/2026-08-protected-group-container-logs.md \
  Formula/vole.rb
git commit -m "$(cat <<'EOF'
chore: release 1.9.0 protected Group Containers Logs

Document protection carve-out coverage and bump package/Formula version.

EOF
)"
```

---

## Spec coverage checklist（self-review）

| Spec 要求 | Task |
|---|---|
| `is_group_container_logs_path` + 步骤 3 / explicit | Task 1 |
| macpaw Logs 放行；Caches/tmp 仍拦 | Task 1 单测 + Task 2 |
| bundle 命名 Logs 放行 | Task 1 + Task 2 |
| Notes / Other/Logs / 非 GC 不退化 | Task 1 单测 |
| coverage / README / 1.9.0 | Task 3 |
| 不动 handler / TOML / apply 旁路 | Global Constraints |
| security-review | 开 PR 时执行（不在本 plan 代码 task）|
