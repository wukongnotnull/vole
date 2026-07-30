# Task 4 Report: Apply re-checks not_running guards

## What was implemented

- `ApplyPlanContext` gains `rules: &'a [Rule]` and `process_probe: &'a dyn ProcessProbe`.
- `ApplyPlanContext::new` and `apply_proto_plan` updated to require rules + probe.
- In `apply_plan`, before path validation/delete: lookup `entry.rule_id` in `ctx.rules`; if
  `should_skip_for_not_running` hits, emit `Skipped { AppRunning }`, increment skipped, continue.
- Missing `rule_id` in rules slice: no guard skip (continues to path validation).
- CLI `run_apply` loads rules via `load_rules_from_dir` and passes `&PgrepProcessProbe`.
- Test `apply_skips_when_not_running_guard_hits_at_apply` uses `FakeTrash` + `FakeProcessProbe`
  (Firefox running) — file kept, trash not called, `AppRunning` in skip summary.

## TDD Evidence

### RED

```bash
cargo test -p vole-core apply_skips_when_not_running_guard_hits_at_apply -- --nocapture
```

After adding the failing test and updated `run_apply` helper (before `ApplyPlanContext` wiring):

```
error[E0061]: this function takes 7 arguments but 9 arguments were supplied
   --> crates/vole-core/src/ops/apply_plan.rs:351:23
```

Expected: test module could not compile until context/probe fields exist. Once wired without
guard logic, the test would fail `assert_eq!(report.succeeded, 0)` (file deleted instead).

### GREEN

```bash
cargo test -p vole-core apply_skips_when_not_running_guard_hits_at_apply -- --nocapture
cargo test -p vole-core -- --nocapture
cargo check -p vole-cli
```

```
test ops::apply_plan::tests::apply_skips_when_not_running_guard_hits_at_apply ... ok
test result: ok. 127 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

Output pristine (no warnings).

## Files changed

- `crates/vole-core/src/ops/apply_plan.rs`
- `crates/vole-cli/src/clean.rs`

## Self-review

- Spec coverage: all Task 4 requirements met; guard runs after plan-level `skip_reason` but before delete.
- `FakeTrash` is test-local (not exported); pattern matches plan tests' `FakeProcessProbe` usage.
- Existing apply tests use `run_apply_defaults` with empty rules + idle probe — behavior unchanged.

## Concerns

None.
