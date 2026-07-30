# Task 3 Report: Plan pipeline wires not_running

## What was implemented

- `Orchestrator` holds `process_probe: Arc<dyn ProcessProbe>`.
- `Orchestrator::new` defaults to `Arc::new(PgrepProcessProbe)`.
- `Orchestrator::with_process_probe(cancel, events, probe)` for test injection.
- In `build_plan_with`, after `disabled` check and before `resolve_strategy`:
  if `should_skip_for_not_running` hits, emit `Skipped { reason: AppRunning }` and `continue`.
- Tests: `plan_skips_rule_when_not_running_guard_hits`, `plan_selects_when_process_idle`.

`clean_fixture.rs` unchanged — `Orchestrator::new` signature unchanged.

## TDD Evidence

### RED

```bash
cargo test -p vole-core plan_skips_rule_when_not_running_guard_hits -- --nocapture
```

After adding tests + `with_process_probe` (no plan skip logic):

```
assertion failed: plan.entries.is_empty()
test ops::plan::tests::plan_skips_rule_when_not_running_guard_hits ... FAILED
```

Expected: rule with `not_running=["claude"]` and Fake probe reporting `claude` running should produce no entries and emit `AppRunning`.

### GREEN

```bash
cargo test -p vole-core plan_skips_rule_when_not_running_guard_hits -- --nocapture
cargo test -p vole-core plan_selects_when_process_idle -- --nocapture
cargo test -p vole-core -- --nocapture
```

```
test ops::plan::tests::plan_skips_rule_when_not_running_guard_hits ... ok
test ops::plan::tests::plan_selects_when_process_idle ... ok
test result: ok. 126 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

Output pristine (no warnings after skip logic wired).

## Files changed

- `crates/vole-core/src/ops/mod.rs`
- `crates/vole-core/src/ops/plan.rs`

## Self-review

- Spec coverage: all Task 3 requirements met; apply unchanged (Task 4).
- Tests use `test_env::lock()`, scratch dirs, `FakeProcessProbe`, and event channel pattern consistent with existing plan tests.
- `process_probe` field is private; only accessed via `build_plan_with` in plan.rs.

## Concerns

None.
