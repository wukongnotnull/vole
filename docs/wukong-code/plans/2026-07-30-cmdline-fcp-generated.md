# cmdline + FCP generated Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans (preferred here: inline) or subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add `not_running_cmdline` (`pgrep -f`) and port Final Cut Pro generated-cache cleanup with FCP process guards.

**Architecture:** Extend `ProcessProbe` + `GuardsConfig`; plan/apply skip on either exact or cmdline hit. New custom handler walks `~/Movies/*.fcpbundle` for safe regenerable media dirs only.

**Tech Stack:** Rust 1.97.1, existing vole-core rules/ops, SysCommand `pgrep`.

**Design:** `docs/wukong-code/specs/2026-07-30-cmdline-fcp-generated-design.md`

## Global Constraints

- GPL-3.0-only; macOS only; vole-core `forbid(unsafe_code)`.
- Fail-closed Unknown → skip; `SkipReason::AppRunning` only.
- No Jianying / Simulator / Chrome cmdline bulk; no schema_version bump.
- Prefer inline execution; commit per task.

---

### Task 1: Schema + ProcessProbe cmdline

**Files:** `schema.rs`, `process_guard.rs`, `mod.rs`, plan.rs / apply_plan.rs skip helper call sites

- [x] Add `not_running_cmdline: Vec<String>` to `GuardsConfig` (default empty)
- [x] Extend `ProcessProbe` with `cmdline_substring_running`
- [x] `PgrepProcessProbe`: `pgrep -f needle`
- [x] `FakeProcessProbe` fields for cmdline
- [x] Replace skip helper with `should_skip_for_guards(probe, &GuardsConfig)` checking both lists
- [x] Update plan/apply to use new helper
- [x] Tests for cmdline skip / idle / unknown
- [x] Commit: `feat(rules): add not_running_cmdline pgrep -f guard`

### Task 2: FCP generated custom handler

**Files:** `custom_handlers.rs`, `data/rules/app-caches.toml`, fixtures

- [x] Implement `final_cut_pro_generated_caches` handler
- [x] Add rule with guards (exact + cmdline)
- [x] Fix/extend fixtures: add `expect_selected` for safe dirs; full paths in `expect_not_selected`
- [x] Unit test: skip when FakeProbe says FCP running
- [x] `cargo test -p vole-core`
- [x] Commit: `feat(rules): Final Cut Pro generated cache with cmdline guard`
- [x] Update README rule count; findings short note; mark plan checkboxes

---

## Execution

Inline in this session (user preference).
