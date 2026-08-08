# Batch 6 收口 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans or inline TDD. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 3 条 `all` 规则补齐 Antigravity/MCP 兄弟路径 + QQ Music 容器 AS 缓存；发版 **1.41.0**；路线图取消「暂停必做」。

**Architecture:** 纯 TOML + `path.rs` 保护扩段；无 handler。

**Tech Stack:** Rust / TOML rules / existing fixture harness

## File map

| 文件 | 职责 |
|---|---|
| `crates/vole-core/src/protection/path.rs` | CACHE_SEGMENTS + QQ AS 豁免 |
| `data/rules/user-devtools.toml` | AG + MCP siblings |
| `data/rules/app-caches.toml` | QQ Music AS |
| fixtures ×3 | 各家族选中叶 |
| `coverage.rs` / README / Formula / releases / 0119 | 540 / 1.41.0 / 必做恢复 |

## Task 1: 保护层

- [ ] RED：segments + `is_qq_music_mac_as_cache_path` 单测（含 iDownloadProxy 仍保护）
- [ ] GREEN：扩段 `GraphiteDawnCache`、`DawnCache`、`GrShaderCache`、`component_crx_cache`、`extensions_crx_cache`、`Service Worker/CacheStorage`；QQ 四目录豁免进 `is_explicit_clean_cache_path` 且 step3 当 container_cache
- [ ] Commit `fix(protection): allow AG/MCP sibling caches and QQ Music AS caches`

## Task 2: TOML + fixtures

- [ ] 三条规则 + 三个 fixture
- [ ] `cargo test -p vole-core --lib`；verify fixtures
- [ ] Commit `feat(clean): antigravity/MCP siblings and QQ Music AS caches`

## Task 3: 发版与路线图

- [ ] version 1.41.0；coverage 540；releases；0119 必做完成
- [ ] Commit `chore(release): bump 1.41.0 for Batch6 siblings closeout`
- [ ] PR + merge commit

## Spec coverage

| Spec | Task |
|---|---|
| 三规则路径 | T2 |
| 保护 / QQ | T1 |
| 1.41.0 / 0119 | T3 |
