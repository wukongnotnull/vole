# Private Var Log Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地 `private-var-log`：`/private/var/log` 下深度 ≤5、`.log`/`.gz`/`.asl`、≥7 天文件经 `sudo -n` permanent 删除（1.15.0）。

**Architecture:** 形状谓词 + Privilege allow + walk candidates + older_than + apply 绑谓词/文件/年龄。

**Tech Stack:** Rust / vole-core / macOS `sudo -n`。

## Global Constraints

- 版本：**1.15.0**；规则 **520 → 521**；不 bump schema
- maxdepth **5**（对齐 Mole `safe_sudo_find_delete`）；扩展名 `.log|.gz|.asl`；年龄 **7**
- apply 必须绑 `is_private_var_log_clean_target`；plan 不 sudo；不打 tag；PR security-review

---

## File Structure

| 文件 | 职责 |
|---|---|
| `safety/critical.rs` | 谓词 + 常量 |
| `privilege/mod.rs` | allow + walk candidates + RULE_ID |
| `ops/plan.rs` / `apply_plan.rs` | 接线 |
| `user-devtools.toml` | 规则 |
| coverage / README / Cargo / Formula / releases / findings | 发版 |

**测试 remap 根**：`Path::new(VOLE_TEST_SYSTEM_LIBRARY).parent()/private/var/log`

---

### Task 1: 形状谓词

- [ ] 单测：深度 1/5 true，6 false；扩展名；根外
- [ ] 实现 + re-export
- [ ] Commit：`feat(safety): private var log clean-target predicate`

### Task 2: Privilege + candidates

- [ ] allowlist + `private_var_log_plan_candidates()`（walk ≤5，文件+扩展名）
- [ ] Commit：`feat(privilege): allow private-var-log targets`

### Task 3: TOML + plan/apply

```toml
[[rule]]
id = "private-var-log"
category = "user-devtools"
label = "System private var logs"
platform = ["macos"]
paths = ["/private/var/log"]
impact = "Old system logs (.log/.gz/.asl, ≤5 deep, ≥7d); safe to rebuild"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "older_than_days"
days = 7
```

- [ ] plan：candidates → OlderThanDays
- [ ] apply：谓词 + file + age + allowlist + sudo permanent
- [ ] 回归：新鲜 skip；三树 + 本 rule_id skip
- [ ] 规则 521
- [ ] Commit：`feat: wire private-var-log plan/apply`

### Task 4: Coverage / 1.15.0

- coverage / README 521；Cargo/Formula 1.15.0；releases + findings
- `cargo test -p vole-core`
- Commit：`chore: release 1.15.0 private-var-log`

---

## Spec coverage

| Spec | Task |
|---|---|
| 谓词 | 1 |
| privilege | 2 |
| TOML + plan/apply | 3 |
| 1.15.0 | 4 |
| security-review | PR |
