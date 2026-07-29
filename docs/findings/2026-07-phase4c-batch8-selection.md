# Phase 4c Batch 8 selection

Target: **40** rules (230 → **270**)  
**Actual: 40** (app-caches +20, user-devtools +20).

## Block A — gaming / Steam / Podcasts (+20)

Steam 子缓存、Minecraft/Lunar Client、RPCS3/PCSX2、Obsidian、Podcasts、Klee desktop 等。

## Block B — AI desktop / DB / API tools (+20)

Claude×6、Qoder×5、OpenCode×2、Navicat/DBeaver/Compass/Redis Insight/Paw、Charles/Proxyman。

详见 `data/rules/*.toml`；完整路径见 inventory。

## 依赖

Batch 8 **Navicat** 等规则依赖保护层 refine（`is_explicit_clean_cache_path`）。

## Milestone

**270** 规则；`ported` **266/513**（≈52%）。
