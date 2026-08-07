# Icon Services System Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地 `icon-services-system-cache`：exact `/Library/Caches/com.apple.iconservices.store` 经 `sudo -n` permanent 删除（1.13.0）。

**Architecture:** exact 谓词 + Privilege allow + plan remapping + apply 特权分支（绑 exact）；仿 Rosetta，无 arm64 / 无 critical carve-out。

**Tech Stack:** Rust / vole-core / macOS `sudo -n`。

## Global Constraints

- 版本：**1.13.0**；规则 **518 → 519**；不 bump schema
- 放行面：**仅** exact iconservices store（live 或 `VOLE_TEST_SYSTEM_LIBRARY` 映射 `$BASE/Caches/com.apple.iconservices.store`）
- apply 必须 `is_icon_services_system_cache`（防 rule_id 篡改走三树）
- **禁止** `/Library/Caches/**` 泛扫；无交互 sudo / 桌面 SMAppService
- PR：security-review；默认不打 tag

---

## File Structure

| 文件 | 职责 |
|---|---|
| **Modify** `safety/critical.rs`（或同模块旁） | `is_icon_services_system_cache` + LIVE 常量 |
| **Modify** `safety/mod.rs` | re-export |
| **Modify** `privilege/mod.rs` | allow + plan candidates + RULE_ID |
| **Modify** `ops/plan.rs` / `ops/apply_plan.rs` | 接线 |
| **Modify** `user-devtools.toml` | 规则（紧邻 `icon-services-cache`） |
| coverage / README / Cargo / Formula / releases / findings | 发版 |

---

### Task 1: Exact 谓词

- [ ] 单测：exact / trailing `/` / 父目录 / 其它 Caches → 形状断言
- [ ] 实现 `is_icon_services_system_cache`（含 test remap）
- [ ] Commit：`feat(safety): exact predicate for icon services system cache`

### Task 2: Privilege + plan candidates

- [ ] `path_allowed_for_privilege` 接纳 exact
- [ ] `icon_services_system_plan_candidates()`（无 arch 门控）
- [ ] 单测：allow / 候选存在性；三树回归绿
- [ ] Commit：`feat(privilege): allow icon-services system cache path`

### Task 3: TOML + plan/apply

```toml
[[rule]]
id = "icon-services-system-cache"
category = "user-devtools"
label = "Icon services system cache"
platform = ["macos"]
paths = ["/Library/Caches/com.apple.iconservices.store"]
impact = "System icon cache; safe to rebuild"
disabled = false
last_verified = "2026-08"

[rule.strategy]
kind = "all"
```

- [ ] plan：`ICON_SERVICES_SYSTEM_CACHE_RULE_ID` → `icon_services_system_plan_candidates()`
- [ ] apply：exact 谓词 + allowlist + probe + permanent（**不** unload）；回归：三树 path + 本 rule_id → skip
- [ ] 规则数 519
- [ ] Commit：`feat: wire icon-services-system-cache plan/apply`

### Task 4: Coverage / 1.13.0

- coverage 注明该点已落地；仍未移植保留交互提权 / 桌面（可加「system.sh 其余」短句）
- README 519；Cargo/Formula 1.13.0；releases + findings
- `cargo test -p vole-core`
- Commit：`chore: release 1.13.0 icon-services-system-cache`

---

## Spec coverage

| Spec | Task |
|---|---|
| exact 谓词 | 1 |
| privilege | 2 |
| TOML + plan/apply | 3 |
| 1.13.0 | 4 |
| security-review | PR |
