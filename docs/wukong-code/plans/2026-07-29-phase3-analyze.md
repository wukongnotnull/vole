# Phase 3：`analyze` 命令 Implementation Plan

**Goal:** 实现 `analyze` 子命令——目录扫描、硬链接去重、大文件榜、`--json` 与 TUI 下钻。

**Architecture:** `vole-proto::AnalyzeOutput` 对齐 mole `jsonOutput`；`vole-core::scan` 用 `jwalk` 并行遍历、`fold` 跳过 `node_modules` 等；`vole-cli` 复用 Phase 2 终端/信号基础设施。

**Tech Stack:** `jwalk` 0.8、ratatui、crossterm。

**参照设计文档：** `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` Phase 3。

## 已实现（2026-07-29）

- [x] `vole-proto/src/analyze.rs` — `AnalyzeOutput` / `AnalyzeEntry` / `AnalyzeFileEntry`
- [x] `vole-core/src/scan/` — `jwalk`、硬链接 `(dev,ino)` 去重、`st_blocks*512` 体积
- [x] `vole-core/src/analyze/` — 组装 JSON
- [x] `vole-cli analyze` — `--json`、TUI（↑↓/Enter/Esc/q）
- [x] `scripts/verify-analyze-json.sh`

## 已知简化 / deferred

- 无参默认 `$HOME`，非 mole 的 `/` 概览模式
- 无 Spotlight 大文件补充、无扫描缓存（`twox-hash` / `cache.go`）
- TUI 无预览/废纸篓；大文件区仅展示前 4 条
- `conformance/fixtures/perf-tree` 性能基准未建

## 验收

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo run -p vole-cli -- analyze /tmp --json
bash scripts/verify-analyze-json.sh
```
