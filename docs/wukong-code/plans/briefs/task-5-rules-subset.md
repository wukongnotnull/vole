### Task 5: 规则数据子集

From plan Task 5.

**Files:**
- Modify: data/rules/user-devtools.toml
- Create fixtures under tests/fixtures/clean/:
  - batch_guard_firefox_cache_selects_child.json
  - batch_guard_dropbox_cache_selects_child.json
  - batch_guard_google_drive_cache_selects_child.json
  - batch_guard_onedrive_cache_selects_child.json

**Rules:**
1. Add to existing firefox-cache:
```toml
[rule.guards]
not_running = ["Firefox"]
```

2. Add new rules (strategy kind=all, last_verified=2026-07, platform macos, category user-devtools):
- dropbox-cache: paths ~/Library/Caches/com.dropbox.* and ~/Library/Caches/com.getdropbox.dropbox; not_running=["Dropbox"]; label "Dropbox cache"
- google-drive-cache: ~/Library/Caches/com.google.GoogleDrive; not_running=["Google Drive"]; label "Google Drive cache"
- onedrive-cache: ~/Library/Caches/com.microsoft.OneDrive; not_running=["OneDrive"]; label "OneDrive cache"

Fixtures: mkdir a selectable child under each path pattern; expect_selected with path|label format like other batch fixtures.

Run: cargo test -p vole-core verify_clean_fixtures
Commit: feat(rules): add Firefox/Dropbox/Drive/OneDrive not_running guards
