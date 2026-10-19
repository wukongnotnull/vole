# Vole 产品需求文档（PRD）

| 项 | 内容 |
|---|---|
| 产品名称 | Vole（命令行版 `vole`）／ Vole for macOS（桌面版） |
| 文档版本 | v1.5（grilling 修订：逐条对照两仓实现审视） |
| 修订日期 | 2026-10-19 |
| 对应代码基线 | `vole` main @ `1566521`（`Cargo.toml` 2.20.0；比最近发版 tag `v2.20.0` @ `46bbe3c` 多 8 个已合并 PR（共 16 个**未发版**提交））；`vole-macos` main @ `414a6bd`（0.3.0；Release DMG 内嵌按 tag `v2.20.0` 构建的 sidecar） |
| 产品负责人 | 悟空非空也 |
| 文档状态 | 待评审；含 8 项待拍板事项（D1–D8，见下节） |
| 适用读者 | 产品、研发、测试、发布与文档维护者 |

## 修订记录

| 版本 | 日期 | 修订人 | 说明 |
|---|---|---|---|
| v1.5 | 2026-10-19 | 悟空非空也（Claude 协助整理） | grilling 修订。基线区分「已发版 / 已合入 main 未发版」；5.1 增加交付状态列；5.2 与 6.2 写清两端的多选与默认勾选矩阵、永久删除开关范围；F17 按实现重写并记录 Helper 旁路删除漏斗、无审计日志、整批计成功的缺口；修正规则目录发现方式、FDA 探测时机、桌面无 PR CI 与 UI 测试现状、CLI 人读输出中英混用、退出码契约、`update --check`；13.2 改为按 issue 列出的待办；新增「待拍板事项」、附录 14.4（发现与处理）、14.5（环境变量） |
| v1.4 | 2026-10-18 | 悟空非空也（Claude 协助整理） | F09 补齐 Mole 对齐：候选标出来源（Downloads / Homebrew / iCloud 等）、Homebrew 哈希前缀只在 Homebrew 缓存中去除、apply 前校验大小未变；确认 fd / find 两条扫描分支已由同一实现覆盖，去掉 `coverage_note` 中的长尾说明 |
| v1.3 | 2026-10-18 | 悟空非空也（Claude 协助整理） | F04 补齐 Mole 前置诊断：系统概要行、性能诊断（syspolicyd 时可卸载用户 DMG）；`--task` 去掉实验性标注；disk_verify 未开启时在 `coverage_note` 说明 |
| v1.2 | 2026-10-18 | 悟空非空也（Claude 协助整理） | F02 补齐 Developer 大户长尾：DerivedData 整树、DeviceSupport 保留最新 2 版、不可用模拟器经 simctl 删除；规则数更新为约 550 |
| v1.1 | 2026-10-18 | 悟空非空也（Claude 协助整理） | 附录 14.2 三处差异定为「以代码为准」，同步修订 v3 规格与桌面版 README；修正吉祥物动画表述 |
| v1.0 | 2026-10-18 | 悟空非空也（Claude 协助整理） | 首份汇总 PRD。依据两个仓库的源码、README、`docs/wukong-code/specs` 设计稿与 `docs/releases` 发版记录整理，描述截至 CLI 2.20.0 / 桌面 0.3.0 已交付的产品能力 |

## 待拍板事项（v1.5）

> 以下问题 PRD 不能自行决定，各附推荐方案。拍板后由对应线程落实，并把结论回写本节与附录 14.4。

| 编号 | 问题 | 现状 | 推荐 |
|---|---|---|---|
| D1 | CLI 是否发 2.21.0 | main 比 `v2.20.0` 多 8 个已合并 PR（16 个提交）：F02 Developer 长尾、F04 前置诊断、F09 安装包对齐三项用户可见能力，以及 Worktree 误跳过（PR #168）、clean 历史会话标记（PR #169）两处修复都未发版，Homebrew / Release 用户拿不到。按 SemVer 规则（README：下一 MINOR 由 main 上已有的用户可见工作触发，是否打 tag 另行确认）已满足发版条件 | **发 2.21.0**：bump workspace version → tag → assets → Formula → 五语 README；本 PRD 基线随之改为 2.21.0 |
| D2 | 桌面是否出 0.4.0 | Release 流水线把 sidecar 钉在 tag `v2.20.0`，0.3.0 DMG 不含 PR #168 / #169 修复（分别影响桌面 Worktree 执行与「历史」显示）；桌面另有 issue #32（卸载默认全选）、#34（Helper 报完成未删除）两个安全 / 诚实性缺陷未修 | **CLI 2.21.0 发布后立即排期 0.4.0**：sidecar 钉 2.21.0，同批修 #32、#34、#35，合并 PR #33 |
| D3 | sidecar 版本门控规则 | 代码只接受「主.次 == 2.20」（`SidecarVersion.supportsWorktreeAgent`）；一旦内嵌 2.21.x，Worktree / Agent 会被标为「需要内嵌 vole 2.20」而不可用 | 改为 **`>= 2.20.0`**（NDJSON `schema_version` 冻结为 1，向后兼容），门控只拒绝过旧或解析失败的 sidecar；之后每次升 sidecar 不再改门控 |
| D4 | 桌面默认勾选 | 卸载扫描后默认全选（issue #32：188 个 App 全选，回车即卸载全部）；优化默认全选（含 Dock 刷新、LaunchServices 重建等动作）。CLI 卸载默认全不选 | **卸载改为默认全不选**（对齐 CLI，P0）；优化维持全选（与 CLI「确认即全部执行」语义一致），但主按钮不再绑定回车默认动作；清理 / 净化 / 安装包维持现状 |
| D5 | 桌面「需 root」路由依据 | 桌面按路径前缀白名单（`PathAuthorization.allowedPrefixes`，含 `/private/tmp`、`/private/var/tmp`）判定条目走 Helper；用户可写的临时目录条目因此被送去 root 永久删除，issue #34 显示该通道报「完成」却未删除 | 短期：把 `/private/tmp`、`/private/var/tmp` 从桌面侧「需 root」判定中移除（这类条目交 sidecar `--apply`，无 sudo 时以 `needs_privilege` 诚实跳过）；中期：由 sidecar 在 Plan 条目上标出需提权（追加可选字段，不破坏协议），桌面只对带标记的条目走 Helper，路径白名单退为 Helper 侧第二道防线 |
| D6 | CLI 人读输出语言 | PRD 原写「英文为主」，实际 clean 计划摘要说明、apply 报告「移入废纸篓 / 永久删除」两行、权限警告、`coverage_note` 全是中文；PR #152（8 月 15 日起挂起）只把摘要两行改成英文 | **CLI 人读输出统一英文**：rebase 并合并 PR #152，其余中文行另开 issue 一次改完；`coverage_note` 暂保留中文（桌面 UI 直接展示），在 `docs/protocol.md` 注明它是说明性文本、不构成字段契约 |
| D7 | apply 退出码契约 | 只有 `installer` 在有失败条目时退出码为 1；clean / uninstall / optimize / purge / worktree / agent 即使 `failed > 0` 也返回 0（只在 I/O 错误时返回 1，Ctrl+C 返回 130） | 统一为 **`failed > 0` → 退出码 1**，对齐 installer；已开 issue #170 |
| D8 | Helper 删除的审计落点 | Helper 用 `FileManager.removeItem` 永久删除，不写操作日志与删除日志，「历史」看不到；成功数按整批条目数计，路径不存在也算成功（issue #35） | **方案 A**：App 在 Helper 返回后调用 sidecar 补写操作 / 删除日志并逐项记录结果（history 单一数据源）；备选方案 B：Helper 自写 os_log + 文件日志。推荐 A |

## 目录

