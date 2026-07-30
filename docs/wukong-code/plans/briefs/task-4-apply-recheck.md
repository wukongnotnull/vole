### Task 4: Apply 再检

From plan Task 4 in docs/wukong-code/plans/2026-07-30-guard-not-running.md

**Files:**
- Modify: crates/vole-core/src/ops/apply_plan.rs
- Modify: crates/vole-cli/src/clean.rs

**Requirements:**
- ApplyPlanContext gains `rules: &'a [Rule]` and `process_probe: &'a dyn ProcessProbe`
- apply_proto_plan / ApplyPlanContext::new updated to take rules + probe (use PgrepProcessProbe in CLI)
- Before delete for each entry: lookup rule by rule_id; if should_skip_for_not_running → emit AppRunning, skip, do not delete
- Missing rule_id: do not skip for guard (continue path validation)
- Failing-then-passing test with FakeTrash + FakeProcessProbe running
- CLI run_apply already loads rules — pass them in

Commit: feat(ops): re-check not_running guards on clean --apply
