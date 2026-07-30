# Task 4 Review: Apply re-checks not_running guards

**Base:** `0b99b60` · **Head:** `742ecb8` · **Diff:** `task-4-diff.patch`

## Spec Compliance

- ✅ Spec compliant
- ⚠️ Cannot verify from diff: runtime behavior of `should_skip_for_not_running` / `PgrepProcessProbe` (implemented in prior tasks; this diff calls them correctly at `apply_plan.rs:152` and `clean.rs:142`).

| Requirement | Evidence |
|-------------|----------|
| `ApplyPlanContext` gains `rules` + `process_probe` | `apply_plan.rs:42-43`, `apply_plan.rs:56-57`, `apply_plan.rs:67-68` |
| `ApplyPlanContext::new` / `apply_proto_plan` take rules + probe | `apply_plan.rs:56-57`, `apply_plan.rs:80-81`, `apply_plan.rs:93-94` |
| Before delete: lookup rule, `should_skip_for_not_running` → `AppRunning`, skip, no delete | `apply_plan.rs:151-162` (after plan `skip_reason` at `:135-148`, before path validation at `:165+`) |
| Missing `rule_id`: no guard skip | `apply_plan.rs:151` — `if let Some(rule)` only; no match falls through |
| Failing-then-passing test with `FakeTrash` + running `FakeProcessProbe` | `apply_plan.rs:549-641` — file kept, `fake_trash.calls` empty, `AppRunning` in skip summary |
| CLI `run_apply` loads rules, passes `PgrepProcessProbe` | `clean.rs:136`, `clean.rs:142`, both `apply_proto_plan` branches `:155-156` / `:170-171` |

**API contract check (named risk):** `apply_proto_plan` signature changed; grep confirms sole production call site is `clean.rs` (both branches updated). `ApplyPlanContext::new` only called from `apply_proto_plan` and test helper.

## Strengths

- Apply guard block mirrors plan-phase pattern (`plan.rs:112-120`): same helper, same `SkipReason::AppRunning`, same event/skip_tracker recording.
- Guard placement is correct: re-check runs only for entries not already skipped at plan time (`skip_reason`), and before `verify_plan_entry_for_apply` / delete.
- New test proves end-to-end “no delete” via `FakeTrash` call recording, not just skip counters.
- Existing tests refactored through `run_apply_defaults` with empty rules + idle `FakeProcessProbe` — no behavior drift for prior cases.
- CLI wiring is symmetric for `--json-stream` and plain apply paths.

## Issues

#### Critical (Must Fix)

_None._

#### Important (Should Fix)

_None._

#### Minor (Nice to Have)

- `apply_plan.rs:151` — linear `rules.iter().find` per entry; fine at current scale; consider a `HashMap` only if rule count grows large.
- No dedicated test for missing `rule_id` (behavior is correct via `if let Some`; brief did not mandate a test).
- TDD RED phase was compile-time (`E0061`) before behavioral failure; acceptable for signature change but weaker than Task 3’s assertion-failed RED.

## Assessment

**Task quality:** Approved

**Reasoning:** All brief requirements are implemented with correct ordering, CLI wiring, and a behavioral test that verifies skip-without-delete. API change is fully propagated; no critical or important defects found.
