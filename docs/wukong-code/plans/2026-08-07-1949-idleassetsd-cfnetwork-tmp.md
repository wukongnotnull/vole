# Plan: idleassetsd-cfnetwork-tmp（1.23.0）

**Goal:** 定位 `*/T/com.apple.idleassetsd` 下陈旧 `CFNetworkDownload_*.tmp`（≥7d、sudo -n）。

谓词 `is_idleassetsd_cfnetwork_tmp_clean_target`；folders 定位 walk + file walk；plan/apply；TOML；coverage；1.23.0。
