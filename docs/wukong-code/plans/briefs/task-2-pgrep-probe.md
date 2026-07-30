### Task 2: `PgrepProcessProbe`（真实探测）

From plan `docs/wukong-code/plans/2026-07-30-guard-not-running.md` Task 2.

**Files:** Modify `crates/vole-core/src/rules/process_guard.rs`

**Produces:**
- `pub(crate) fn state_from_pgrep_status(code: Option<i32>, timed_out: bool) -> ProcessState`
  - timed_out → Unknown; Some(0) → Running; Some(1) → Idle; else Unknown
- `pub struct PgrepProcessProbe;` + `impl ProcessProbe`
  - uses `vole_sys::macos::MacSysCommand` + `SysCommand::run(&["pgrep", "-x", name], Duration::from_secs(2))`
  - map Timeout → Unknown; Ok(output) → status code via state_from_pgrep_status; Err → Unknown

Export `PgrepProcessProbe` from `rules/mod.rs`.

TDD on `state_from_pgrep_status`. Commit:
```
feat(rules): implement PgrepProcessProbe via pgrep -x
```

Do NOT wire plan/apply yet.
