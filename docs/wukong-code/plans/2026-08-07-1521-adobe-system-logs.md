# Adobe System Logs Implementation Plan

**Goal:** `adobe-system-logs`（1.20.0）— 双树 + adobegc。

### Task 1
谓词 `is_adobe_system_log_clean_target`；Privilege candidates（双 walk + exact）；plan/apply；TOML `older_than_days=7`；coverage；1.20.0。

### Task 2
测：旧 Adobe 叶删；旧 adobegc 删；新鲜 skip；三树伪造 skip；PR + security + merge。
