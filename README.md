# Vole

用 **Rust** 实现的 macOS 清理与监控 CLI：单一二进制、类型安全、默认可恢复，并为自动化与未来桌面端预留稳定协议。

> 清理规则知识、路径保护清单与安全校验语义源自 [tw93/Mole](https://github.com/tw93/Mole) v1.48.1。感谢 Mole 作者与贡献者多年积累。Vole 是独立衍生项目，不隶属于 Mole。

## 为什么选 Vole

| 亮点 | 说明 |
|---|---|
| **单一 Rust 二进制** | 不依赖 bash 运行时，也不要求用户额外安装 `fd` / `jq` / `sqlite3` |
| **类型安全与可测试** | 把「会删用户文件」的逻辑从数万行 Bash 收敛为可单测的 Rust crate |
| **plan → apply 两阶段清理** | 先预览候选，再按不可信 plan 重新过安全闸口；默认进废纸篓 |
| **冻结的 NDJSON 协议** | `--json-stream` / Plan / Report 已冻结，CLI、脚本与未来桌面 app 共用同一编排层 |
| **Mole 兼容 JSON** | `status` / `analyze` / `history` 同名字段口径对齐，现有脚本可平滑迁移 |
| **废纸篓口径诚实** | 报告区分 `trashed_bytes` / `deleted_bytes`，不把「进废纸篓」吹成「已释放」 |
| **扫描更扎实** | 硬链接去重、折叠目录、`jwalk` 并行遍历 |
| **规则数据化** | **470** 条高价值清理规则以 TOML 声明，可 diff、可禁用、可 fixture 回归 |

适合：想要更现代、更可脚本化、更偏「安全预览再执行」的日常清理与磁盘洞察。若你需要成熟全家桶（卸载、系统优化、purge 等），请继续用 [Mole](https://github.com/tw93/Mole)。

## 与 Mole 对比

两者共享同一套安全语义基因（保护路径、白名单、操作日志），但定位不同：

| | **Vole** | **Mole** |
|---|---|---|
| 实现 | 纯 Rust 单一二进制 | Bash + Go 混合 |
| 成熟度 | 早期可用（核心命令已落地） | 成熟、功能最全 |
| 核心命令 | `status` / `analyze` / `clean` / `history` | 另有 `uninstall` / `optimize` / `purge` / `installer` 等 |
| 清理模型 | `--plan` / `--apply` 两阶段 + 默认废纸篓 | `--dry-run` 预览 + 深度清理流水线 |
| 机器可读输出 | Mole 兼容 JSON **子集** + 自有 NDJSON 事件流 | `--json`（status / analyze / history） |
| 外部依赖 | 无第三方 CLI 依赖 | 部分场景推荐 `fd` 等 |
| 规则规模 | **470** 条高价值规则（Phase 4c+ 持续扩展） | 全量数百条 `safe_clean` 目标 |
| 桌面端路线 | 协议已为 SwiftUI sidecar 预留 | 另有商业 [Mole for Mac](https://mole.fit) |
| 许可证 | GPL-3.0（衍生作品，唯一可选） | GPL-3.0 |

**一句话**：Mole 是功能最全的「瑞士军刀」；Vole 是同知识底座上的 Rust 重写——更自包含、更可编排、更适合自动化与后续图形前端。

必须与 Mole **严格对齐**的三类契约：删除前路径校验与保护清单、同名 JSON 字段口径、操作日志格式（便于从 Mole 迁移）。交互与功能允许分叉，不追求 12 个子命令对齐。

## 快速开始

### 安装（macOS 12+）

**源码**（需 Rust 1.97+）：

```bash
git clone https://github.com/wukongnotnull/vole.git
cd vole
./install.sh
export PATH="$HOME/.local/bin:$PATH"
export VOLE_RULES_DIR="$HOME/.local/share/vole/rules"
```

**预编译包**（GitHub Release [v0.0.2](https://github.com/wukongnotnull/vole/releases/tag/v0.0.2)，ad-hoc 未公证）：

```bash
curl -LO https://github.com/wukongnotnull/vole/releases/download/v0.0.2/vole-0.0.2-aarch64-apple-darwin.tar.gz
tar xzf vole-0.0.2-aarch64-apple-darwin.tar.gz
install -m 755 vole-0.0.2-aarch64-apple-darwin/bin/vole ~/.local/bin/vole
mkdir -p ~/.local/share/vole && cp -R vole-0.0.2-aarch64-apple-darwin/share/vole/rules ~/.local/share/vole/
export VOLE_RULES_DIR="$HOME/.local/share/vole/rules"
```

Intel 将 `aarch64-apple-darwin` 换为 `x86_64-apple-darwin`。Gatekeeper 拦截：`xattr -cr ~/.local/bin/vole`。

**Homebrew（草稿）**：`brew install --HEAD ./HomebrewFormula/vole.rb`

### 使用

```bash
# 交互菜单
vole

# 系统状态 / 磁盘分析 / 清理预览与执行 / 操作历史
vole status
vole status --json
vole analyze
vole analyze --json ~/Library
vole clean --plan
vole clean --apply <plan.json>
vole clean --whitelist
vole history
vole history --json

# Shell 补全
vole completions zsh > ~/.zfunc/_vole
```

本地一键验证（CI 门禁 + release 构建 + 子系统脚本）：

```bash
bash scripts/verify-local.sh
# 跳过交叉编译以加快迭代
VERIFY_LOCAL_SKIP_CROSS=1 bash scripts/verify-local.sh
```

签名 / Homebrew 发布见 [`docs/findings/2026-07-phase5-signing.md`](docs/findings/2026-07-phase5-signing.md)。

## 命令一览

| 命令 | 能力 |
|---|---|
| `vole` / 无参 | 轻量交互菜单 |
| `vole status` | 实时健康面板；`--json` / `--json-stream` |
| `vole analyze [path]` | 目录磁盘下钻（默认 `$HOME`）；`--json` |
| `vole clean` | `--plan` / `--apply`、`--json-stream`、`--whitelist`；默认废纸篓，`--permanent` 永久删除 |
| `vole history` | 操作历史；`--json` / `--limit` |
| `vole completions` | 生成 shell 补全 |

协议说明（已冻结）：[`docs/protocol.md`](docs/protocol.md)。

## 当前范围与状态

v1 聚焦高频三命令 + `history`。`uninstall` / `optimize` / `purge` / `installer` 等不在范围内——需要它们时请用 Mole；`clean` 的 plan 报告也会提示未移植类别。

| 阶段 | 状态 |
|---|---|
| Phase 1 基础设施（协议、oplog、互斥） | 已落地 |
| Phase 2 `status` | 可用 |
| Phase 3 `analyze` | 可用（目录模式） |
| Phase 4 `clean` | 可用；规则 **470** 条（Phase 4c+ Batch 13） |
| Phase 5 `history` + 协议冻结 + 菜单 / 补全 | 可用；**v0.0.7** ad-hoc Release（470 规则） |

设计与计划见 `docs/wukong-code/`；规则收官总结见 [`docs/findings/2026-07-phase4c-v1-summary.md`](docs/findings/2026-07-phase4c-v1-summary.md)（v1）与 [`docs/findings/2026-07-phase4c-plus-summary.md`](docs/findings/2026-07-phase4c-plus-summary.md)（Batch 6–13 / 470 规则）。

## 许可证

GPL-3.0。因为 Vole 是 GPL-3.0 作品的衍生作品，这是唯一可选的许可证——包括未来的桌面 app 在内，本项目的所有部分都以 GPL-3.0 发布。详见 [LICENSE](LICENSE)。
