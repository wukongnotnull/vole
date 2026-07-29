# Vole

用 Rust 实现的 macOS 清理与监控命令行工具，目前处于早期开发阶段。

## 与 Mole 的关系

Vole 的清理规则知识、路径保护清单与安全校验语义来自 [tw93/Mole](https://github.com/tw93/Mole) v1.48.1，
是 Mole 的衍生作品。感谢 Mole 作者与贡献者多年积累的这份知识。

Vole 是一个独立项目，不隶属于 Mole，也不与 Mole 保持功能对齐。如果你想要一个成熟可用的工具，
请直接使用 Mole。

## 许可证

GPL-3.0。因为 Vole 是 GPL-3.0 作品的衍生作品，这是唯一可选的许可证——包括未来的桌面 app 在内，
本项目的所有部分都以 GPL-3.0 发布。

## 范围

v1 子命令：`status`、`analyze`、`clean`、`history`（另有 `completions` 与无参交互菜单）。
设计文档见 `docs/wukong-code/specs/`。

**Phase 2 状态（2026-07-29）**：`vole status` 可用（TUI、`--json`、`--json-stream`）。Phase 1 基础设施（协议、oplog、互斥等）已落地。TCC 完整矩阵 deferred，见 `docs/findings/2026-07-phase1-tcc-deferred.md`。

**Phase 3 状态（2026-07-29）**：`vole analyze` 目录模式可用（TUI 下钻、`--json`）；默认路径为 `$HOME`（mole 无参为 `/` 概览，待后续）。扫描含硬链接去重、折叠目录、`jwalk` 并行遍历。验证：`scripts/verify-analyze-json.sh`。

**Phase 4 状态（2026-07-29）**：`vole clean` 可用——`--plan` / `--apply` 两阶段、`--json-stream` NDJSON、`--whitelist` 白名单管理；默认移入废纸篓，`--permanent` 永久删除。报告区分 `trashed_bytes` / `deleted_bytes`。验证：`scripts/verify-clean-candidates.sh`。计划见 `docs/wukong-code/plans/2026-07-29-phase4-clean.md`。

**Phase 4c Batch 2（2026-07-29）**：规则覆盖扩至约 **46** 条（本批净增 40：`data/rules/app-caches.toml` + `user-devtools.toml`）。仍远低于设计 Top 100–150；下一批继续。选批见 `docs/findings/2026-07-phase4c-batch2-selection.md`。计划见 `docs/wukong-code/plans/2026-07-29-phase4c-rules-batch2.md`。

**Phase 5 状态（2026-07-29）**：`vole history`（文本 / `--json` / `--limit`，对齐 mole）；`docs/protocol.md` 已 FROZEN；无参 `vole` 进入轻量菜单；`vole completions <shell>` 生成补全。验证：`scripts/verify-history-mole.sh`、`scripts/check-protocol-doc.sh`。计划见 `docs/wukong-code/plans/2026-07-29-phase5-history-protocol.md`。签名 / Homebrew 仍为占位，见 `docs/findings/2026-07-phase5-signing.md`。

**补全**：

```bash
vole completions zsh > ~/.zfunc/_vole
```
