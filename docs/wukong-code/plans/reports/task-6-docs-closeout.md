# Task 6 Report: Docs and closeout

## What was implemented

1. **`docs/findings/2026-07-guard-not-running.md`** — findings short note covering engine behavior (`pgrep -x`, fail-closed, plan + apply), rule subset table (Firefox + three cloud caches + existing AI/Codex), non-goals (cmdline/generated/Simulator), and rule count 473.

2. **`crates/vole-core/src/ops/coverage.rs`** — softened `coverage_note`: replaced「需进程检测的 guard 规则」with「generated/cmdline 类 guard … 尚未移植；精确进程名 not_running 子集已落地」.

3. **`README.md`** — updated rule count **470 → 473** (4 occurrences in rule-scale tables + link to new findings doc).

4. **Plan checkboxes** — all 25 `- [ ]` in `docs/wukong-code/plans/2026-07-30-guard-not-running.md` marked `[x]`.

## Verification

```bash
rg -c '^\[\[rule\]\]' data/rules/*.toml
# ai-agents:3 codex:1 app-caches:243 user-devtools:224 example:2 → total 473

cargo fmt --all -- --check
# ok (ran cargo fmt once to fix prior-task drift in apply_plan/plan/process_guard)

cargo clippy -p vole-core -p vole-cli --all-targets -- -D warnings
# ok after #[allow(clippy::too_many_arguments)] on ApplyPlanContext::new and test run_apply

cargo test -p vole-core
# 127 passed; 0 failed; 1 ignored
```

## Files changed

- `docs/findings/2026-07-guard-not-running.md` (new)
- `crates/vole-core/src/ops/coverage.rs`
- `README.md`
- `crates/vole-core/src/ops/apply_plan.rs` (clippy allow + rustfmt)
- `crates/vole-core/src/ops/plan.rs` (rustfmt)
- `crates/vole-core/src/rules/process_guard.rs` (rustfmt)
- `docs/wukong-code/plans/2026-07-30-guard-not-running.md` (checkboxes)

## Self-review

- Spec coverage: findings, coverage_note, README count, verification, two commits per plan.
- Rule count verified via `rg`; matches plan expectation 470 + 3 = 473.
- Clippy/fmt fixes from Tasks 3–4 were included in the docs commit so verification gate passes; minimal `allow` only, no API refactor.

## Concerns

- None blocking. Historical release notes (v0.0.7–v0.0.9) still say 470; left unchanged (point-in-time release docs).

## Commits

| SHA | Subject |
|---|---|
| `25df0ac` | docs: record guard not_running subset landing |
| `1529ecb` | docs(plan): mark guard not_running tasks complete |
