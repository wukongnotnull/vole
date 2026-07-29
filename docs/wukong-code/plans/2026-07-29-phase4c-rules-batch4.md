# Phase 4c Batch 4：Clean 规则扩展 Implementation Plan

> **For agentic workers:** Use wukong-code:executing-plans task-by-task. Steps use checkbox (`- [x]`) syntax.

**Goal:** 净增 **40** 条规则（86 → **126**），跨过 Top 100 里程碑。

**参照：** `docs/wukong-code/specs/2026-07-29-phase4c-rules-batch4-design.md`

## Global Constraints

- 净增 ∈ [30, 50]；新增 custom = 0
- 分支 `cursor/phase4c-batch4-378f`

---

## Task 1: 基线 — [x]

- [x] 确认 ported=80、总规则≈86
- [x] 记录 custom 占比

## Task 2: 选批 — [x]

- [x] `docs/findings/2026-07-phase4c-batch4-selection.md`

## Task 3: Block A app-caches +18 — [x]

- [x] TOML + fixture（Spotify、Blender、Teams legacy GPU、NetNewsWire）

## Task 4: Block B user-devtools +22 — [x]

- [x] TOML + fixture（TypeScript、Vite、Expo Go）

## Task 5: 门禁 — [x]

```bash
cargo test -p vole-core
bash scripts/verify-clean-candidates.sh
cargo clippy -p vole-core -- -D warnings
```

- [x] ported=120；总规则≈126

## Task 6: 文档 — [x]

- [x] README、spec 状态、selection Actual count

## Task 7:（可选）VOLE_TEST_ROOT 双跑 — [x]

- [x] 跳过（无 VOLE_TEST_ROOT；CI conformance-plan-only 已通过 prior batch 模式）

---

Plan complete. Batch 4 implemented in `cursor/phase4c-batch4-378f`.
