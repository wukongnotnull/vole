<div align="center">

<img src="images/vole.webp" alt="Vole mascot" width="180" />

# Vole

**用 Rust 实现的 macOS 清理与监控 CLI**  
单一二进制 · 类型安全 · 默认可恢复 · 协议可编排

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/github/v/tag/wukongnotnull/vole?label=version)](https://github.com/wukongnotnull/vole/releases)
[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-black.svg)](https://github.com/wukongnotnull/vole)
[![Rust](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](https://www.rust-lang.org)
[![Stars](https://img.shields.io/github/stars/wukongnotnull/vole?style=social)](https://github.com/wukongnotnull/vole/stargazers)

</div>

> 清理规则知识、路径保护清单与安全校验语义源自 [tw93/Mole](https://github.com/tw93/Mole) v1.48.1。感谢 Mole 作者与贡献者多年积累。Vole 是独立衍生项目，不隶属于 Mole。

---

**快捷导航**
[特性](#特性) · [快速开始](#快速开始) · [安全设计](#安全设计) · [使用提示](#使用提示) · [功能详解](#功能详解) · [与 Mole 对比](#与-mole-对比) · [仓库结构](#仓库结构) · [相关项目：vole-macos](#相关项目vole-macos) · [关于我](#关于我) · [许可证](#许可证)

---

## 特性

- **单一 Rust 二进制**：不依赖 bash 运行时，也不要求用户额外安装 `fd` / `jq` / `sqlite3`
- **plan → apply 两阶段**：先预览候选，再按不可信 plan 重新过安全闸口；默认进废纸篓
- **智能卸载**：移除应用本体 + 用户域/系统残留（含 LaunchDaemons/`/Library` sudo 主路径）
- **系统优化**：缓存重建、偏好修复、LaunchServices 等有界维护任务
- **磁盘洞察**：目录体积下钻，硬链接去重、折叠目录、`jwalk` 并行遍历
- **实时监控**：CPU / 内存 / 磁盘等健康面板；`--json` / `--json-stream` 可脚本化
- **冻结 NDJSON 协议**：CLI、脚本与未来桌面 app 共用同一编排层
- **规则数据化**：**536** 条高价值清理规则以 TOML 声明，可 diff、可禁用、可 fixture 回归（含用户域 orphaned app data）

适合：想要更现代、更可脚本化、更偏「安全预览再执行」的日常清理与磁盘洞察。若你需要成熟全家桶（`purge` / `installer` / 交互提权 / 完整 `/Library` 扫描），请继续用 [Mole](https://github.com/tw93/Mole)。

---

## 快速开始

### 安装（macOS 12+）

**Homebrew（推荐）**

```bash
brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
brew install vole
```

规则随 formula 装到 `$(brew --prefix vole)/share/vole/rules`，一般无需设置 `VOLE_RULES_DIR`。

源码 HEAD：`brew install --HEAD wukongnotnull/vole/vole`

**预编译包**（[v1.6.0](https://github.com/wukongnotnull/vole/releases/tag/v1.6.0)，Developer ID 签名 + 公证）

```bash
# Apple Silicon；Intel 将 aarch64 换为 x86_64
curl -LO https://github.com/wukongnotnull/vole/releases/download/v1.6.0/vole-1.6.0-aarch64-apple-darwin.tar.gz
tar xzf vole-1.6.0-aarch64-apple-darwin.tar.gz
install -m 755 vole-1.6.0-aarch64-apple-darwin/bin/vole ~/.local/bin/vole
mkdir -p ~/.local/share/vole && cp -R vole-1.6.0-aarch64-apple-darwin/share/vole/rules ~/.local/share/vole/
```

保持 `bin` + `share/vole/rules` 相对布局即可；自定义规则目录时再设 `VOLE_RULES_DIR`。

**源码构建**（需 Rust 1.97+）

```bash
git clone https://github.com/wukongnotnull/vole.git
cd vole
./install.sh
export PATH="$HOME/.local/bin:$PATH"
```

> 暂未进 Homebrew Core（知名度未达标）；用自建 tap 即可一键安装。

### 运行

```bash
vole                           # 交互菜单
vole status                    # 实时健康面板
vole analyze                   # 目录磁盘下钻（默认 $HOME）
vole clean --plan              # 清理预览（默认行为）
vole uninstall --plan          # 卸载预览
vole optimize --plan           # 系统优化预览
vole history                   # 操作历史

vole completions zsh > ~/.zfunc/_vole
vole --help
vole --version
```

### 安全预览

```bash
# 只产出候选，不改动任何文件
vole clean --plan
vole clean --dry-run           # 同 --plan
vole uninstall --plan
vole optimize --plan

# 确认后再执行
vole clean --apply <plan.json>
vole uninstall --apply <plan.json>
vole optimize --apply <plan.json>

# 白名单 / 历史 / 机器可读
vole clean --whitelist
vole history
vole history --json
vole status --json
vole analyze --json ~/Library
```

默认进废纸篓；需要永久删除时加 `--permanent`（仅与 `--apply` 联用）。

---

## 安全设计

Vole 是本地系统维护工具，部分命令会执行破坏性文件操作。

安全优先的默认行为：

| 机制 | 说明 |
|------|------|
| **路径校验** | 删除前重新过保护清单与白名单 |
| **plan → apply** | apply 视 plan 为不可信输入，TTL + TOCTOU 重验 |
| **默认可恢复** | 进废纸篓，而非直接永久删除 |
| **口径诚实** | 报告区分 `trashed_bytes` / `deleted_bytes` |
| **操作日志** | 可用 `vole history` 审计 |

高风险或不确定时，Vole 会跳过、拒绝或要求更强确认，而不是扩大删除范围。

协议说明（已冻结）：[`docs/protocol.md`](docs/protocol.md)。

---

## 使用提示

- **先预览再执行**：`clean` / `uninstall` / `optimize` 默认 `--plan`；确认后再 `--apply`
- **已卸载 vs 仍安装**：应用已卸干净用 `vole clean`；仍装着用 `vole uninstall`
- **白名单持久化**：`vole clean --whitelist` 的选择会写入配置，后续扫描自动跳过
- **自动化**：`--json` / `--json-stream` 对齐 Mole 同名字段口径；详见协议文档
- **本地验证**：`bash scripts/verify-local.sh`（跳过交叉编译：`VERIFY_LOCAL_SKIP_CROSS=1`）

---

## 功能详解

### 深度清理

```bash
$ vole clean --plan

# 扫描缓存、日志、残留与孤儿应用数据
# 产出 plan.json（候选集 + 体积），不改动文件

$ vole clean --apply plan.json

# 默认移入废纸篓；--permanent 才永久删除
# 报告区分 trashed_bytes / deleted_bytes
```

**536** 条高价值规则以 TOML 声明，覆盖浏览器缓存、开发工具、应用残留与用户域 orphaned 数据等常见目标。

### 智能卸载

```bash
$ vole uninstall --plan
# 或按名称 / bundle id 过滤
$ vole uninstall --plan "Some App"

$ vole uninstall --apply uninstall-plan.json
```

移除应用本体 + 用户域残留（Application Support、Caches、Preferences、LaunchAgents 等），以及可读的系统 LaunchDaemons/Agents/PHT 与窄 `/Library` 叶（需 `sudo -n`；TTY 可先 `sudo -v`）。`rule_id` 前缀为 `uninstall:` / `uninstall:leftover:` / `uninstall:system-leftover:`——勿用 `vole clean --apply` 执行卸载 plan。

### 系统优化

```bash
$ vole optimize --plan
$ vole optimize --apply optimize-plan.json
```

18 项主路径（含无 sudo 缓存/saved state/坏 prefs/quarantine/sqlite/Dock/LaunchServices 等，以及需 `sudo -n` 的 DNS、`memory_pressure_relief`、`network_stack_optimize`、`disk_permissions_repair`、`periodic_maintenance`）。TTY 下可至多一次 `sudo -v` 缓存凭证。其余 optimize 长尾（spotlight* / disk_verify / login_items / shared_file_list）与桌面 Helper 会诚实跳过，并写入 plan 的 `coverage_note`。

### 磁盘分析

```bash
$ vole analyze
$ vole analyze ~/Library
$ vole analyze --json ~/Documents
```

目录体积下钻；默认从 `$HOME` 起步。适合找出「到底谁占了空间」。

### 实时状态

```bash
$ vole status
$ vole status --json
$ vole status --json-stream
```

健康面板 + 机器可读输出；`--json-stream` 对齐 mole `--watch` 风格的连续 NDJSON。

### 操作历史与补全

```bash
$ vole history
$ vole history --json --limit 50
$ vole completions zsh > ~/.zfunc/_vole
```

---

## 与 Mole 对比

两者共享同一套安全语义基因（保护路径、白名单、操作日志），但定位不同：

| | **Vole** | **Mole** |
|---|---|---|
| 实现 | 纯 Rust 单一二进制 | Bash + Go 混合 |
| 成熟度 | **1.39.0**：Antigravity browser Cache + optimize W2b③（network/disk/periodic）+ Zed npm cache + memory_pressure_relief + uninstall 系统 LaunchDaemons/`/Library` sudo 主路径 + Login Items + brew cask + Filo + optimize DNS/mDNS + 本地快照报告 + TM 失败备份 + system.sh 主链；余项：Mole 广谱 `/Library` 边缘 / optimize spotlight* 等长尾 / 桌面 Helper | 成熟、功能最全 |
| 核心命令 | `status` / `analyze` / `clean` / `history` / `uninstall` / `optimize` | 另有 `purge` / `installer` 等 |
| 清理模型 | `--plan` / `--apply` 两阶段 + 默认废纸篓；orphaned 启发式 | `--dry-run` 预览 + 深度清理流水线 |
| 机器可读输出 | Mole 兼容 JSON **子集** + 自有 NDJSON 事件流 | `--json`（status / analyze / history） |
| 外部依赖 | 无第三方 CLI 依赖 | 部分场景推荐 `fd` 等 |
| 规则规模 | **536** 条高价值规则 | 全量数百条 `safe_clean` 目标 |
| 桌面端路线 | [vole-macos](https://github.com/wukongnotnull/vole-macos) Clean MVP（内嵌 sidecar） | 另有商业 [Mole for Mac](https://mole.fit) |
| 许可证 | GPL-3.0 | GPL-3.0 |

---

## 仓库结构

```
vole/
├── crates/
│   ├── vole-cli/          # CLI 入口与子命令
│   ├── vole-core/         # 清理 / 卸载 / 优化编排
│   ├── vole-sys/          # macOS 系统调用（仅 darwin）
│   └── vole-proto/        # 冻结 NDJSON / Plan / Report
├── data/rules/            # 536 条 TOML 清理规则
├── conformance/           # mole ↔ vole 对照 harness
├── Formula/               # Homebrew tap formula
├── scripts/               # 校验、发布、本地 verify
├── docs/
│   ├── protocol.md        # 协议说明（已冻结）
│   ├── releases/          # 发版说明
│   └── wukong-code/       # 设计与计划
└── third_party/mole-1.48.1/  # 知识底座参考快照
```

## 相关项目：vole-macos

[vole-macos](https://github.com/wukongnotnull/vole-macos) 是配套的 macOS SwiftUI 图形客户端，消费本仓冻结协议，内嵌 `vole` sidecar。

当前里程碑：**Clean MVP**（plan → 勾选 → apply，默认废纸篓）。尚未覆盖 `uninstall` / `optimize` / `status` 等命令的完整桌面流。

- 原生 SwiftUI，本地运行；需授予完全磁盘访问（FDA）
- 与本仓共用 Plan / Report / NDJSON 事件与操作日志口径
- 非 App Sandbox（开发期）；特权删仅 `sudo -n`（TTY 可先 `sudo -v`）；不宣称常驻 root / SMAppService

详细说明见 [vole-macos 仓库](https://github.com/wukongnotnull/vole-macos)。

---

## 关于我

**悟空非空也** — AI之道创始人，独立开发者，Up主。

| 平台 | 链接 |
|------|------|
| 🌐 官网 | [AI之道官网](https://waytoai.cn) |
| 𝕏 Twitter | [悟空非空也](https://x.com/wukongnotnull) |
| 📺 B站 | [悟空非空也](https://space.bilibili.com/456634391) |
| ▶️ YouTube | [悟空非空也](https://www.youtube.com/@wukongnotnull) |
| 📕 小红书 | [悟空非空也](https://www.xiaohongshu.com/user/profile/5ca89c2f000000001100952b) |
| 💬 公众号 | 微信搜「悟空非空也」 |

---

## 许可证

Vole 遵循 GPL-3.0 协议，属于 GPL-3.0 授权作品的衍生项目，详细许可信息请参见 [LICENSE](LICENSE)。
如需 fork 并开发自有产品，请更换名称以避免混淆，并注明来源于 Mole / Vole。

---

<div align="center">

GPL-3.0 license © [悟空非空也](https://github.com/wukongnotnull)

</div>
