# Diagnostic Reports System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地 `diagnostic-reports-system`：`/Library/Logs/DiagnosticReports` 下 ≥7 天单层叶经 `sudo -n` permanent 删除（1.14.0）。

**Architecture:** 形状谓词 + Privilege 叶 allow + plan remapping + `older_than_days` + apply 绑形状与年龄重验。

**Tech Stack:** Rust / vole-core / macOS `sudo -n`。

## Global Constraints

- 版本：**1.14.0**；规则 **519 → 520**；不 bump schema
- 仅 `/Library/Logs/DiagnosticReports/<leaf>`（文件）；年龄 **7** 天
- apply 必须形状谓词 + 年龄重验（防 rule_id / 新鲜文件篡改）
- plan 不 sudo；默认不打 tag；PR security-review

---

## File Structure

| 文件 | 职责 |
|---|---|
| `safety/critical.rs`（或旁） | `is_system_diagnostic_report_leaf` |
| `privilege/mod.rs` | allow + plan candidates + RULE_ID + AGE |
| `ops/plan.rs` / `apply_plan.rs` | 接线 |
| `app-caches.toml` | 规则（紧邻 `diagnostic-reports`） |
| coverage / README / Cargo / Formula / releases / findings | 发版 |

---

### Task 1: 形状谓词

- [ ] 单测：叶 / 目录 / 嵌套 / 其它路径；test remap
- [ ] 实现 + re-export
- [ ] Commit：`feat(safety): leaf predicate for system DiagnosticReports`

### Task 2: Privilege + plan candidates

- [ ] `path_allowed_for_privilege` 接纳叶
- [ ] `diagnostic_reports_system_plan_candidates()`：列 remapped/live 根下 **文件** 叶（存在可读）
- [ ] Commit：`feat(privilege): allow system DiagnosticReports leaves`

### Task 3: TOML + plan/apply

```toml
[[rule]]
id = "diagnostic-reports-system"
category = "app-caches"
label = "Diagnostic reports (system)"
platform = ["macos"]
paths = ["/Library/Logs/DiagnosticReports/*"]
impact = "System crash reports older than 7 days; safe to rebuild"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "older_than_days"
days = 7
```

- [ ] plan：本 rule → candidates 再 `OlderThanDays(7).select`
- [ ] apply：形状 + allowlist + mtime≥7d 重验 + probe + permanent
- [ ] 回归：新鲜叶 skip；三树 path + 本 rule_id skip
- [ ] 规则数 520
- [ ] Commit：`feat: wire diagnostic-reports-system plan/apply`

### Task 4: Coverage / 1.14.0

- coverage / README 520；Cargo/Formula 1.14.0；releases + findings
- `cargo test -p vole-core`
- Commit：`chore: release 1.14.0 diagnostic-reports-system`

---

## Spec coverage

| Spec | Task |
|---|---|
| 形状 | 1 |
| privilege | 2 |
| TOML + plan/apply + 年龄 | 3 |
| 1.14.0 | 4 |
| security-review | PR |
