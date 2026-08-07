# Claude Pending Uploads Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地 `claude-pending-uploads`：保护层形状豁免 + TOML 规则，清理 Claude Desktop pending-uploads 叶（1.11.0）。

**Architecture:** `is_claude_pending_uploads_path` 接入 `is_explicit_clean_cache_path`；`user-devtools.toml` 追加 `all` 规则；apply 无旁路。

**Tech Stack:** Rust / vole-core / TOML。

## Global Constraints

- 版本：**1.11.0**；规则 **516 → 517**；不 bump schema
- 形状写死：`…/Library/Application Support/Claude/pending-uploads/<leaf>`（单层叶）
- **禁止**：改 `protection.toml`、apply carve-out、sudo、打 tag（除非用户明确要求发版）
- PR：security-review

---

## File Structure

| 文件 | 职责 |
|---|---|
| **Modify** `crates/vole-core/src/protection/path.rs` | helper + explicit + 单测 |
| **Modify** `data/rules/user-devtools.toml` | 追加规则（紧邻其它 Claude Electron 规则） |
| **Modify** `coverage.rs` / README / Cargo.toml / Formula / releases / findings | 发版文案 |

---

### Task 1: 保护层形状豁免

**Files:** `crates/vole-core/src/protection/path.rs`

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn claude_pending_uploads_leaf_allowed() {
    let c = cat();
    let home = "/Users/t";
    assert!(!should_protect_path(
        &format!("{home}/Library/Application Support/Claude/pending-uploads/upload.bin"),
        &c,
        ProtectionMode::Cleanup
    ));
}

#[test]
fn claude_non_pending_uploads_still_protected() {
    let c = cat();
    let home = "/Users/t";
    assert!(should_protect_path(
        &format!("{home}/Library/Application Support/Claude/Local Storage/file"),
        &c,
        ProtectionMode::Cleanup
    ));
    assert!(should_protect_path(
        &format!("{home}/Library/Application Support/Claude/pending-uploads"),
        &c,
        ProtectionMode::Cleanup
    ));
}
```

- [ ] **Step 2:** `cargo test -p vole-core --lib claude_pending` — expect FAIL

- [ ] **Step 3: Implement**

```rust
fn is_claude_pending_uploads_path(path: &str) -> bool {
    const MARKER: &str = "/Library/Application Support/Claude/pending-uploads/";
    let Some(rest) = path.split(MARKER).nth(1) else {
        return false;
    };
    !rest.is_empty() && !rest.contains('/')
}
```

在 `is_explicit_clean_cache_path`：`if is_claude_pending_uploads_path(path) { return true; }`

- [ ] **Step 4:** tests PASS

- [ ] **Step 5: Commit** `feat(protection): allow Claude pending-uploads leaves`

---

### Task 2: TOML 规则

**Files:** `data/rules/user-devtools.toml`（`claude-sentry-cache` 附近）

```toml
[[rule]]
id = "claude-pending-uploads"
category = "user-devtools"
label = "Claude pending uploads"
platform = ["macos"]
paths = ["~/Library/Application Support/Claude/pending-uploads/*"]
impact = "Queued upload stubs; safe to rebuild"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "all"
```

- [ ] **Step 1:** 规则数 517（`rg -c '^id = ' data/rules/*.toml`）

- [ ] **Step 2: Commit** `feat(rules): add claude-pending-uploads`

---

### Task 3: Coverage / docs / 1.11.0 bump

- coverage 去掉 unported「claude pending-uploads」；注明已落地
- README 516→517；Cargo.toml / Formula → 1.11.0
- `docs/releases/v1.11.0.md`、findings
- `cargo test -p vole-core`
- Commit：`chore: release 1.11.0 claude-pending-uploads`（不打 tag）

---

## Spec coverage

| Spec | Task |
|---|---|
| 形状豁免 | 1 |
| TOML | 2 |
| coverage / 1.11.0 | 3 |
