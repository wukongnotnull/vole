# Task 5 Review

**Verdict: Approved**

Task 5 diff (`742ecb8..1359fad`) is data-only and matches the brief: `firefox-cache` gains `[rule.guards] not_running = ["Firefox"]`; three new rules (`dropbox-cache`, `google-drive-cache`, `onedrive-cache`) each declare `not_running` with mole-aligned paths and exact `pgrep -x` process names; and four clean fixtures (`batch_guard_firefox_cache_selects_child.json`, `batch_guard_dropbox_cache_selects_child.json`, `batch_guard_google_drive_cache_selects_child.json`, `batch_guard_onedrive_cache_selects_child.json`) assert path selection when the target apps are idle. No engine or schema changes; guard semantics remain covered by Tasks 3–4 unit tests. The only operational caveat (fixtures may fail if Firefox/Dropbox/Google Drive/OneDrive is running locally) is documented in the task report and acceptable per plan.
