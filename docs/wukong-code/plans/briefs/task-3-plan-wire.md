### Task 3: Plan 管线接入

From: docs/wukong-code/plans/2026-07-30-guard-not-running.md Task 3.

**Files:**
- Modify: crates/vole-core/src/ops/mod.rs
- Modify: crates/vole-core/src/ops/plan.rs
- Modify: crates/vole-core/src/clean_fixture.rs if Orchestrator::new signature changes

**Requirements:**
- Orchestrator gains `process_probe: Arc<dyn ProcessProbe>`
- `Orchestrator::new(cancel, events)` defaults to `Arc::new(PgrepProcessProbe)`
- `Orchestrator::with_process_probe(cancel, events, probe: Arc<dyn ProcessProbe>)` for tests
- In `build_plan_with`, after `disabled` check, before strategy resolve:
  if should_skip_for_not_running(probe, &rule.guards.not_running) {
    emit Skipped { rule_id, reason: AppRunning }; continue;
  }
- Tests: plan_skips_rule_when_not_running_guard_hits + plan_selects_when_process_idle
- Use FakeProcessProbe + VOLE_TEST_HOME / test_env like existing plan tests
- Commit: feat(ops): skip clean rules when not_running guard matches

Do NOT change apply yet (Task 4).
