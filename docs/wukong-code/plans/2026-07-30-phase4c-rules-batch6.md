# Phase 4c Batch 6 Implementation Plan

**Goal:** +40 rules (150 → **190**); Phase 4c+ expansion.

**参照:** `docs/wukong-code/specs/2026-07-30-phase4c-rules-batch6-design.md`

## Task 1: 基线 — [x]

- [x] `python3 scripts/inventory-mole-rules.py` → ported=185, enabled=190

## Task 2: 选批 — [x]

- [x] `docs/findings/2026-07-phase4c-batch6-selection.md`

## Task 3: Block A +20 app-caches — [x]

- [x] Append to `data/rules/app-caches.toml`

## Task 4: Block B +20 user-devtools — [x]

- [x] Append to `data/rules/user-devtools.toml`

## Task 5: Fixtures — [x]

- [x] 5× `tests/fixtures/clean/batch6_*_selects_child.json`
- [x] Update `dual_run_allowlist*.txt`

## Task 6: 门禁 — [x]

- [x] `cargo test -p vole-core verify_clean_fixtures`
- [x] `bash scripts/verify-clean-candidates.sh`（无 VOLE_TEST_ROOT）

## Task 7: 文档 — [x]

- [x] README 规则 **190**
- [x] Spec/plan

Branch: `cursor/phase4c-batch6-378f`
