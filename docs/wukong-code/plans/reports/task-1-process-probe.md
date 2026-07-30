# Task 1 Report: ProcessProbe + unit tests (TDD)

## Status

DONE

## What Was Implemented

- Created `crates/vole-core/src/rules/process_guard.rs` with:
  - `ProcessState` enum (`Running`, `Idle`, `Unknown`)
  - `ProcessProbe` trait (`Send + Sync`, `exact_name_running`)
  - `FakeProcessProbe` struct with `running` and `unknown` `HashSet<String>` fields
  - `should_skip_for_not_running(probe, names)` helper:
    - empty list → `false`
    - ignores empty strings
    - any `Running` or `Unknown` → `true`
- Exported all public types from `crates/vole-core/src/rules/mod.rs`

Did **not** implement `PgrepProcessProbe` (Task 2) or wire plan/apply (Tasks 3–4).

## TDD Evidence

### RED

```bash
cargo test -p vole-core should_skip_for_not_running -- --nocapture
```

Expected compile failure — symbols missing:

```
error[E0432]: unresolved imports `process_guard::should_skip_for_not_running`, ...
error[E0425]: cannot find function `should_skip_for_not_running` in this scope
error[E0433]: cannot find type `FakeProcessProbe` in this scope
```

### GREEN

```bash
cargo test -p vole-core process_guard -- --nocapture
```

```
running 4 tests
test rules::process_guard::tests::empty_names_never_skips ... ok
test rules::process_guard::tests::idle_when_none_running ... ok
test rules::process_guard::tests::skips_when_any_exact_name_running ... ok
test rules::process_guard::tests::unknown_fail_closed_skips ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 117 filtered out
```

Full suite before commit:

```bash
cargo test -p vole-core
```

```
test result: ok. 120 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

Output pristine (no warnings in final build).

## Files Changed

- `crates/vole-core/src/rules/process_guard.rs` (new)
- `crates/vole-core/src/rules/mod.rs` (added `mod process_guard` + `pub use`)

## Commit

- `76a8ba4` feat(rules): add ProcessProbe and not_running skip helper

## Self-Review

- **Completeness:** All Task 1 interfaces implemented per plan; no scope creep.
- **Quality:** Names match design spec; logic is minimal and matches plan snippet exactly.
- **Discipline:** No plan/apply wiring, no `PgrepProcessProbe`, no extra tests beyond plan.
- **Testing:** Four unit tests cover empty list, running match, idle, and unknown fail-closed paths.

## Concerns

None.
