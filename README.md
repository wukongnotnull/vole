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

计划中的 v1 只实现 Mole 十二个子命令中的三个：`status`、`analyze`、`clean`。
设计文档见 `docs/wukong-code/specs/`。

**Phase 2 状态（2026-07-29）**：`vole status` 可用（TUI、`--json`、`--json-stream`）；`analyze` 与真实 `clean` 仍不可用。Phase 1 基础设施（协议、oplog、互斥等）已落地。TCC 完整矩阵 deferred，见 `docs/findings/2026-07-phase1-tcc-deferred.md`。