1. [产品概述](#1-产品概述)
2. [目标用户与使用场景](#2-目标用户与使用场景)
3. [产品目标与成功指标](#3-产品目标与成功指标)
4. [产品形态与架构](#4-产品形态与架构)
5. [功能需求](#5-功能需求)
6. [交互与界面需求](#6-交互与界面需求)
7. [数据、协议与配置](#7-数据协议与配置)
8. [非功能需求](#8-非功能需求)
9. [权限与安全模型](#9-权限与安全模型)
10. [分发、版本与发布](#10-分发版本与发布)
11. [范围约束与明确不做](#11-范围约束与明确不做)
12. [风险与应对](#12-风险与应对)
13. [版本规划与里程碑](#13-版本规划与里程碑)
14. [附录](#14-附录)

---

## 1. 产品概述

### 1.1 背景

macOS 用户的磁盘会被缓存、日志、卸载残留、安装包、陈旧构建产物、遗留 Git worktree 与 AI Agent 工作区残留持续占满。市面上的方案大致两类：

- 商业图形清理工具（如 CleanMyMac、腾讯柠檬清理）：体验成熟，但规则不透明，常见「一键全删」。
- 开源脚本工具 [Mole](https://github.com/tw93/Mole)：规则透明、安全思路扎实，但主体是约 3.5 万行 Bash，依赖外部命令，难以单测与扩展，也不便承载图形前端。

Vole 以 Mole v1.48.1 为知识来源与正确性基准，用 Rust 重写为单一二进制，并在此基础上补充自有能力（Worktree、Agent 清理）和 SwiftUI 桌面版。

### 1.2 产品定位

> 给 macOS 用的清理与监控工具：**先看再清 · 默认进废纸篓 · 一个命令装好就能用**。

- 安全优先：先列候选、再由用户确认；默认移入废纸篓，可恢复。
- 透明可追溯：每次操作写日志，可用「历史」回看。
- 一套引擎，两种入口：命令行（含交互式 TUI）与桌面窗口共用同一引擎与安全语义。

### 1.3 产品组成

| 组成 | 仓库 | 技术栈 | 已发版版本 | main 现状 | 形态 |
|---|---|---|---|---|---|
| Vole CLI | `wukongnotnull/vole` | Rust workspace（`vole-cli` / `vole-core` / `vole-proto` / `vole-sys`） | 2.20.0（2026-09-03） | 仍标 2.20.0，含未发版能力与修复（见 13.1） | 单一二进制 + 规则数据；交互式 TUI 首页 |
| Vole for macOS | `wukongnotnull/vole-macos` | SwiftUI + SMAppService 特权助手 | 0.3.0（2026-09-03） | 与发版一致（仅 README 改动） | Universal DMG；内嵌 `vole` sidecar |

### 1.4 与 Mole 的关系

- 清理规则、保护清单与安全校验语义源自 Mole v1.48.1（钉版 vendor 于 `third_party/mole-1.48.1/`，只做一致性比对基准，不参与构建，不跟随上游演进）。
- Vole 是独立开源项目，不隶属于 Mole；按 GPL-3.0 发布，并在 README 注明来源。
- 下列三类必须与 Mole 严格一致：删除前的路径校验与保护清单；`--json` 中与 Mole 同名字段的口径；操作日志格式。

---

## 2. 目标用户与使用场景

### 2.1 用户画像

| 画像 | 特征 | 主要入口 | 核心诉求 |
|---|---|---|---|
| **P1 开发者 / 终端用户** | 熟悉终端，机器上有大量项目、构建物、依赖缓存，常用 Cursor / Codex / Claude 等 AI 编程工具 | CLI / TUI | 快速找出大件并安全清理；可脚本化（JSON、退出码） |
| **P2 普通 Mac 用户** | 不常用终端，磁盘告急时想清理 | 桌面版 | 图形界面勾选清理，删错能找回 |
| **P3 Mole 迁移用户** | 已在用 Mole，有依赖其 JSON 或日志的脚本 | CLI | 平滑迁移，字段与日志兼容 |
| **P4 前端集成方** | 桌面 App 或第三方前端 | NDJSON 协议 | 稳定、有版本治理的事件流与 Plan 格式 |

### 2.2 典型场景

| 编号 | 场景 | 对应功能 |
|---|---|---|
| S1 | 磁盘快满，想知道谁占了空间 | 分析（analyze） |
| S2 | 清理系统 / 应用缓存与日志，又怕误删 | 清理（clean）+ 预览 + 废纸篓 |
| S3 | 卸掉不用的 App 并清掉残留 | 卸载（uninstall） |
| S4 | 机器变慢，想做一轮安全的系统维护 | 优化（optimize） |
| S5 | 清理几十个旧项目的 `node_modules` / `target` 等构建物 | 净化（purge） |
| S6 | 清理下载目录里落灰的 `.dmg` / `.pkg` | 安装包（installer） |
| S7 | 清理 AI 编程工具留下的遗留 Git worktree | Worktree |
| S8 | 清理 Cursor / Codex / Claude 的会话、缓存与容器残留 | Agent |
| S9 | 查看机器实时健康状况 | 状态（status） |
| S10 | 回看之前删了什么 | 历史（history） |
| S11 | 让 `sudo` 支持 Touch ID、升级或卸载 Vole 本身 | touchid / update / remove |

---

## 3. 产品目标与成功指标

### 3.1 产品目标

1. **安全**：任何删除都经过预览、确认、保护层校验；默认可恢复。
2. **覆盖**：命令面覆盖 Mole「日常全家桶」，并提供 Worktree、Agent 等自有增值能力。
3. **自包含**：不依赖 bash 运行时，不要求用户额外安装第三方工具（`fd`、`sqlite3`、`jq` 等）；只调用 macOS 自带命令。
4. **可集成**：编排逻辑与前端解耦，CLI 与桌面都是薄前端，通过稳定协议通信。
5. **可找到、可安装**：Homebrew tap 与签名公证的预编译包，五语 README。

### 3.2 成功标准（定性，已作为各代际验收）

| 维度 | 标准 | 现状缺口（v1.5 核对） |
|---|---|---|
| 能力完整 | Mole 对齐命令面全部落地（`scripts/check-command-surface.sh --enforce` 通过） | 已满足；F02 / F04 / F09 的补齐部分尚未发版（D1） |
| 安全 | 无「未经确认即删除」路径；所有删除走同一删除漏斗；保护层不可绕过 | 桌面 Helper 通道不经删除漏斗（F17、NFR-S1 例外、issue #35） |
| 诚实 | 未覆盖、跳过、需提权的项目响亮告知，不假装已处理；UI 与 JSON 不出现「可安全删除」类承诺（Worktree / Agent） | 桌面 Helper 批次报「完成」却可能未删除（issue #34）；除 installer 外的命令有失败条目时退出码仍为 0（D7） |
| 一致 | CLI 与桌面使用同一引擎、同一 Plan 协议 | 已满足；两端默认勾选不一致（6.2、D4） |
| 质量 | CLI：CI（macOS runner）fmt / clippy / test / 守卫脚本全绿后才合并。桌面：合并前本机 `xcodebuild -scheme vole-macos test` 全绿并经代码评审 | 桌面仓无 PR CI；UI 测试 target 未接入 scheme（issue #31，PR #33 草稿） |

### 3.3 建议量化指标（待埋点能力确认后启用）

产品默认不联网、不上报，以下指标只能来自公开渠道：

| 指标 | 数据来源 |
|---|---|
| Release 下载量（CLI / 桌面） | GitHub Releases 下载计数 |
| Star / Issue 数 | GitHub 仓库 |
| 误删相关缺陷数 | GitHub Issues 标签统计 |
| 版本采纳 | Homebrew tap 安装与 `vole update` 使用反馈（无遥测，仅定性） |

---

## 4. 产品形态与架构

### 4.1 系统架构

```
┌──────────────────────┐        ┌───────────────────────────────┐
│  vole（CLI / TUI）    │        │  Vole for macOS（SwiftUI）      │
│  vole-cli：薄前端     │        │  侧栏十项 · Plan 五态            │
└─────────┬────────────┘        └──────┬───────────────┬────────┘
          │ 直接调用                    │ 子进程 + NDJSON  │ 特权 XPC
          ▼                            ▼                 ▼
┌──────────────────────────────────────────┐   ┌──────────────────────┐
│ vole-core：编排（ops）、规则、保护层、删除漏斗 │   │ VolePrivilegedHelper  │
│ vole-proto：Plan / 事件 / Report 协议类型  │   │ （SMAppService 守护进程）│
│ vole-sys：macOS 平台原语（废纸篓、plist、IOKit）│ │ 白名单前缀内永久删除    │
└──────────────────────────────────────────┘   │ / launchctl bootout   │
                                               │ ※ 不经 vole-core 漏斗  │
                                               └──────────────────────┘
```

- **依赖方向**：`vole-cli → vole-core → vole-proto / vole-sys`，由 `scripts/check-dep-direction.sh` 守卫。
- **平台边界**：`vole-sys` 以 trait 隔离平台能力；非 macOS target 直接 `compile_error!`。
- **桌面与 CLI 边界**：桌面通过进程边界调用内嵌 `vole` sidecar（`--plan --json-stream --plan-out` → `--apply`），不链接 Rust 库。
- **第二条执行路径（现状）**：桌面把「需 root」条目交给 Helper，由 Helper 以 `FileManager.removeItem` 永久删除。它绕过 vole-core 的保护层、身份复核与日志，是 NFR-S1 的已知例外（见 5.16、8.1、D5 / D8）。

### 4.2 两阶段模型（Plan → Apply）

所有破坏性命令遵循同一模型：

1. **Plan**：扫描并生成候选清单（Plan 文件），每条记录路径、大小、规则、文件身份（`dev` / `ino` / `mtime`）。Plan 条目**没有**「需提权」标记；桌面按路径前缀自行判定（见 D5）。
2. **确认**：TTY 下确认（部分命令先分页多选），或桌面勾选，或由脚本审阅 Plan 文件。
3. **Apply**：校验 Plan 的 TTL（默认 900 秒）与每条条目的身份（TOCTOU 复核），通过后经删除漏斗执行。

---

## 5. 功能需求

### 5.1 功能清单总览

优先级：P0 = 核心主路径；P1 = 重要能力；P2 = 辅助能力。交付状态：「已发版」指 CLI v2.20.0 / 桌面 0.3.0 已包含；「未发版」指已合入 main 但尚未打 tag（见 D1）。桌面一列标示桌面版是否提供对应入口。

| 编号 | 功能 | CLI 命令 | 优先级 | CLI | 桌面 | 来源 | 交付状态 |
|---|---|---|---|---|---|---|---|
| F01 | 交互式首页 | `vole` | P0 | ✓ | 侧栏替代 | Vole | 已发版 |
| F02 | 清理 | `vole clean` | P0 | ✓ | ✓ | Mole 对齐 | 已发版；Developer 长尾（第 5 点）未发版；DeviceSupport 规则有缺陷（issue #171） |
| F03 | 卸载 | `vole uninstall` | P0 | ✓ | ✓ | Mole 对齐 | 已发版；桌面默认全选待改（D4） |
| F04 | 优化 | `vole optimize`（别名 `optimise`） | P1 | ✓ | ✓ | Mole 对齐 | 已发版；前置诊断未发版 |
| F05 | 分析 | `vole analyze`（别名 `analyse`） | P0 | ✓ | ✓（只读） | Mole 对齐 | 已发版 |
| F06 | 状态 | `vole status` | P1 | ✓ | ✓ | Mole 对齐 | 已发版 |
| F07 | 历史 | `vole history` | P1 | ✓ | ✓ | Mole 对齐 | 已发版；clean `--apply` 会话标记修复（PR #169）未发版 |
| F08 | 净化 | `vole purge` | P1 | ✓ | ✓ | Mole 对齐 | 已发版 |
| F09 | 安装包 | `vole installer` | P1 | ✓ | ✓ | Mole 对齐 | 已发版；来源列 / 大小复核等对齐补齐未发版 |
| F10 | Worktree | `vole worktree` | P1 | ✓ | ✓ | Vole 自有 | 已发版；agent-container worktree 误跳过修复（PR #168）未发版 |
| F11 | Agent | `vole agent` | P1 | ✓ | ✓ | Vole 自有 | 已发版 |
| F12 | 白名单 | `clean --whitelist*` / `optimize --whitelist*` | P2 | ✓ | — | Mole 对齐 | 已发版 |
| F13 | Touch ID for sudo | `vole touchid status/enable/disable` | P2 | ✓ | 设置页 | Mole 对齐 | 已发版 |
| F14 | 自更新 | `vole update` | P2 | ✓ | 设置页 | Mole 对齐 | 已发版 |
| F15 | 卸载 Vole 自身 | `vole remove` | P2 | ✓ | 设置页 | Mole 对齐 | 已发版 |
| F16 | Shell 补全 | `vole completions`（别名 `completion`） | P2 | ✓ | — | Vole | 已发版 |
| F17 | 特权助手 | — | P1 | — | ✓ | 桌面自有 | 已发版；审计与逐项结果缺口（issue #34 / #35，D5 / D8） |
| F18 | 完全磁盘访问引导 | — | P1 | 提示 | ✓ | 桌面自有 | 已发版 |

### 5.2 通用交互契约（适用于 F02–F04、F08–F11）

| 调用方式 | 行为 |
|---|---|
| TTY 下不带参数（clean / optimize） | 扫描 → 分组展示**全部**候选 → `Proceed with <cmd>? [y/N]`（默认否）→ 全部执行。不提供逐项多选，排除个别路径或任务用白名单（F12） |
| TTY 下不带参数（uninstall / purge / installer / worktree / agent） | 扫描 → 分页多选（默认勾选见 6.2 矩阵）→ `Proceed with …? [y/N]`（默认否）→ 执行 |
| 非 TTY，或带 `--plan` / `--dry-run` / `-n` / `--json` / `--json-stream` / `--plan-out` | 只输出候选，不做任何修改 |
| `--apply <PLAN>` | 读取 Plan 文件，校验 TTL 与条目身份后执行 |
| `--permanent` | 仅与 `--apply` 或交互确认合用：永久删除，不进废纸篓 |
| `--json` | 输出单个 JSON 结果 |
| `--json-stream` | 向 stdout 输出 NDJSON 事件流；人读日志只走 stderr |
| `--plan-out <FILE>` | 把 Plan 写入文件 |

验收：
- 非 TTY 环境下，任何不带 `--apply` 的调用都不得修改文件系统。
- `--dry-run` 与 `-n` 是 `--plan` 的隐藏别名，`--help` 只展示 `--plan`。
- 一个命令的 Plan 不得被另一个命令的 `--apply` 执行（以 `rule_id` 前缀区分，不匹配则逐条跳过）。
- 同一命令同一时间只允许一个实例执行（`flock` 配置级互斥锁），冲突时报错。
- 退出码：成功 0；用户中断（Ctrl+C）130；参数或 I/O 错误 1；**有失败条目时应为 1**（现状只有 installer 满足，其余命令返回 0，见 D7、issue #170）。

### 5.3 F01 交互式首页

**描述**：终端运行 `vole`（无子命令）进入 TUI 首页，用方向键或数字键选择功能。

| 键 | 项 | 说明 |
|---|---|---|
| 1 | Clean | 文案对齐 Mole |
| 2 | Uninstall | 文案对齐 Mole |
| 3 | Optimize | 文案对齐 Mole |
| 4 | Analyze | 文案对齐 Mole |
| 5 | Status | 文案对齐 Mole |
| 6 | Worktree | `Remove leftover git worktrees`（Vole 增值） |
| 7 | Agent | `Remove leftover agent data`（Vole 增值） |

需求要点：
- 品牌行显示 crate 版本号；底部 footer 显示 `H Helper`、`T TouchID`，有可用更新时显示 `U Update`。
- Enter 或数字键进入的流程与直接运行对应子命令一致。
- `purge` / `installer` 不进首页，通过子命令访问。
- footer 只展示已接线的按键，不虚标。

验证：`crates/vole-cli/src/tui/home_menu_state.rs` 单元测试锁定七项顺序、文案与 footer 组合；`crates/vole-cli/tests/interactive_cli.rs`。

### 5.4 F02 清理（clean）

**用户故事**：作为 Mac 用户，我想扫出缓存、日志和残留文件，确认后再清理，删错可以从废纸篓找回。

功能要点：
1. **规则引擎**：内置 550 条 TOML 规则（`data/rules/*.toml`；按 `category` 计：user-devtools 275、app-caches 264、orphaned / developer / ai-agents 各 3、user 与 browser 各 1），每条规则声明 `id`、`category`、`paths`（glob）、`strategy`（如 `all`、`keep_newest_by_mtime` 保留最新 N 个、按 mtime 等，部分带 `env_override`）、`impact`、`last_verified`。
2. **自定义处理器与守卫**：对需要过程逻辑的规则提供专用 handler（如进程运行中则跳过，即 process guard）。
3. **孤立应用数据**：`orphaned-app-data` 规则识别已卸载应用在用户域 Caches / Logs / Saved State 留下的数据；它本身不扫 Containers / Group Containers / LaunchAgents / `/Library/**`。容器残桩（CleanMyMac allowlist）与 Group Containers 的 logs / caches 由另外的专门规则覆盖。
4. **系统级项**：系统服务孤立项（plan 阶段只读 `/Library/LaunchDaemons`、`LaunchAgents`、`PrivilegedHelperTools` 可读子集；apply 用 `sudo -n` 永久删除，TTY 下无凭证时至多一次 `sudo -v`，否则记 `needs_privilege`）；Time Machine 失败中的备份（`*.inProgress`，48 小时安全窗，经 `tmutil delete` 删除）；本地快照只报告数量，不删除。
5. **Developer 大户**（已合入 main，未发版）：Xcode DerivedData 整树（Xcode 运行时跳过）；iOS / watchOS / tvOS DeviceSupport 按 mtime 保留最新 2 个版本（规则 `env_override = MOLE_XCODE_DEVICE_SUPPORT_KEEP`），并清理保留版本内的符号缓存与日志；不可用模拟器由 `xcrun simctl list devices unavailable` 选出，apply 前复验仍不可用，再经 `xcrun simctl delete` 永久删除，simctl 失败时不回退为直接删目录。`/Library/Developer` 下需 sudo 的系统级项（DocumentationCache 旧索引、CoreSimulator 系统缓存、模拟器运行时卷）尚未移植，在 `coverage_note` 中注明（issue #163）。**已知缺陷**：DeviceSupport 三条规则的 glob 以 `*)` 结尾，只匹配 `17.5 (21F79)` 这类目录名，匹配不到 Xcode 12 起常见的 `17.5 (21F79) arm64e`，与 Mole「列出全部子目录」不一致（issue #171）。
6. **只读提示（hints）**：plan 后附加四类只读提示：项目构建物可用 `purge` 清理、系统数据、用户 LaunchAgent 归属、孤立点目录。每个探针有超时预算（默认 15 秒，`VOLE_TIMEOUT_HINT_SCAN_SEC`），超时或失败则跳过，不阻塞 plan / apply；`--apply` 不运行 hints。
7. **覆盖说明**：Plan 携带 `coverage_note`（中文），列出已覆盖类别，末尾「请关注后续版本」列出未移植项；权限不足时响亮提示（例如引导开启完全磁盘访问）。
8. **白名单**：见 F12。

验收：
- 不存在顶层 `vole hints` 命令。
- 保护清单命中的路径不进入候选；被塞进 Plan 时 apply 拒绝执行。
- 报告区分「移入废纸篓的字节数」与「永久删除的字节数」，不使用单一 freed 字段。
- 不可用模拟器只经 `simctl delete` 删除；simctl 失败或 apply 复验不通过时目录原样保留。
- DeviceSupport 规则须选中 `<版本> (<build>)` 与 `<版本> (<build>) arm64e` 两种命名的旧版本目录（issue #171 修复后补 fixture）。

验证：`tests/fixtures/clean/*.json` 一致性 fixture；`crates/vole-cli/tests/clean_*.rs`。

### 5.5 F03 卸载（uninstall）

**用户故事**：我想卸掉某个 App，并尽量清掉它在用户域留下的残留。

功能要点：
1. 枚举已安装应用（可按 bundle id 或名称过滤）→ 套用 App 保护策略（系统关键应用不可卸载）→ 发现残留 → 生成 Plan。
2. 识别 Homebrew Cask 安装来源，按来源处理（`brew uninstall --cask` / `--zap`）。
3. 处理登录项；系统 LaunchDaemons 与 `/Library` 残留需要提权，CLI 使用 `sudo -n`（非交互，TTY 下至多一次 `sudo -v`），无权限则跳过并记 `needs_privilege`。
4. `rule_id` 前缀：`uninstall:`（应用本体）、`uninstall:leftover:`（残留）。
5. TTY 下分页选择应用（**默认全不选**）→ 确认 → 卸载。桌面版扫描后默认全选，待 D4 拍板。

验收：系统关键 bundle（如 Finder、Safari、系统设置等，见 `data/protection.toml`）不出现在可卸载列表中。

### 5.6 F04 优化（optimize）

**用户故事**：我想跑一组有边界、安全的系统维护任务。

任务清单（与 Mole `lib/optimize/catalog.sh` 对齐，共 23 项，`crates/vole-core/src/optimize/catalog.rs`）：

| 类别 | 任务 |
|---|---|
| 删除类（4 项，`optimize:delete:<id>`，默认进废纸篓） | `cache_refresh` Finder 缓存刷新、`saved_state_cleanup` App 状态清理（30 天以上）、`fix_broken_configs` 损坏配置修复、`launch_agents_cleanup` Launch Agents 清理 |
| 动作类（19 项，`optimize:action:<id>`，副作用操作） | DNS 与 Spotlight 检查、网络缓存刷新、SQLite 数据库压缩（Mail / Safari / Messages，应用运行中跳过）、LaunchServices 修复、Dock 刷新、禁止网络卷生成 `.DS_Store`、旧版覆盖审计、内存优化、网络栈刷新、权限修复、Spotlight 优化、Spotlight 孤立规则清理、周期性维护、共享文件列表修复、磁盘健康检查、登录项审计、隔离数据库清理、通知清理、使用数据清理 |

功能要点：
- **前置诊断**（已合入 main，未发版；对齐 Mole `show_system_health` 与 `run_optimize_diagnostics`）：TTY 确认轨与 TTY 下的人读 `--plan` 在列任务前先输出
  - 系统概要行：`System  已用/总 GB RAM | 已用/总 GB Disk | Uptime Nd`；
  - 性能诊断：间隔 1 秒两次采样 `ps`，按进程族（CloudShell / syspolicyd / WindowServer / Spotlight / CoreSimulator 磁盘镜像）累计 CPU，两次都 ≥ 25% 判为持续瓶颈并给出说明（阈值与间隔可由 `VOLE_OPTIMIZE_DIAG_CPU_THRESHOLD` / `VOLE_OPTIMIZE_DIAG_SAMPLE_DELAY` 调整，兼容 `MOLE_` 前缀）；
  - syspolicyd 持续高占用时再查 `spctl --status` 与 `hdiutil info`，列出挂在 `/Volumes` 下的用户镜像（系统管理镜像、受保护路径、白名单路径不列）；确认轨中询问 `Detach now? [y/N]`，确认后逐个 `hdiutil detach`，人读 `--plan` 只提示人工复核。
  - `--json` / `--json-stream` / `--plan-out` / `--apply` / `--task` 不跑诊断，Plan 结构不变。
- CLI TTY 不提供逐项勾选：确认即执行全部未被任务白名单跳过的任务；桌面版逐项勾选，扫描后默认全选（见 D4），且不提供永久删除开关。
- 需要 root 的动作（DNS 刷新、`purge`、路由刷新、权限重置等）使用 `sudo -n`；TTY 下无凭证时至多一次 `sudo -v` 请求管理员权限，仍无凭证则跳过并记为 `needs_privilege`。
- 磁盘健康检查（`disk_verify`）默认不执行（`diskutil verifyVolume /` 在 APFS 异常时可能卡死系统），须 `VOLE_ENABLE_DISK_VERIFY=1`（兼容 `MOLE_ENABLE_DISK_VERIFY`）；未开启时 Plan 的 `coverage_note` 写明如何开启。
- `--task <TASK_ID>` 单独执行一项；未知 id 报错并列出全部有效 id。
- 支持任务白名单（见 F12）。

验收：
- 非 TTY、`--json`、`--json-stream` 下不输出诊断、不运行 `hdiutil detach`。
- 只有用户在确认轨输入 `y` 后才卸载镜像；`/System`、`/Library/Apple`、CoreSimulator 卷等系统管理镜像从不作为卸载目标。
- 未开启 `disk_verify` 时 Plan 不含该任务，`coverage_note` 含开启方法。

### 5.7 F05 分析（analyze）

**用户故事**：我想看清哪个文件夹、哪些大文件最占空间。

功能要点：
- 默认从 `$HOME` 开始，可指定目录；`--json` 输出与 Mole 同名字段对齐。
- TUI：方向键 / `h` `l`（`b` 亦可返回）进出目录，`/` 过滤，`T` 大文件 Top 视图，`S` 切换实时排序模式，`O` 打开，`P` 预览，`F` 在 Finder 中显示，`R` 刷新，`Q` / `Esc` / `Ctrl+C` 退出。
- `Space` 多选 + `Delete` / `Backspace` 删除：二次确认（Enter 确认、Esc 取消）后固定移入废纸篓，经统一删除漏斗。
- 桌面版的分析只读（调用 `analyze --json <dir>`），不提供删除。

### 5.8 F06 状态（status）

**用户故事**：我想一眼看到机器健康分与 CPU、内存、磁盘等实时状况。

功能要点：
- 采集项：主机与硬件信息、运行时长、进程数、CPU、GPU、内存（含内存压力）、磁盘、废纸篓大小、磁盘 IO、网络与历史、代理、电池、温度与传感器、蓝牙、Top 进程。
- **健康分**（0–100）：CPU 30、内存 25、磁盘 20、温度 15、IO 10 为各项最大扣分权重，叠加内存压力、电池循环与容量、运行时长等扣分；85 以上为优秀，65 以上良好，45 以上一般。阈值与 Mole 对齐（`vole-core/src/status/health.rs`）。
- TUI 按键：`K` 切换 Vole 吉祥物，`C` 核心视图，`B` 返回，`Q` / `Esc` / `Ctrl+C` 退出；偏好存于 `~/.config/vole/status_prefs`（可读取 Mole 旧配置）。
- `--json` 输出单次快照；`--json-stream` 持续输出（字段集与 `mo status --watch` 对齐）。桌面版定时调用 `status --json` 拉取单次快照。

### 5.9 F07 历史（history）

- 展示操作会话（命令、起止时间、条目数、大小、各类动作计数）与删除审计记录（时间、模式、状态、大小、路径），最新在前。
- `--limit` 范围 1–200，默认 20；`--json` 与 Mole `history --json` 字段对齐。
- 数据源为操作日志与删除日志（见 7.3）；同时读取 Mole 旧日志目录 `~/Library/Logs/mole/`，迁移用户的历史不丢。

验收：
- 每条 apply 路径（含桌面 sidecar 调用的 `clean --apply`）都写会话开始 / 结束标记，history 中会话闭合、条目数与字节数不为 0（issue #167 / PR #169，未发版；回归测试 `crates/vole-cli/tests/clean_apply_stream.rs`）。
- **缺口**：桌面 Helper 删除的条目不写日志，history 看不到（issue #35，D8）。

### 5.10 F08 净化（purge）

**用户故事**：我想清理旧项目里的构建产物。

功能要点：
- 搜索根：`$HOME` 下的 `www`、`dev`、`Projects`、`GitHub`、`Code`、`Workspace`、`Repos`、`Development`、`Library/CloudStorage`、`.codex/worktrees`、`.claude/worktrees`；最大深度 6；可用 `~/.config/vole/purge_paths` 覆盖搜索根（hints 与 purge 共用）。
- 以项目标识文件（`package.json`、`Cargo.toml`、`go.mod`、`pyproject.toml`、`Package.swift`、`.git` 等）识别项目。
- 目标目录与 Mole 钉版一致：`node_modules`、`target`、`build`、`dist`、`.venv`、`__pycache__`、`.next`、`DerivedData`、`Pods`、`.gradle` 等 30 余种。
- 只列出 7 天以上未修改的构建物（`DEFAULT_PURGE_MIN_AGE_DAYS = 7`）；`--include-empty` 包含空目录。
- CLI 分页多选扫描后默认全选（与 Worktree / Agent 相反）；桌面同为默认全选。

### 5.11 F09 安装包（installer）

- 在 Downloads、Desktop、Documents、Public、`~/Library/Downloads`、`/Users/Shared`（及其 Downloads）、Homebrew 缓存、iCloud Downloads、Mail 下载、Telegram 下载等目录（深度 2，`DEFAULT_INSTALLER_SCAN_MAX_DEPTH`）查找 `.dmg`、`.pkg`、`.mpkg`、`.iso`、`.xip`。
- 扫描包含隐藏文件、不读 ignore 规则、不跟随 symlink、扩展名不区分大小写，等同 Mole 的 fd 与 find 两条分支；不存在的目录静默跳过。深度可由 `VOLE_INSTALLER_SCAN_MAX_DEPTH` 调整，根可由 `VOLE_INSTALLER_SCAN_ROOTS` 覆盖。
- 识别内含安装包的 `.zip`（只检查前 50 个条目，条目路径含 `.app` / `.pkg` / `.dmg` / `.xip`）；损坏或无法列出的 zip 跳过。
- 每个候选标出来源（对齐 Mole `get_source_display`）：Downloads、Desktop、Documents、Public、Library、Shared、Homebrew、iCloud、Mail、Telegram，其余显示所在目录名；TTY 多选与人读 `--plan` 都显示来源列。Homebrew 缓存中的文件去掉开头的 `<sha256>--` 作为显示名。（以上对齐补齐已合入 main，未发版。）
- apply 前除 dev / inode / mtime 外还校验文件大小与扫描时一致，不一致则跳过（记为 `path_vanished`），对齐 Mole「changed since scan」。
- `rule_id`：`installer:<ext>`；没有未移植的分支，Plan 不带 `coverage_note`。
- CLI 的 TTY 多选默认全不选（对齐 Mole）；桌面版扫描后默认全选。有删除失败时退出码非零（目前唯一满足 D7 契约的命令）。

验收：
- 隐藏的 `.dmg`、大写扩展名 `.DMG`、带空格与特殊字符的文件名都能列出；`.pdf`、`.tar.gz`、指向安装包的 symlink 不列出。
- zip 第 51 个条目之后才出现 `.app` 时不列出。
- 扫描后文件大小变化，apply 不删除该文件。

验证：`crates/vole-cli/tests/installer_cli.rs`。

### 5.12 F10 Worktree

**用户故事**：AI 编程工具和我自己留下了很多 Git worktree，我想看清楚并把不要的整棵 checkout 移进废纸篓。

功能要点：
- 只管理整棵 Git checkout，候选分 `worktree:linked`、`worktree:stale`、`worktree:orphan-dir` 三类，不管理其他目录。
- 展示阻塞项（如未提交改动）；默认全不选；需逐项确认。
- 删除后执行 `git worktree prune` 移除注册；从废纸篓恢复不会自动重新关联 worktree，需在说明中告知。
- Plan TTL 900 秒。
- 话术约束：不宣称「可安全删除」。

验收：
- `~/.claude/worktrees`、`~/.codex/worktrees` 下的 linked worktree 以 `git worktree list` 首行的主 checkout 作为记录的 repo；apply 不得把它们当作白名单跳过（issue #166 / PR #168，未发版；回归测试 `crates/vole-cli/tests/worktree_cli.rs`）。

### 5.13 F11 Agent

**用户故事**：Cursor / Codex / Claude 在我机器上留下了大量会话、缓存与工作区容器，我想审阅后清理。

功能要点：
1. **封闭允许表**：只扫描 `~/.cursor/`、`~/.codex/`、`~/.claude/`，以及已发现仓库下的 `<repo>/.cursor/`、`<repo>/.claude/`；禁止对整个 `$HOME` 深扫，不扫描其他未点名产品。
2. **三类候选**（同一张表，用 `rule_id` 区分）：`agent:container`（非 checkout 的工作区容器）、`agent:session`（会话 / transcript）、`agent:cache`（可重建缓存）。
3. **永不候选**：`auth.json`、`config.toml`、`settings.json`、`.credentials.json`、`mcp.json` 等配置与凭证文件；`extensions`、`plugins`、`skills`、`prompts`、`memories`、`rules`、`.git`、`worktrees` 等目录。
4. **硬排除**（plan 与 apply 都执行）：当前工作目录本身或其祖先；`vole worktree` 认领的 checkout（同一路径只归一个命令）；保护层拒绝的路径。
5. **排序**：体积大、mtime 旧的优先，只排序不隐藏；阻塞项只染色不参与排序。
6. **预算**：总墙钟 15 秒（`VOLE_TIMEOUT_AGENT_SCAN_SEC`），单根 2 秒，目录计量单次 `du` 上限 0.8 秒；超时跳过该根且不当作可删。
7. 默认全不选；默认废纸篓；Plan TTL 900 秒；`id = agent:{kind}:{canonical-path}`。
8. 话术约束：UI 与 JSON 不得出现 `safe`、`deletable` 或「可安全删除」；不得以「mtime 超过 N 天」作为可删依据。

验证：`crates/vole-cli/tests/agent_cli.rs`。

### 5.14 F12 白名单

- `clean --whitelist`：TTY 分页多选维护受保护的缓存路径；`--whitelist-add/--whitelist-remove/--whitelist-list` 供脚本使用。写入 `~/.config/vole/whitelist`，读取时兼容 `~/.config/mole/whitelist`。
- `optimize --whitelist`：同样的交互，维护跳过的优化任务 id；写入 `~/.config/vole/whitelist_optimize`。
- 白名单操作与 `--apply`、`--plan` 等参数互斥。
- 桌面版没有白名单入口（列入 13.2 待办）。

### 5.15 F13–F16 辅助命令

| 功能 | 需求 |
|---|---|
| **touchid** | `status` 查看 sudo 是否启用 Touch ID；`enable` / `disable` 修改 `/etc/pam.d/sudo_local`（在用户临时目录暂存后写入）；省略动作时交互切换；支持 `--plan` 预览、`--json` |
| **update** | 只有用户主动执行才联网；查询 GitHub Releases 最新版并替换二进制；`--check` 只查不装；`--force` / `-f`（Homebrew 安装时同时绕过「优先用 brew」门）、`--nightly`（追 main，Homebrew 安装不支持）、`-y` 跳过确认；识别安装来源（Homebrew / 手动）；`--json`。桌面设置页用 `update --check --json` 检查、`update --yes --json` 执行 |
| **remove** | 卸载 Vole 本身；`--plan` 预览（`--dry-run` / `-n` 别名），`-y` 跳过确认，`--purge-oplog` 一并删除操作日志（默认保留），`--json` |
| **completions** | 生成 bash / zsh / fish / elvish / powershell 补全，随 clap 命令定义自动更新 |

### 5.16 F17 桌面特权助手（仅桌面）

**现状（按 `vole-macos/Privilege/*`、`PrivilegedHelper/*` 实现描述）：**

- 以 `SMAppService.daemon` 注册 root LaunchDaemon（Mach 服务名 `cn.waytoai.vole-macos.helper`，可执行文件 `VolePrivilegedHelper`），通过 XPC 提供三个方法：`ping`（返回 pid / uid）、`removeAuthorizedPaths`（对通过校验的路径以 root 执行 `FileManager.removeItem`，**永久删除，不进废纸篓**）、`bootoutLaunchdLabel`（`launchctl bootout system/<label>`，label 须匹配 `^[A-Za-z0-9][A-Za-z0-9._-]*$`）。
- 路径白名单（App 与 Helper 各持一份代码常量，须同步）：`/Library/Caches`、`/Library/LaunchDaemons`、`/Library/LaunchAgents`、`/Library/PrivilegedHelperTools`、`/private/tmp`、`/private/var/tmp`、`/private/var/log`、`/private/var/db/diagnostics`、`/private/var/db/DiagnosticPipeline`、`/private/var/db/powerlog`；拒绝前缀 `/Library/Updates`、`/macOS Install Data`。路径做 realpath 规范化，拒绝 `..`，`/tmp` 归一到 `/private/tmp`。
- Helper 在建立连接时校验调用方代码签名：Team ID `WCYC8XY4V2` 且 bundle id `cn.waytoai.vole-macos`（Debug 构建保留同 Team 回退）。
- 状态：未注册 / 待系统设置批准 / 已启用 / Bundle 缺少 Helper。用户在 App 首页点「启用 Root 特权助手」，再在系统设置批准后台项。
- 路由：桌面 apply 前按路径前缀把已选条目分成「用户域」（sidecar `--apply`，进废纸篓）与「需 root」（Helper，永久删除）两批；候选列表给需 root 行打「需 root」标签，可筛「仅需 root」。
- 未启用时：需 root 条目跳过并计入 `skipped`，结果页附说明「已跳过 N 项需管理员权限的文件」，不假装已删除（满足）。
- 用户域路径仍由 sidecar 处理，清理编排不搬进 Helper（满足）。

**需求（现状未满足，issue #34 / #35）：**

1. Helper 须逐路径返回结果（已删除 / 不存在 / 校验拒绝 / 删除失败及原因）；App 据此分别计入 `succeeded` / `skipped` / `failed`，不得整批计成功。目前 Helper 对不存在的路径静默跳过且整批回 `ok`，App 把整批条目数加进成功数。
2. Helper 删除须记入操作日志与删除日志，使 `vole history` 与桌面「历史」能看到（落点见 D8）。目前 Helper 不写任何日志（设计稿中的 os_log 也未实现）。
3. Helper 删除前须复核条目身份（plan 记录的 `dev` / `ino` / `mtime`，至少大小），对齐 NFR-S3；目前只做路径白名单校验。
4. 「需 root」的判定应来自 Plan 条目的提权标记而非路径前缀；在此之前 `/private/tmp`、`/private/var/tmp` 不应把条目路由到 Helper（D5）。
5. 以上补齐前，白名单不得扩大（见 11）。

### 5.17 F18 完全磁盘访问引导（仅桌面）

- 在清理与各 Plan 模块的空闲页、设置页探测完全磁盘访问（FDA）：尝试列举 `~/Library/Mail`，失败即视为未授权；开始扫描时再探测一次。
- 未授权时弹「需要完全磁盘访问」提示（按钮：打开系统设置 / 稍后），说明扫描结果可能偏少，并引导到「系统设置 → 隐私与安全性 → 完全磁盘访问」。不在 App 启动时弹窗。

---

## 6. 交互与界面需求

### 6.1 CLI / TUI

- 人读输出使用简洁 ASCII 与主题色，不复刻 Mole 的 banner 与猫；status 使用 Vole 自有的 ASCII 动画吉祥物（`K` 可隐藏）。
- 长记录自动换行；列表显示已选与总大小；分页列表 `Q` 标为 Quit。
- 耗时扫描显示 spinner（如 Worktree 盘点）。
- 确认提示默认否（`[y/N]`）。
- `--help` 与首页 Helper 必须列全已合入 main 的顶层命令，且不得列出未发布的命令。
- 语言现状：帮助、菜单、确认提示为英文；apply 报告中「移入废纸篓 / 永久删除」两行、clean 计划摘要说明、权限警告为中文（见 D6）。

### 6.2 桌面版

**侧栏十项**（顺序固定）：清理 → 卸载 → 优化 → 净化 → 安装包 → Worktree → Agent → 分析 → 历史 → 状态。

**Plan 五态**（清理、卸载、优化、净化、安装包、Worktree、Agent 共用）：

| 状态 | 内容 |
|---|---|
| idle | 模块说明、开始扫描按钮、权限与 Helper 状态卡片 |
| scanning | 实时进度（已扫描数、当前路径），可取消 |
| candidates | 候选列表、勾选、全选 / 全不选、合计大小；永久删除开关只在净化 / 安装包 / Worktree / Agent 提供（默认关），清理 / 卸载 / 优化无此开关，用户域条目固定进废纸篓 |
| applying | 执行进度 |
| result | 移入废纸篓与永久删除分开统计；跳过项与原因（Helper 批次目前整批计入成功，见 F17） |

**默认勾选矩阵（两端对照）：**

| 模块 | CLI（TTY） | 桌面 | 备注 |
|---|---|---|---|
| 清理 | 不逐项选，确认即全部执行（白名单排除） | 扫描后默认全选 | 语义对齐 |
| 卸载 | 分页多选，默认全不选 | 扫描后默认全选 | 不一致，issue #32，待 D4 |
| 优化 | 不逐项选，确认即全部执行（任务白名单排除） | 扫描后默认全选 | 待 D4 |
| 净化 | 分页多选，默认全选 | 默认全选 | 对齐 |
| 安装包 | 分页多选，默认全不选（对齐 Mole） | 默认全选 | 已知两端不同 |
| Worktree | 分页多选，默认全不选 | 默认全不选 | 对齐 |
| Agent | 分页多选，默认全不选 | 默认全不选 | 对齐 |

需求要点：
- App 不得对用户勾选项用 `FileManager` 直接删除，一律走 sidecar `--apply` 或 Helper 白名单通道。
- **版本门控**：启动时读取 sidecar `--version`；现行实现只接受 `2.20.x`（主.次精确匹配），否则 Worktree / Agent 标为「需要内嵌 vole 2.20」并灰显、不启动扫描。规则待 D3 拍板。
- 设置页（「工坊设置」）提供 sidecar 版本、完全磁盘访问状态、Touch ID、检查与执行更新、卸载 Vole 预览与确认。
- 视觉：田鼠（Vole）主题、吉祥物活动状态、「土层」风格面板。

---

## 7. 数据、协议与配置

### 7.1 NDJSON 协议（冻结，`schema_version = 1`）

- stdout 只输出协议事件，人读日志走 stderr。
- 事件类型：`progress`（`scanned`, `current`）、`candidate`（`id`, `path`, `label`, `size`, `rule_id`）、`skipped`（`rule_id`, `reason`）、`done`（`report`）、`aborted`（`reason`）。
- 演进规则：只允许追加可选字段或追加 `SkipReason` 变体；破坏性变更必须提升 `schema_version` 并单独立项。
- 权威说明：[`docs/protocol.md`](docs/protocol.md)。

### 7.2 Plan 与 Report

| 对象 | 字段 |
|---|---|
| Plan | `schema_version`, `created_at`, `ttl_secs`（默认 900）, `entries[]`, 可选 `coverage_note` |
| Plan entry | `id`, `path`, `label`, `size`, `rule_id`, `skip_reason`, `dev`, `ino`, `mtime`（无「需提权」标记，见 D5） |
| Report | `succeeded`, `skipped`, `failed`, `skipped_by_reason[]`, `trashed_bytes`, `deleted_bytes`, 可选 `coverage_note` |

`rule_id` 前缀约定：clean 为规则 id；`uninstall:` / `uninstall:leftover:`；`optimize:delete:` / `optimize:action:`；`purge:`；`installer:`；`worktree:`；`agent:`。

### 7.3 本地数据与配置

| 数据 | 位置 | 说明 |
|---|---|---|
| 操作日志 | `~/Library/Logs/vole/operations.log` | 格式与 Mole 一致；`VOLE_OPERATIONS_LOG` 可覆盖；history 同时读取 `~/Library/Logs/mole/` |
| 删除日志 | 同目录 `deletions.log` | 供 history 审计；`VOLE_DELETE_LOG` 可覆盖 |
| 配置目录 | `~/.config/vole` | `VOLE_CONFIG_DIR` 可覆盖；含 `whitelist`、`whitelist_optimize`、`status_prefs`、`purge_paths`、安装来源记录 |
| 清理白名单 | `~/.config/vole/whitelist` | 读取兼容 `~/.config/mole/whitelist` |
| 优化白名单 | `~/.config/vole/whitelist_optimize` | 读取兼容 Mole |
| status 偏好 | `~/.config/vole/status_prefs` | 读取兼容 Mole |
| 规则数据 | 随包 `share/vole/rules` | 运行时按 `VOLE_RULES_DIR` → 二进制同级 `data/rules` → 二进制相对 `../share/vole/rules` → 源码树 `data/rules` 的顺序发现。Homebrew、`install.sh`、以及手动安装到 `~/.local/bin` + `~/.local/share/vole/rules` 都落在第三条规则上；桌面内嵌于 `Vole.app/Contents/share/vole/rules` |
| 保护清单 | `data/protection.toml` | 由 `scripts/gen-protection-toml.sh` 从 Mole 生成，禁止手改 |

### 7.4 Mole 兼容 JSON

`status --json`、`analyze --json`、`history --json` 中与 Mole 同名的字段必须口径一致；Vole 可以追加字段（Mole 字段集是 Vole 的子集）。

---

## 8. 非功能需求

### 8.1 安全性（最高优先级）

| 编号 | 需求 | 现状 |
|---|---|---|
| NFR-S1 | 所有删除只走统一删除漏斗（`mole_delete_verified` / 路径校验 / 保护层 / 废纸篓），禁止第二条删除路径或平行 `rm -rf` | CLI 满足。**已知例外**：桌面 Helper 用 `FileManager.removeItem` 直接永久删除，仅有前缀白名单与规范化校验，无保护清单、无身份复核、无日志。补救见 F17、D5 / D8；补齐前 Helper 白名单不得扩大 |
| NFR-S2 | 删除前校验路径（`validate_path_for_deletion`）与 App 保护清单，语义与 Mole 一致 | 满足（Helper 批次除外） |
| NFR-S3 | Apply 前校验 Plan TTL 与条目身份（`dev` / `ino` / `mtime`），身份变化则跳过 | 满足（Helper 批次除外） |
| NFR-S4 | 默认 `DeleteMode::Trash`；永久删除需显式 `--permanent` 或桌面开关 | 满足；Helper 批次固定永久删除且无开关，须在确认文案中明示（已有） |
| NFR-S5 | 不确定时跳过（fail-closed），不悄悄扩大删除范围 | 满足 |
| NFR-S6 | Plan 文件视为不可信输入，apply 时重新校验 | 满足 |

### 8.2 隐私

- 日常功能全部本地完成，不上报任何文件信息或使用数据。
- 只有 `vole update`（及桌面设置中的检查更新）会访问 GitHub API。

### 8.3 性能

| 项 | 需求 |
|---|---|
| 扫描预算 | hints 探针 15 秒；Agent 总扫描 15 秒、单根 2 秒；目录计量带超时，禁止无界 `du` |
| 外部命令 | 尽量用原生 API 替代子进程；只保留没有稳定公开 API 的系统命令（`defaults`、`lsregister`、`launchctl`、`tmutil`、`diskutil`、`mdfind` 等），并统一设置超时 |
| 取消 | 支持 Ctrl+C 中止，流模式输出 `aborted`；桌面扫描与执行可取消，取消后的 apply 结果页注明「可能已部分执行」 |
| 交互流畅 | analyze 后台扫描、逐项刷新；status 定时刷新 |

### 8.4 兼容性

| 项 | 要求 |
|---|---|
| CLI 系统版本 | macOS 12 及以上 |
| CLI 架构 | Apple Silicon（aarch64）与 Intel（x86_64）分别出包 |
| 桌面系统版本 | 部署目标 macOS 26.5（SMAppService 需要 macOS 13+） |
| 桌面架构 | Universal（Apple Silicon + Intel） |
| 其他平台 | 不支持；Linux / Windows 只保留 trait 边界 |

### 8.5 可靠性与可观测性

- 同一命令配置级互斥锁，防止并发执行。
- 每次会话写操作日志；报告区分成功、跳过（含原因）、失败。
- 权限不足、超时、未移植的类别都有明确提示（loud hint）与 `coverage_note`。
- 退出码契约见 5.2 验收（D7）。

### 8.6 可维护性与质量门禁

- **CLI**：CI（GitHub Actions，`macos-latest`）：`check-license`、`check-dep-direction`、`check-protocol-doc`、`check-command-surface --enforce`、`cargo fmt --check`、`cargo clippy -D warnings`、`cargo test --workspace`、双架构交叉构建、plan 阶段一致性双跑。
- 一致性测试（`conformance`）：在一次性测试根目录（`VOLE_TEST_ROOT`）下对比 Mole 与 Vole 的行为，不得指向真实 `$HOME`。
- **桌面**：仓库**没有 PR CI**，只有 tag 触发的 Release 工作流（签名、公证、DMG）。合并前依赖本机 `xcodebuild -scheme vole-macos test`（约 99 个单元测试，覆盖协议解析、Plan IO、版本门控、Helper 路径授权、默认勾选等）与「每次合并前代码评审」仓库规则。UI 测试 target 现仅为 Xcode 模板且未加入 scheme（issue #31）；PR #33（草稿）接入 7 个冒烟用例与一套可选的 e2e 用例（需指定 fixture HOME），待合并。
- 许可证：GPL-3.0-only，由脚本守卫。

### 8.7 国际化

- README 提供五种语言：English、简体中文、繁體中文、日本語、한국어，内容须同步。
- CLI 人读输出：帮助、菜单、提示为英文；apply 报告部分行、clean 摘要说明、权限警告与 `coverage_note` 为中文（统一方向待 D6）。桌面 UI 以中文为主（侧栏 Worktree / Agent 保留英文名）。

---

## 9. 权限与安全模型

| 场景 | CLI | 桌面 |
|---|---|---|
| 用户域文件 | 当前用户权限 + 废纸篓 | sidecar 处理，同 CLI |
| 受 TCC 保护的目录 | 提示开启终端的完全磁盘访问 | 引导为 Vole 开启完全磁盘访问 |
| 系统路径 / LaunchDaemon | `sudo -n` 非交互，无凭证则跳过并提示；TTY 下可先 `sudo -v` 缓存凭证 | 特权助手 XPC：白名单前缀内**永久删除**（不进废纸篓），未启用则跳过并提示；审计缺口见 F17 |
| Touch ID | 可为 sudo 启用 `pam_tid.so` | 设置页同功能 |

原则：不以交互式 sudo 冒充桌面 Helper；Helper 只提供窄接口，不承载清理编排。

---

## 10. 分发、版本与发布

### 10.1 分发渠道

| 渠道 | CLI | 桌面 |
|---|---|---|
| Homebrew tap | `brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole && brew install vole` | — |
| GitHub Releases | `vole-<ver>-<arch>-apple-darwin.tar.gz`（含 `bin/vole` 与 `share/vole/rules`） | `Vole-*.dmg` / `.zip` + `SHA256SUMS` |
| AI 助手安装 | README 提供安装提示词（只安装，不执行修改系统的命令） | — |
| 安装脚本 | `install.sh` | — |

约束：不提交 Homebrew core；不上架 Mac App Store。

### 10.2 签名与公证

- CLI 与桌面均使用 Developer ID 签名并经 Apple 公证；签名流水线脚本见两仓 `scripts/`。
- 桌面 App 与 Helper 须使用同一 Team ID 签名，启用 Hardened Runtime；sidecar 也须带 Hardened Runtime。

### 10.3 版本策略

- 严格 SemVer：MAJOR 用于破坏 CLI / 协议 / 默认行为的变更；MINOR 用于兼容的新能力（新命令、规则扩量）；PATCH 用于兼容的缺陷修复。
- 「产品 v2 / v3」是能力代际话术，不等于包 MAJOR；当前包版本 2.x，不会因「v3」发布 3.0.0。
- 唯一版本来源：`Cargo.toml` 的 `[workspace.package].version`；tag 为 `v${version}`；`docs/releases/v${version}.md` 与 Release 正文同源；README 与 Formula 版本同步。
- 「已合入未发版」状态：main 上的用户可见变更在打 tag 前对 Homebrew / Release 用户不可见；PRD 与 README 不得把未发版能力写成当前版本已提供。下一 MINOR 由 main 上已有的用户可见工作触发，是否打 tag 由产品负责人确认（D1）。
- `schema_version` 独立于包版本。
- 打 tag、建 Release 须经产品负责人确认。
- 桌面版无 `docs/releases/`，Release 正文直接写在 GitHub Release（列入 13.2 待办对齐）。

### 10.4 CLI 与桌面的版本联动

- Release 工作流（tag 触发）从 `wukongnotnull/vole` 的 **tag `v2.20.0`** 检出并构建 Universal sidecar。本机 Debug 构建默认用相邻 `../vole` 工作副本（`VOLE_SRC` 可改），`scripts/embed-vole.sh` 只校验 `--version` 字符串等于 `2.20.0`，因此 main 上的未发版改动也能通过校验被嵌入。「内嵌 2.20.0」在本机构建与 Release 产物之间并不等价；对外描述以 Release 产物为准。
- 桌面里程碑按「追上 CLI 某版本」规划；sidecar 不满足要求时，相关模块必须显示不可用。门控规则现为精确 `2.20.x`，待 D3。
- 桌面每次升 sidecar 须同时更新 `embed-vole.sh` 的 `REQUIRED_SIDECAR_VERSION`、`release.yml` 的 `ref` 与 `SidecarVersion` 常量。

---

## 11. 范围约束与明确不做

| 项 | 说明 |
|---|---|
| 顶层 `vole hints` 命令 | hints 只作为 `clean` 的只读附加信息 |
| 扩大 `worktree` 管理范围 | 只管整棵 Git checkout，其余归 `agent` |
| `clean --apply` 删除本地快照 | 本地快照只报告 |
| 删除 `/Library/Updates`、`/macOS Install Data` | 不做（Helper 亦列为拒绝前缀） |
| 桌面版 analyze 删除 | 桌面分析只读 |
| 非 macOS 平台 | 不做 |
| 遥测与上报 | 不做 |
| Homebrew core、Mac App Store | 不做 |
| 第二条删除路径 | 禁止；桌面 Helper 为已知例外，在 F17 审计需求补齐前不得扩大白名单 |
| 无必要提升 `schema_version` | 禁止 |
| 复刻 Mole UI 细节（banner、动画猫等） | 不做 |
| Agent 扫描未点名的产品或整个 `$HOME` | 不做 |

---

## 12. 风险与应对

| 风险 | 影响 | 应对 |
|---|---|---|
| 误删用户数据 | 高 | 预览 + 确认 + 保护层 + 默认废纸篓 + TOCTOU 复核；保护清单与 Mole 对齐并由脚本生成 |
| 桌面卸载默认全选，一次确认卸载全部 App | 高 | D4：卸载默认全不选；issue #32 |
| Helper 通道旁路删除漏斗（无保护清单复核、无日志、整批计成功） | 高 | 白名单冻结；D5 / D8；issue #34 / #35 |
| 规则过期（应用升级改了目录结构） | 中 | 规则记录 `last_verified`；一致性测试；跳过不确定项；fixture 须覆盖真实命名（issue #171 教训） |
| Agent 与 Worktree 认领重叠 | 中 | 同一路径只归一个命令；checkout 归 worktree |
| 误删 Agent 会话数据 | 中 | 默认全不选；不宣称可安全删除；配置与凭证文件永不候选；排除当前目录 |
| TCC / 权限导致扫描不全 | 中 | 响亮提示与 FDA 引导；结果中说明 |
| 扫描耗时过长 | 中 | 墙钟预算与超时跳过 |
| main 与发版漂移（用户拿不到已合入的能力与修复） | 中 | D1 发 2.21.0；PRD 标注「未发版」 |
| 版本门控精确匹配，sidecar 升级后 Worktree / Agent 不可用 | 中 | D3 改为 `>= 2.20.0` |
| 桌面与 CLI 版本漂移 | 中 | sidecar 钉版与运行时版本门控 |
| GPL 衍生作品合规 | 低 | 两仓 GPL-3.0；README 注明 Mole 来源；分叉须更名 |
| Mole 上游演进导致对齐面变化 | 低 | 钉版 v1.48.1，不跟随上游 |

---

## 13. 版本规划与里程碑

### 13.1 已交付

| 代际 | 主题 | 包版本区间 | 关键交付 |
|---|---|---|---|
| v1 | Rust 重写核心三命令 | 0.0.x → 1.0.0 | `status` / `analyze` / `clean` / `history`、NDJSON 协议冻结、签名公证 |
| v2 | CLI 补齐 Mole 命令面 | 1.1.0 → 2.19.x | `uninstall`、`optimize`、`purge`、`installer`、`touchid`、`update`、`remove`、clean hints 主路径、交互式 TUI 与 Mole 对齐、`worktree` |
| v3 | 体验、增值、分发 | 2.20.0（2026-09-03 发版） | `vole agent`、首页第 7 项、hints 长尾、帮助与 footer 诚实化、五语 README 与 Formula 对齐 |
| CLI main（未发版） | Mole 对齐长尾与缺陷修复 | 仍标 2.20.0，8 个已合并 PR（16 个提交） | F02 Developer 大户长尾（PR #162）、F04 前置诊断（PR #164）、F09 安装包对齐补齐（PR #165）、Worktree 误跳过修复（PR #168）、clean 历史会话标记修复（PR #169）、Formula 钉 2.20.0（PR #159）、PRD（PR #160 / #161） |
| 桌面 0.x | 图形入口 | 0.1.0 → 0.3.0（2026-09-03 发版） | Clean MVP、侧栏八项、特权助手、Universal DMG 与公证流水线、Worktree + Agent（十项侧栏，sidecar 2.20.0） |

### 13.2 待办与待规划

截至本文档修订日，仓库中没有已批准的下一代际规格。以下为已登记的待办（按 issue / PR）与评审时可考虑立项的方向：

| 事项 | 仓库 / 编号 | 说明 | 关联决定 |
|---|---|---|---|
| CLI 2.21.0 发版 | vole | 打包 main 上的未发版能力与修复 | D1 |
| 桌面 0.4.0 | vole-macos | sidecar 升 2.21.0，修 #32 / #34 / #35，合并 PR #33 | D2、D3 |
| sudo-only `/Library/Developer` Xcode 项 | [issue #163](https://github.com/wukongnotnull/vole/issues/163) | DocumentationCache 旧索引、CoreSimulator 系统缓存、模拟器运行时卷 | — |
| apply 退出码契约 | [issue #170](https://github.com/wukongnotnull/vole/issues/170) | 除 installer 外失败条目不影响退出码 | D7 |
| DeviceSupport 规则 glob 只匹配 `)` 结尾 | [issue #171](https://github.com/wukongnotnull/vole/issues/171) | 匹配不到 `… arm64e` 目录，与 Mole 不一致 | — |
| clean 摘要说明英文化 | [PR #152](https://github.com/wukongnotnull/vole/pull/152)（挂起） | 需 rebase；覆盖面不足 | D6 |
| UI 测试接入 scheme | [issue #31](https://github.com/wukongnotnull/vole-macos/issues/31) / [PR #33](https://github.com/wukongnotnull/vole-macos/pull/33)（草稿） | 7 个冒烟用例 + 可选 e2e | — |
| 卸载默认全选 | [issue #32](https://github.com/wukongnotnull/vole-macos/issues/32) | 一次确认卸载全部 | D4 |
| Helper 报「完成」却未删除 | [issue #34](https://github.com/wukongnotnull/vole-macos/issues/34) | `/private/tmp` 路由到 Helper 的场景 | D5 |
| Helper 删除无审计、整批计成功 | [issue #35](https://github.com/wukongnotnull/vole-macos/issues/35) | 日志、逐项结果、身份复核 | D8 |
| 桌面能力补齐 | vole-macos | 白名单管理入口、状态与分析体验打磨 | — |
| 特权助手扩展 | vole-macos | 在 F17 审计需求补齐之后，再评估扩大系统路径覆盖并同步 CLI 覆盖说明 | D5 / D8 |
| 桌面 Release 说明 | vole-macos | 建立 `docs/releases/` 与 Release 正文同源的约定，对齐 CLI | — |
| 分发 | 两仓 | 截图、安装文档与五语 README 持续同步 | — |

---

## 14. 附录

### 14.1 术语表

| 术语 | 含义 |
|---|---|
| Plan | 扫描阶段生成的候选清单文件，apply 的唯一输入 |
| Apply | 按 Plan 执行删除或动作 |
| TOCTOU | Time-of-check to time-of-use；apply 前复核文件身份，防止 plan 后路径被替换 |
| 删除漏斗 | 路径校验 → 保护层 → 废纸篓 / 永久删除的统一通道 |
| coverage_note | 说明已覆盖 / 未覆盖类别或被跳过原因的文本（中文） |
| sidecar | 桌面 App 内嵌并以子进程调用的 `vole` 二进制 |
| 特权助手（Helper） | 由 SMAppService 注册的 root 守护进程，经 XPC 提供窄接口 |
| FDA | Full Disk Access，完全磁盘访问权限 |
| Worktree | 由 `git worktree` 创建的额外 checkout |
| Agent 残留 | AI 编程工具（Cursor / Codex / Claude）的容器、会话与缓存数据 |
| 已合入未发版 | 已进入 main 但尚未打 tag、未进入 Homebrew / Release 的变更 |

### 14.2 文档与代码差异说明（v1.1）

修订时发现以下设计稿与代码现状不一致。**结论（2026-10-18，产品负责人决定）：以代码为准**，设计稿与 README 已同步修订。

| 原设计稿表述 | 代码现状（即现行需求） | 处理 |
|---|---|---|
| v3 规格 §3.3 写明「analyze 删除、analyze Space 多选删」不做 | CLI analyze TUI 已支持 `Space` 多选与 `Delete` 移入废纸篓（`crates/vole-cli/src/tui/analyze_state.rs`、`crates/vole-core/src/delete/analyze_trash.rs`） | 已改 v3 规格 |
| v3 规格 §3.3 写明「`optimize --whitelist`、status 的 `k` / `c` 键与动画 cat」不做 | `optimize --whitelist*` 参数已存在（`crates/vole-cli/src/main.rs`）；status 已接线 `K`（吉祥物显隐，含动画）/ `C`（CPU 核心视图）键 | 已改 v3 规格 |
| 桌面 README 平台徽章只写「macOS」 | 工程部署目标为 macOS 26.5 | 已在 vole-macos 五语 README 写明 macOS 26.5 及以上 |

### 14.3 参考文档

| 文档 | 位置 |
|---|---|
| Rust 重写总设计 | `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` |
| SemVer 版本策略 | `docs/wukong-code/specs/2026-07-30-semver-policy-design.md` |
| 产品 v2 目标 | `docs/wukong-code/specs/2026-07-30-1900-v2-product-goals-design.md` |
| 产品 v3 CLI 代际规格 | `docs/wukong-code/specs/2026-09-02-1530-v3-cli-generation-design.md` |
| Worktree 设计 | `docs/wukong-code/specs/2026-08-14-0228-worktree-design.md` |
| NDJSON 协议 | `docs/protocol.md` |
| 发版记录 | `docs/releases/` |
| 桌面：特权助手设计 | `vole-macos/docs/wukong-code/specs/2026-08-08-1822-smappservice-privileged-helper-design.md` |
| 桌面：DMG 与 CI 发版设计 | `vole-macos/docs/wukong-code/specs/2026-08-10-2315-macos-dmg-ci-release-design.md` |
| 桌面：Worktree + Agent 对齐设计 | `vole-macos/docs/wukong-code/specs/2026-09-03-2053-desktop-worktree-agent-parity-design.md` |
| 桌面：本机人工测试指南 | 项目共享文件 `guides/vole-macos-manual-test.md` |

### 14.4 v1.5 grilling 发现与处理

对照 `vole` main @ `1566521` 与 `vole-macos` main @ `414a6bd` 逐条核对 v1.4 后的发现。「处理」列说明本版 PRD 的改法；需拍板的指向 D 编号。

| 编号 | 类型 | 发现 | 处理 |
|---|---|---|---|
| G1 | 不符 | 「基线 vole main @ 2.20.0」掩盖了 main 比 tag 多 8 个已合并 PR（16 个提交）未发版；v1.2–v1.4 新写入的 F02 / F04 / F09 能力对 Homebrew / Release 用户并不存在 | 基线写 SHA；5.1 加交付状态列；13.1 加「CLI main（未发版）」行；D1 |
| G2 | 不符 | 桌面 Release 流水线 `ref: v2.20.0`，0.3.0 DMG 不含 PR #168 / #169；本机构建却可嵌入 main，「内嵌 2.20.0」语义不唯一 | 10.4 重写；D2 |
| G3 | 隐患 | 版本门控精确匹配 `2.20.x`，下一次 sidecar 升级即把 Worktree / Agent 置为不可用 | 6.2、10.4 注明；D3 |
| G4 | 遗漏 | 6.2 只写了 Worktree / Agent 与净化 / 安装包的默认勾选，漏掉清理 / 卸载 / 优化（桌面默认全选），且 CLI 卸载默认全不选与桌面不一致（issue #32） | 6.2 增加两端默认勾选矩阵；5.5 补 CLI 默认；D4 |
| G5 | 不符 | 5.2 写「需要时分页多选」，实际 clean / optimize 无逐项多选，确认即全部执行 | 5.2 拆为两行 |
| G6 | 不符 | 6.2 写永久删除开关为五态共有，实际只有净化 / 安装包 / Worktree / Agent 有 | 6.2 改写 |
| G7 | 矛盾 | NFR-S1「禁止第二条删除路径」与桌面 Helper 用 `FileManager.removeItem` 永久删除并存；Helper 无保护清单、无身份复核、无日志、整批计成功、对不存在路径静默成功（issue #34 现象） | 4.1、5.16、8.1、9、11 按实现重写并记为已知例外；开 issue #35；D5 / D8 |
| G8 | 遗漏 | 5.16 未列白名单前缀；`/private/tmp`、`/private/var/tmp` 会把用户可写目录条目路由到 root 永久删除 | 5.16 列全前缀与拒绝前缀；D5 |
| G9 | 不符 | 8.6 写「桌面仓有单元测试与 UI 测试 target」，UI 测试实为模板且不在 scheme（issue #31）；桌面无 PR CI；3.2「CI 全绿后才合并」只对 CLI 成立 | 3.2、8.6 改写 |
| G10 | 不符 | 8.7「CLI 人读输出以英文为主」：apply 报告「移入废纸篓 / 永久删除」行、clean 摘要说明、权限警告、`coverage_note` 均为中文；PR #152 挂起两个月 | 6.1、8.7 写明现状；D6 |
| G11 | 遗漏 | 退出码契约缺失；只有 installer 在失败时返回非零 | 5.2 验收补退出码；开 issue #170；D7 |
| G12 | 不符 | 7.3「手动安装需复制到 `~/.local/share/vole/rules`」把安装步骤写成了发现规则；实际按 `VOLE_RULES_DIR` → 二进制相对路径发现 | 7.3 改写 |
| G13 | 遗漏 | 日志 / 配置的环境变量覆盖、`purge_paths`、history 读取 Mole 旧日志均未写 | 7.3、5.10、14.5 补齐 |
| G14 | 不符 | 5.17 写「启动时探测 FDA」，实际在空闲页 / 设置页与开始扫描时探测，不在启动弹窗 | 5.17 改写 |
| G15 | 遗漏 | `update --check`（桌面设置页依赖）未写入 F14 | 5.15 补齐 |
| G16 | 缺陷 | DeviceSupport 三条规则 glob `*)` 匹配不到 `… arm64e` 目录；fixture 只覆盖 `)` 结尾命名，与 Mole 不一致 | 5.4 第 5 点注明；开 issue #171 |
| G17 | 含糊 | 5.4「系统服务孤立项（只读子集）」未说明 apply 会用 `sudo -n` 永久删除；「孤立应用数据不扫描 Containers / Group Containers」与另有专门规则覆盖的事实混在一起 | 5.4 第 3、4 点改写 |
| G18 | 不可验收 | 多数功能无「验证方式」，验收只能靠人工 | F01 / F02 / F07 / F09 / F10 / F11 补测试与 fixture 位置 |
| G19 | 遗漏 | 13.2「待规划」未对应任何 issue / PR，无法追踪 | 13.2 改为按 issue / PR 列表 |
| G20 | 遗漏 | 已发现的回归（issue #166 / #167）没有沉淀为验收条款 | 5.9、5.12 增加验收 |

### 14.5 用户可见环境变量

测试专用变量（`VOLE_TEST_*`、`VOLE_UPDATE_FAKE*`、`VOLE_PAM_*` 等）不在此列。

| 变量 | 作用 | 兼容别名 |
|---|---|---|
| `VOLE_RULES_DIR` | 规则目录，覆盖自动发现 | — |
| `VOLE_CONFIG_DIR` | 配置目录（默认 `~/.config/vole`） | — |
| `VOLE_OPERATIONS_LOG` / `VOLE_DELETE_LOG` | 操作日志 / 删除日志路径 | `MOLE_OPERATIONS_LOG` / `MOLE_DELETE_LOG` |
| `VOLE_NO_OPLOG` | 关闭操作日志写入 | — |
| `VOLE_DRY_RUN` | 置 1 时强制只计划不执行 | `MOLE_DRY_RUN` |
| `MOLE_DELETE_MODE` | 删除模式覆盖（Mole 兼容） | — |
| `VOLE_TIMEOUT_HINT_SCAN_SEC` | clean hints 探针总预算（默认 15 秒） | — |
| `VOLE_TIMEOUT_AGENT_SCAN_SEC` | Agent 扫描总预算（默认 15 秒） | — |
| `VOLE_INSTALLER_SCAN_MAX_DEPTH` / `VOLE_INSTALLER_SCAN_ROOTS` | 安装包扫描深度（默认 2）/ 扫描根 | — |
| `VOLE_ENABLE_DISK_VERIFY` / `VOLE_TIMEOUT_DISK_VERIFY_SEC` | 开启 `disk_verify` / 其超时 | `MOLE_ENABLE_DISK_VERIFY` |
| `VOLE_OPTIMIZE_DIAG_CPU_THRESHOLD` / `VOLE_OPTIMIZE_DIAG_SAMPLE_DELAY` | optimize 前置诊断 CPU 阈值 / 采样间隔 | `MOLE_` 前缀 |
| `VOLE_DOTDIR_ORPHAN_AGE_DAYS` | 孤立点目录提示的年龄阈值 | `MOLE_ORPHAN_AGE_DAYS` |
| `MOLE_CLAUDE_VM_ORPHAN_AGE_DAYS` | Claude Desktop VM 孤立判定年龄（默认 7 天） | — |
| `MOLE_XCODE_DEVICE_SUPPORT_KEEP` / `MOLE_JETBRAINS_TOOLBOX_KEEP` | 规则 `keep_newest` 的保留数覆盖 | — |
| `VOLE_MENU_SORT_MODE` / `VOLE_MENU_SORT_REVERSE` / `VOLE_MENU_IGNORE_INITIAL_ENTER` | 分页多选菜单排序与首个 Enter 行为 | `MOLE_` 前缀 |
| `VOLE_ANALYZE_LIVE_SORT` | analyze 实时排序模式 | `MOLE_ANALYZE_LIVE_SORT` |
| `VOLE_TEST_ROOT` | 一致性测试的一次性根目录（仅测试，列出以警示不得指向真实 `$HOME`） | — |
