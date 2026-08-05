# Apply Permission Loud Hint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:subagent-driven-development (recommended) or wukong-code:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** uninstall/optimize apply 在出现 `TccDenied`/`NeedsPrivilege` skip 时响亮提示；发版 **1.4.2**。

**Architecture:** core 提供检测 + 文案常量 + coverage 追加 helper；CLI 在 apply 收口分通道输出（json 改 note；human stderr 单打）。

**Design:** [`../specs/2026-08-05-2122-apply-permission-loud-hint-design.md`](../specs/2026-08-05-2122-apply-permission-loud-hint-design.md)

## Global Constraints

- 零 schema bump；不改 plan 期 coverage 英/混文案
- 文案固定见 design §3
- human 不与 json 双重埋同一 note（分通道）
- PATCH 1.4.2 在 Task 3

---

### Task 1: core helpers + 单测

**Files:** `crates/vole-core/src/ops/coverage.rs`, `ops/mod.rs`

- [ ] 增加 `APPLY_PERMISSION_WARN`、`report_has_permission_skips`、`coverage_with_apply_permission_hint`
- [ ] 单测：空 skip / 仅 Whitelisted → false；含 TccDenied → true；append 行为
- [ ] Commit: `feat(ops): apply permission loud-hint helpers`

### Task 2: CLI uninstall + optimize

**Files:** `crates/vole-cli/src/uninstall.rs`, `optimize.rs`

- [ ] apply 后 json：改写 `report.coverage_note`
- [ ] human：`print_human_report` 末尾条件 `eprintln!(APPLY_PERMISSION_WARN)`
- [ ] Commit: `feat(cli): loud permission hint on uninstall/optimize apply`

### Task 3: docs + bump 1.4.2

**Files:** `Cargo.toml` / lock、`docs/releases/v1.4.1.md` 模式写 `v1.4.2.md`、README 指针、findings、spec 状态

- [ ] Commit: `chore(release): prepare v1.4.2 apply permission hints`

---

按惯例默认 Inline Execution。
