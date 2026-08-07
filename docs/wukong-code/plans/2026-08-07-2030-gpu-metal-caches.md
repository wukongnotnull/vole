# Plan: gpu-metal-caches（1.25.0）

**Goal:** 陈旧 `*/C/*/com.apple.metal*` 目录（Mole stale 语义、跳过 EDR、sudo -n）。

谓词 `is_gpu_metal_cache_clean_target`；`gpu_metal_cache_is_stale`；folders walk maxdepth 8；plan/apply；TOML `kind=all`；1.25.0。
