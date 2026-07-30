# Task 5 Report: Rules data subset for not_running guards

## What was implemented

1. **`firefox-cache`** — added `[rule.guards] not_running = ["Firefox"]` to existing rule in `data/rules/user-devtools.toml`.

2. **Three new rules** (category `user-devtools`, `platform = ["macos"]`, `kind = "all"`, `last_verified = "2026-07"`):
   - `dropbox-cache` — paths `~/Library/Caches/com.dropbox.*`, `~/Library/Caches/com.getdropbox.dropbox`; `not_running = ["Dropbox"]`; label "Dropbox cache"
   - `google-drive-cache` — path `~/Library/Caches/com.google.GoogleDrive`; `not_running = ["Google Drive"]`; label "Google Drive cache"
   - `onedrive-cache` — path `~/Library/Caches/com.microsoft.OneDrive`; `not_running = ["OneDrive"]`; label "OneDrive cache"

3. **Four clean fixtures** under `tests/fixtures/clean/`:
   - `batch_guard_firefox_cache_selects_child.json` — `~/Library/Caches/Firefox/temp`
   - `batch_guard_dropbox_cache_selects_child.json` — `~/Library/Caches/com.dropbox.foo` (+ cache file)
   - `batch_guard_google_drive_cache_selects_child.json` — `~/Library/Caches/com.google.GoogleDrive`
   - `batch_guard_onedrive_cache_selects_child.json` — `~/Library/Caches/com.microsoft.OneDrive`

Paths/labels align with mole `lib/clean/user.sh` (`pgrep -x` names and `safe_clean` labels).

## TDD Evidence

Task 5 is data-only (no new Rust). Verification is fixture-driven:

### GREEN

```bash
cargo test -p vole-core verify_clean_fixtures -- --nocapture
# test clean_fixture::verify_clean_fixtures::all_extracted_fixtures_satisfy_plan_expectations ... ok

cargo test -p vole-core -- --nocapture
# test result: ok. 127 passed; 0 failed; 1 ignored
```

All four new fixtures pass with default `Orchestrator::new()` / `PgrepProcessProbe` on this machine (Firefox/Dropbox/Google Drive/OneDrive not running).

## Files changed

- `data/rules/user-devtools.toml` — guards on `firefox-cache`; new `dropbox-cache`, `google-drive-cache`, `onedrive-cache`
- `tests/fixtures/clean/batch_guard_firefox_cache_selects_child.json` (new)
- `tests/fixtures/clean/batch_guard_dropbox_cache_selects_child.json` (new)
- `tests/fixtures/clean/batch_guard_google_drive_cache_selects_child.json` (new)
- `tests/fixtures/clean/batch_guard_onedrive_cache_selects_child.json` (new)

## Self-review

- Spec coverage: all four rules + four fixtures per brief; mole-aligned paths and process names.
- `verify_clean_fixtures` uses real `pgrep` — if Firefox/Dropbox/Google Drive/OneDrive is running locally or on CI, matching fixtures will fail (rule skipped, `expect_selected` empty). Guard logic is covered by Task 3/4 unit tests with `FakeProcessProbe`; these fixtures only assert path selection when idle.
- No engine or schema changes in this task.

## Concerns

- **Flaky on busy machines:** Same as plan note — running target apps cause fixture failures. Acceptable; document for operators running full suite with apps open.
