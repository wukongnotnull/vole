# Task 2 Report: PgrepProcessProbe

## Status

DONE

## What Was Implemented

- Added `state_from_pgrep_status(code, timed_out)` in `process_guard.rs`:
  - `timed_out` → `Unknown`
  - `Some(0)` → `Running`
  - `Some(1)` → `Idle`
  - all other codes / `None` → `Unknown`
- Added `PgrepProcessProbe` implementing `ProcessProbe`:
  - runs `MacSysCommand::run(&["pgrep", "-x", name], Duration::from_secs(2))`
  - `Timeout` or any `Err` → `Unknown`
  - `Ok(output)` → maps exit code via `state_from_pgrep_status`
- Exported `PgrepProcessProbe` from `rules/mod.rs`

Did **not** wire plan/apply (Task 3+) or add integration tests against real `pgrep`.

## TDD Evidence

### RED

Tests added before implementation body; expected compile failure for missing `state_from_pgrep_status`:

```bash
cargo test -p vole-core state_from_pgrep_status -- --nocapture
```

Expected:

```
error[E0425]: cannot find function `state_from_pgrep_status` in this scope
```

### GREEN

```bash
cargo test -p vole-core state_from_pgrep_status -- --nocapture
```

```
running 4 tests
test rules::process_guard::tests::state_from_pgrep_status_exit_one_is_idle ... ok
test rules::process_guard::tests::state_from_pgrep_status_exit_zero_is_running ... ok
test rules::process_guard::tests::state_from_pgrep_status_other_exit_is_unknown ... ok
test rules::process_guard::tests::state_from_pgrep_status_timeout_is_unknown ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 121 filtered out
```

Full suite before commit:

```bash
cargo test -p vole-core
```

```
test result: ok. 124 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

Output pristine (no warnings).

## Files Changed

- `crates/vole-core/src/rules/process_guard.rs` — mapping helper, `PgrepProcessProbe`, 4 new unit tests
- `crates/vole-core/src/rules/mod.rs` — export `PgrepProcessProbe`

## Commit

- `039c107` feat(rules): implement PgrepProcessProbe via pgrep -x

## Self-Review

- **Completeness:** All Task 2 interfaces implemented per plan; no plan/apply wiring.
- **Quality:** `state_from_pgrep_status` is testable in isolation; probe fail-closes to `Unknown` on errors/timeouts.
- **Discipline:** No scope creep; no integration test (plan marks as optional).
- **Testing:** Four mapping tests cover exit 0/1, other exits, and timeout flag.

## Concerns

None.
