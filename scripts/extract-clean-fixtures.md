# extract-clean-fixtures

Semi-automated bridge from Mole `clean_*.bats` tests to table-driven JSON fixtures
for dual-run / rule-engine tests (design doc §7B).

## Usage

```bash
python3 scripts/extract-clean-fixtures.py
```

Optional flags:

- `--repo-root PATH` — repository root (default: parent of `scripts/`)
- `--out-dir PATH` — output directory (default: `tests/fixtures/clean/`)
- `--bats clean_ai_cli_caches.bats` — restrict to specific bats files (repeatable)

## Output shape

Each file is one `@test` block:

```json
{
  "id": "clean_dev_ai_agents_reaps_stale_claude_desktop_bundled_versions_when_active_version_is_known",
  "source_bats": "third_party/mole-1.48.1/tests/clean_ai_cli_caches.bats",
  "source_test": "clean_dev_ai_agents reaps stale Claude Desktop bundled versions when active version is known",
  "fixture": [
    { "mkdir": "~/Library/Application Support/Claude/claude-code/2.1.140", "mtime": "2026-04-01T00:00" },
    { "write": "~/Library/Application Support/Claude/claude-code-vm/.sdk-version", "content": "2.1.150" }
  ],
  "expect_selected": [
    "~/Library/Application Support/Claude/claude-code/2.1.140|Claude Desktop bundled Claude Code old version"
  ],
  "expect_not_selected": [
    "~/Library/Application Support/Claude/claude-code/2.1.142"
  ]
}
```

Paths use `~` instead of `$HOME`. Selected expectations use `path|label` when the
bats assertion encodes a `SAFE_CLEAN:label|path` line.

## Allowlist

By default only bats files with predictable `mkdir -p` / `touch -t` / `SAFE_CLEAN`
patterns are scanned:

- `clean_ai_cli_caches.bats`
- `clean_dev_caches.bats`
- `clean_app_caches.bats`

Expand the allowlist in `extract-clean-fixtures.py` after spot-checking a file.

## Limitations (human review required)

- **Semi-automated only.** Complex heredocs, staging roots built from `mktemp`,
  inline `[[ "$output" == ... ]]` without `SAFE_CLEAN`, and browser version tests
  that assert item counts instead of paths are skipped or partially extracted.
- Local `local foo="$HOME/..."` variables are expanded; other indirection is not.
- `touch -t` times are converted to ISO-like `YYYY-MM-DDThh:mm` (seconds dropped).
- Negative assertions that are substring guards (e.g. `"· skipped"`) are filtered out.
- Extracted JSON is a starting point — review `expect_*` before wiring rule tests.

## Verification

```bash
python3 scripts/extract-clean-fixtures.py
jq . tests/fixtures/clean/*.json
cargo test -p conformance clean_fixture::
```
