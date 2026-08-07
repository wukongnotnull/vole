# Private Var DB Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地 `private-var-db-diagnostics`：`/private/var/db/diagnostics` 下深度 ≤5 文件经 `sudo -n` permanent 删除；非 `.tracev3` ≥7d，`.tracev3` ≥30d（1.16.0）。

**Architecture:** 形状谓词 + Privilege allow + walk candidates（分龄）+ apply 绑谓词/文件/分龄。

**Tech Stack:** Rust / vole-core / macOS `sudo -n`。

## Global Constraints

- 版本：**1.16.0**；规则 **521 → 522**；不 bump schema
- maxdepth **5**；分龄 7 / 30；不打 tag；PR security-review
- 不含 DiagnosticPipeline / powerlog

---

## File Structure

| 文件 | 职责 |
|---|---|
| `safety/critical.rs` | 谓词 + LIVE / MAX_DEPTH |
| `privilege/mod.rs` | allow + age helper + candidates + RULE_ID |
| `ops/plan.rs` / `apply_plan.rs` | 接线 |
| `user-devtools.toml` | 规则 |
| coverage / README / Cargo / Formula / releases / findings | 发版 |

**测试 remap**：`parent(VOLE_TEST_SYSTEM_LIBRARY)/private/var/db/diagnostics`

---

### Task 1: 形状谓词

- [ ] 单测：深度 1/5 true，6 / 根 / 根外 false
- [ ] 实现 + re-export
- [ ] Commit：`feat(safety): private-var-db-diagnostics clean-target predicate`

### Task 2: Privilege + candidates + 分龄

- [ ] `diagnostics_age_days`；allowlist；walk ≤5 + 分龄过滤
- [ ] Commit：`feat(privilege): allow private-var-db-diagnostics`

### Task 3: TOML + plan/apply

```toml
[[rule]]
id = "private-var-db-diagnostics"
category = "user-devtools"
label = "System diagnostics db logs"
platform = ["macos"]
paths = ["/private/var/db/diagnostics"]
impact = "Old /private/var/db/diagnostics files (≤5 deep; ≥7d, .tracev3 ≥30d); safe to rebuild"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "older_than_days"
days = 7
```

- [ ] plan candidates；apply 分龄；回归（新鲜 / 中间龄 tracev3 / 三树）
- [ ] Commit：`feat: wire private-var-db-diagnostics plan/apply`

### Task 4: Coverage / 1.16.0

- coverage / README 522；Cargo/Formula 1.16.0；releases + findings
- `cargo test -p vole-core --lib`
- Commit：`chore: release 1.16.0 private-var-db-diagnostics`

---

## Spec coverage

| Spec | Task |
|---|---|
| 谓词 | 1 |
| privilege + 分龄 | 2 |
| TOML + plan/apply | 3 |
| 1.16.0 | 4 |
| security-review | PR |
