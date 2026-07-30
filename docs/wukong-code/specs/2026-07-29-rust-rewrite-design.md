# Vole：用 Rust 实现 Mole 核心三命令的设计方案

- 日期：2026-07-29
- 参照上游：[tw93/Mole](https://github.com/tw93/Mole) v1.48.1（commit `27123a9`）
- 状态：待评审。第 11 节列出仍需决策的开放问题。
- **进入实施前必须先做第 8 节的 Phase 0.5 风险 spike。** 第 10 节的工期在 spike 之后重估，在那之前不要承诺日期。

### 关于范围的说明

本文档**不是「重写 Mole」的方案**，标题曾如此表述但名不副实。Mole 有 12 个子命令，本方案的 v1 只做 `status`、`analyze`、`clean` 三个。

按真实使用频率，这三个覆盖约 80% 的场景；但 Mole 的招牌功能之一 `uninstall`（1740 行）、以及 `optimize`（1496 行）、`purge`、`installer` 都**不在**本方案内。完成本方案得到的是「Mole 大部分日常用途的替代品」，不是 Mole 的替代品。

同时要清楚本文档的性质：它是**施工方案，不是规格说明**。547 条规则的具体路径、保护清单的 bundle ID、健康分的计算公式都在 Mole 源码里，本文档描述的是「怎么搬」而非「搬什么」。全程需要 Mole 源码在手边，任何规则语义争议都得回查 bash。这也正是第 7 节的一致性测试成为整个计划地基的原因——**它是唯一能替代规格说明的东西**。

## 目录

1. [目标与非目标](#1-目标与非目标)
2. [许可证约束](#2-许可证约束)
3. [源项目分析结论](#3-源项目分析结论)
4. [macOS 平台现实：TCC 与提权](#4-macos-平台现实tcc-与提权)
5. [架构](#5-架构) — 含 [5.6 协议与两阶段模型](#56-前端边界协议与两阶段模型)、[5.7 废纸篓口径](#57-废纸篓与释放空间的口径)、[5.8 健壮性基础设施](#58-健壮性基础设施)
6. [规则模型](#6-规则模型) — 含 [6.2 路径语义](#62-路径语义必须显式定义)、[6.3 过期与复核](#63-规则的过期与复核)
7. [一致性测试策略](#7-一致性测试策略) — 含 [7.0 执行环境](#70-执行环境不能在开发机上跑)
8. [分阶段计划](#8-分阶段计划)
9. [风险登记](#9-风险登记)
10. [工作量估算与止损判据](#10-工作量估算与止损判据)
11. [开放问题](#11-开放问题)

---

## 1. 目标与非目标

### 目标

1. **性能与自包含**：削减 Mole 现有的外部命令子进程调用，产出单一二进制。

   「自包含」需要精确定义，否则会与保留系统子进程的决策自相矛盾。本文档中它指：**不依赖 bash 运行时，不依赖任何需要用户额外安装的第三方工具**（`fd`、`sqlite3`、`jq`）。macOS 系统自带的命令（`defaults`、`lsregister`、`launchctl`、`tmutil`、`diskutil`、`mdfind`）仍会作为子进程调用，因为它们没有稳定的公开 API 替代。见 [3.5](#35-外部命令依赖) 的逐项判定。
2. **可维护性**：把 3.5 万行 Bash 换成有类型、可单测的 Rust 代码库。
3. **可分叉**：Mole 是知识来源与正确性基准，不是功能契约。交互与功能允许走自己的路。
4. **为跨平台留门**：v1 只实现 macOS 后端，但平台边界从第一天就是 trait，加 Linux 是加后端而非重构。
5. **为 SwiftUI 桌面 app 留接口**：编排逻辑与前端解耦，CLI 与将来的 app 都是薄前端。详见 [5.6](#56-前端边界协议与两阶段模型)。

### 非目标（v1）

- 不追求 12 个子命令的完整对齐。v1 只做 `status`、`analyze`、`clean`。
- 不实现 `uninstall`、`optimize`、`touchid`、`update`、`installer`、`purge`。
- 不实现 Linux/Windows 后端（只留 trait 边界，非 macOS target 直接 `compile_error!`）。
- 不实现需要提权的清理规则（详见 [4.2](#42-提权模型v1-不提权但必须响亮地告知)）。
- 不实现桌面 app 本身。v1 只交付它所需的协议与库边界。
- **不实现 `hints` 子系统**（`lib/clean/hints.sh` 967 行）。它是 `mo clean` 里的非破坏性提示（例如提醒有 build artifacts 可用 `mo purge` 清理），不影响清理正确性，且依赖 `purge_paths` 配置——而 `purge` 本身不在 v1 范围内。砍掉它同时消除了那条配置兼容的悬空需求。
- 不复刻 Mole 的 UI 细节（猫、banner 等）。

### 对齐的边界

允许分叉交互与功能，但以下三类必须与 Mole 严格一致，它们是安全与数据正确性的基准：

| 必须对齐 | 原因 |
|---|---|
| 删除前的路径校验语义与保护清单 | 一旦放宽就会删掉用户数据 |
| `--json` 中与 mole 同名字段的口径与数值定义 | 用户脚本依赖，也是一致性测试的比对面 |
| 操作日志格式（`~/Library/Logs/mole/operations.log`） | 让用户能从 mole 平滑迁移 |

配置文件方面，v1 只需兼容 `~/.config/mole/whitelist`（`clean` 用到）。`purge_paths` 属于 v1 不实现的 `purge` 命令，不在 v1 兼容范围内。

### 三种契约必须分开

文档中出现过三种不同的数据契约，早期版本把它们混为一谈，导致「字段集完全一致」这类无法同时成立的要求。明确区分：

| 契约 | 稳定性要求 | 谁在消费 |
|---|---|---|
| **Mole 兼容 JSON**：`vole status --json`、`vole analyze --json` 中与 mole 同名的字段 | mole 的字段集是 Vole 的**子集**。同名字段口径必须一致；Vole 可以追加 mole 没有的字段 | 用户既有脚本、一致性测试 |
| **Vole NDJSON 协议**：`--json-stream` 的事件流与 `Plan` | 由 `schema_version` 治理，见 [5.6](#56-前端边界协议与两阶段模型)。Phase 4 结束时冻结 v1 | 桌面 app、第三方前端 |
| **`vole-proto` 的 Rust 类型** | 无外部稳定性承诺。crate 未发布到 crates.io 前可自由重构 | 仅 workspace 内部 |

「子集而非相等」这条是决策，不是待定项。理由：Vole 需要输出 mole 没有的信息（如 `SkipReason` 明细、废纸篓与永久删除的区分），强求相等会逼着我们隐藏对用户有用的数据。一致性测试相应地只比对同名字段。

---

## 2. 许可证约束

Mole 是 GPL-3.0。阅读其源码进行的重写属于衍生作品，**Vole 必须以 GPL-3.0 发布**。仓库当前的 Apache-2.0 LICENSE 需在第一个实质提交前替换。从 Mole 提取的路径清单与保护清单同样是 GPL 作品的一部分，这一点被 GPL-3.0 覆盖后即无问题。

Mole 的 README 另有两项要求：分叉需换名（`vole` 已满足）、需标注来源。README 需加归属声明。

现在做成本接近零，等有了外部贡献者再改则需逐个征求同意。

注意 Vole 无法采用双许可。双许可要求持有全部版权，而 Vole 是 Mole 的衍生作品——规则数据、保护清单、安全校验语义都源自 Mole，这部分版权不在我们手上。因此 GPL-3.0 是唯一选项，没有商业授权的回避路径。

**本节不构成法律意见。** 以上是基于 GPL-3.0 文本与业界通行实践的理解。「阅读源码后重写是否构成衍生作品」在不同司法辖区有解释空间，但由于我们选择的方案（Vole 与桌面 app 均为 GPL-3.0）无论如何都满足最严格的解释，实践上不需要为此纠结。若将来出现商业化意图，必须先做专业法律确认，而不是重新解读本节。

### 桌面 app 的许可证

计划中的 SwiftUI 桌面 app **同样以 GPL-3.0 发布**。这消除了法律层面对架构的约束：可以自由地把 vole 二进制内嵌进 app bundle 一起分发，也不需要为了论证「两个独立程序」而刻意疏远协议设计。

但语言边界仍然存在：Swift 无法直接 link Rust crate。因此 app 与 vole 之间依然走进程边界（sidecar + NDJSON），这是**技术选择而非法律要求**。选它而不选 C ABI 的理由见 [5.6](#56-前端边界协议与两阶段模型)。

### 上游关系

Vole 一次性从 Mole v1.48.1（commit `27123a9`）取走规则知识，之后**不跟踪上游演进**，独立发展。

`third_party/mole-1.48.1/` 的源码快照仍需 vendor，但用途只有一个：作为第 7 节一致性测试的比对基准。它不参与构建，也不用于持续同步。

---

## 3. 源项目分析结论

### 3.1 构成：Mole 的主体是 Bash，不是 Go

| 部分 | 规模 | 内容 |
|---|---|---|
| Bash | 35,356 行 / 65 文件 | `clean`、`uninstall`、`optimize`、`purge`、`installer`、`touchid`、`history`、`update`，以及全部安全逻辑 |
| Go | 21,118 行 / 57 文件 | 仅 `cmd/analyze`、`cmd/status` 两个二进制，其中约 7,000 行是测试 |

`mole` 入口是 322 行 bash 路由器；`bin/analyze.sh`、`bin/status.sh` 是 15 行的 `exec` 转发壳。

最大单文件：`bin/clean.sh` 1820、`lib/core/app_protection.sh` 1766、`bin/uninstall.sh` 1740、`lib/core/file_ops.sh` 1658、`lib/optimize/tasks.sh` 1496、`cmd/analyze/update.go` 1232。

**结论**：本项目本质是「把会删用户 Mac 文件的 3.5 万行 Bash 重写成 Rust」，风险集中在删除逻辑，不在 TUI。

### 3.2 规则的真实规模：547 条，不是 108 条

这是本次评审修正的最重要一处。`clean_*` **函数**有 108 个，但真正的规则粒度是 `safe_clean` **调用点**，实测 **547 个**：

| 文件 | `safe_clean` 调用点 |
|---|---|
| `lib/clean/app_caches.sh` | 195 |
| `lib/clean/dev.sh` | 175 |
| `lib/clean/user.sh` | 172 |
| 其余 | 5 |

工期估算必须按 547 条算。

### 3.3 规则不是纯声明式的

规则里含大量过程性逻辑，实测出现次数：

| 策略 | 出现次数 | 例子 |
|---|---|---|
| mtime 排序 / 保留最新 N 个 | 80 | `clean_dev_ai_agents` 保留最新 N 个版本目录 |
| `pgrep` 进程在跑则跳过 | 63 | `clean_xcode_xctest_devices` 检测 `xcodebuild` |
| symlink 解析与保护 | 43 | 保护 `~/.local/bin/claude` 指向的那个版本；symlink 断裂时整条规则跳过 |
| `MOLE_*` 环境变量旋钮 | 69 个不同变量 | `MOLE_AI_AGENTS_KEEP`、`MOLE_JETBRAINS_TOOLBOX_KEEP` |

因此规则模型不能是纯 glob 列表，必须是「声明式 schema + 一组封闭的可组合策略 + 少量逃逸出口」。详见第 6 节。

### 3.4 bats 测试不是黑盒的

Mole 的 clean 类测试主流写法是 **source 进 shell 库后 monkey-patch `safe_clean` 函数**来捕获「本应被清理什么」：

```bash
source "$PROJECT_ROOT/lib/clean/dev.sh"
safe_clean() { echo "$1|$2"; }
clean_dev_ai_agents
# 断言 output 含 /2.1.112|Claude Code old version，且不含 /2.1.114
```

实测有 **172 个用例**依赖这种函数覆盖。这些测试**不能**直接指向 Rust 二进制。

但它们仍然高度可复用，因为覆盖点捕获的正是 `(路径, 标签)` 二元组——这与 Vole 规则引擎该输出的候选清单一一对应。做法是把每个用例的 fixture 构造与期望集合**抽取成数据**，在 Rust 侧做表驱动测试。详见第 7 节。

### 3.5 外部命令依赖

Bash 中的调用次数：`plutil` 35、`lsregister` 34、`defaults` 33、`mdfind` 26、`security` 22、`osascript` 22、`tmutil` 18、`sqlite3` 15、`launchctl` 13、`hdiutil` 8、`diskutil` 8、`sysctl` 7、`killall` 6、`pkgutil` 5、`lsof` 4、`xattr` 3、`vm_stat` 3、`scutil` 3、`mdutil` 3、`ioreg` 3、`dscacheutil` 3。Go 侧另有 `osascript` 2、`open` 2、`du` 2、`mdfind` 1。

**关于 `plutil` 与 `defaults` 必须分开看**（初版文档在此有错，把两者合并称「68 次可进程内化」）：

| 调用 | 次数 | 能否进程内化 |
|---|---|---|
| `plutil -extract` | 27 | **能**。读 app bundle 的 `Info.plist`，是普通文件，`plist` crate 直接读 |
| `plutil -lint` / `-p` | 4 | 能，同上 |
| `defaults read` | 11 | **需谨慎**。这是 CFPreferences 域读取，前面有 cfprefsd 内存缓存层，直接读 plist 文件可能拿到未落盘的过期值 |
| `defaults write` / `delete` | 6 | **不能**。直接写文件会被 cfprefsd 之后覆盖 |

那 17 次 CFPreferences 域操作必须走 `core-foundation` 的 `CFPreferences*` API，或保留 `defaults` 子进程。v1 建议保留子进程（只有 17 次，不是性能热点），把 API 迁移列为后续优化。

`lsregister`、`launchctl`、`tmutil`、`diskutil`、`spctl`、`pkgutil` 无稳定 API，永远保留子进程。连带地，它们的挂起风险也一并接下来了，见 [5.8](#58-健壮性基础设施)。

### 3.6 clean 的真实依赖闭包

早期版本按目录名推断依赖，漏算了两处。`bin/clean.sh` 直接 source 10 个文件，而 `lib/core/common.sh` 又传递引入 `base`、`log`、`timeout`、`timeouts`、`file_ops`、`help`、`ui`、`app_protection`、`bundle_resolver`、`pkg_receipts`。两处漏算：

**一、`app_protection` 在 v1 关键路径上，不只服务 `uninstall`。** `app_protection.sh`（1766 行）加 `app_protection_data.sh`（591 行）合计 2357 行，实测 clean 路径有 43 处引用（`app_caches.sh`、`caches.sh`、`apps.sh`、`user.sh`、`project.sh` 与 `file_ops.sh`）。它必须与安全闸口一同在 Phase 4a 移植——这是 4a 工期从 1 周上调到 2.5 周的原因。

**二、`timeout` / `timeouts` 被每一次 clean 运行传递引入。** 超时不是边角功能，见 [5.8](#58-健壮性基础设施)。

安全闸口本身仍是 `lib/core/file_ops.sh` 里的 `validate_path_for_deletion`（193 行起）与 `mole_delete`（750 行起），所有删除的必经之路。

---

## 4. macOS 平台现实：TCC 与提权

初版文档完全遗漏本节，而这两件事会在 Phase 4 直接卡住 `clean`。

### 4.1 TCC（完全磁盘访问）

Mole 有 `check_tcc_permissions()`（`lib/clean/caches.sh:8`），在清理前主动 touch `~/Library/Caches`、`Logs`、`Application Support`、`Containers`、`~/.cache` 来预热 TCC 弹窗，避免运行中被打断，并用 `~/.cache/mole/permissions_granted` 标记避免重复。

对 Rust 重写而言这是一等问题，因为 **TCC 授权绑定「负责进程」与代码签名身份**：

- 本地 `cargo build` 出的未签名二进制会触发全新一轮授权弹窗。
- 每次重新编译，若签名身份（或 ad-hoc 签名的 cdhash）变化，可能被视为新程序而重新弹窗。这会严重干扰开发迭代，必须在 Phase 1 实测清楚。
- Homebrew 分发的签名二进制又是第三种身份，用户需再次授权。
- 从终端 `exec` 启动时，负责进程通常仍是终端（用户已给终端授权则继承），但从 Raycast / Alfred 等启动器调用时不成立。
- 桌面 app 内嵌 vole 并 spawn 时，负责进程预期是 app 自身，授权按 app 的 bundle id 走。这**可能比 CLI 从终端跑更干净**（授权稳定绑在一个签名身份上），但属于待验证的预期而非结论。

**动作项**：Phase 1 必须做一次 TCC 行为实测，把结论写回本文档。在此之前不要对 `clean` 的可达路径集做任何假设。测试矩阵：

| 签名身份 | 启动方式 |
|---|---|
| 未签名（`cargo build`） | 终端 |
| ad-hoc 签名 | 终端、Raycast |
| Developer ID 签名 | 终端、Raycast、从 app bundle 内 spawn |

重点观察三件事：重新编译后是否重新弹窗（直接影响开发迭代效率）、授权能否从终端继承、以及 app spawn 时授权归属于谁。

**Phase 0.5 最小子集**：本机 ad-hoc 签名下读 `~/Library/Containers` 退出码 0，未观测弹窗；不足以代表 Full Disk Access 场景。

**Phase 1 完整矩阵（2026-07-30，Developer ID 已具备）**：见 `docs/findings/2026-07-phase1-tcc-devid-matrix.md`，脚本 `scripts/tcc-devid-matrix.sh`。要点：

| 观察 | 结论 |
|---|---|
| 未签名二进制 | 本机终端 exec 被 **SIGKILL (137)**，未进入读路径 |
| ad-hoc / Developer ID（终端） | 探针目录 `analyze` 均为 exit 0；未见新弹窗（可能继承终端权限） |
| Developer ID CDHash | rebuild+重签必变；同字节带 `--timestamp` 再签也会变 |
| Raycast / GUI `open -a` | 未自动化，仍需按需手测 |
| 产品含义 | 分发必须签名；开发勿依赖 debug CDHash 稳定；桌面 app 应与内嵌 vole 同签身份 |

旧 deferred 说明：`docs/findings/2026-07-phase1-tcc-deferred.md`。

### 4.2 提权模型：v1 不提权，但必须响亮地告知

Mole 用的是 `sudo -n`（非交互）：只有当 sudo 凭证已缓存时才清理 root 拥有的路径，否则跳过。需要提权的规则集中在 `lib/clean/system.sh`（35 处 sudo 引用），目标是 `/Library/Caches`、`/Library/Logs/DiagnosticReports`、`/private/var/log`、`/private/var/folders` 等系统级路径。相对 547 条规则（绝大多数在 `$HOME` 下）占比不高。

**v1 决策**：不实现提权。遇到需要提权的规则一律跳过，但**必须在报告中显式列出被跳过的规则条数与类别名称**。

报告**不承诺体积估算**。这是早期版本的一个逻辑缺陷：没有权限读取的路径同样无法 `stat`，算不出占用体积，承诺「预计涉及 X GB」在实现上不可兑现。正确的措辞是「跳过 12 条系统级规则（系统缓存、诊断报告、系统日志），需要管理员权限」，把体积留空而不是编造。

理由：静默少清比不清更糟——用户会以为清干净了。显式告知让用户知道 mole 在这一块仍有价值，也为 v2 加提权留下明确的产品缺口。

**对桌面 app 的影响**：GUI 不能用 sudo。app 要提权得走 `SMAppService` 注册特权 helper，与 CLI 的 sudo 是两套完全不同的机制。v1 两边都不提权，所以现在不冲突；但 v2 加提权时必须意识到这是两条独立路径，`vole-core::ops` 的提权接口要抽象成 trait 而不是直接调 sudo。

---

## 5. 架构

### 5.1 Workspace 划分

起步只建 **4 个 crate**。早期版本列了 8 个，对单人项目是过度拆分——crate 边界画错的代价（循环依赖、反复搬文件）远高于晚拆的代价。

```
vole/
├── Cargo.toml                 # workspace
├── crates/
│   ├── vole-proto/            # 协议类型：事件流、Plan、Report。serde + schema 版本，无逻辑
│   ├── vole-sys/             # 平台 trait + macOS 后端（plist/sqlite/IOKit/trash/子进程）
│   ├── vole-core/             # 路径校验、保护判定、文件操作、操作日志、配置、单位格式化
│   │                          #   + rules/ scan/ ops/ 三个 module（达到阈值再拆 crate）
│   └── vole-cli/              # clap 入口 + tui/ module + --json-stream sidecar 模式
├── rules/                     # *.toml 规则数据（include_str! 内嵌）
├── conformance/               # 一致性测试框架与抽取出的 fixture 数据
└── third_party/mole-1.48.1/   # 上游源码快照，仅作一致性测试基准
```

**拆分触发条件**（满足任一即拆，不满足就不动）：

- 单个 module 超过 2500 行；
- 出现需要独立版本发布的外部消费者；
- 编译时间成为迭代瓶颈且该 module 是关键路径。

预期最先达标的是 `ops`（编排）与 `scan`（遍历），大概在 Phase 3 到 Phase 4 之间。

**分层规则:依赖只能单向向下，禁止反向与横向依赖。**

```
vole-cli  ──→ vole-core ──→ vole-sys ──→ vole-proto
     └──────────────────────────────────────┘
```

`vole-proto` 是叶子，不依赖任何 workspace 内 crate，依赖的外部 crate 也要压到最少（`serde` 加标准库为主），这样将来第三方前端可以只依赖它而不背上整个 vole。CI 里用 `cargo-deny` 或一条简单的依赖检查脚本把这个方向固化，否则它一定会在某次「顺手 import 一下」中被破坏。

**编排逻辑（扫描 → 候选 → 确认 → 执行）必须住在 `vole-core::ops` 而不是 `vole-cli`。** `vole-cli` 退化成薄前端，这样 TUI、sidecar、一致性测试是同一套编排的三个消费者。这条是为桌面 app 引入的关键调整，也是一致性测试能绕开 CLI 直接驱动编排的前提。

`unsafe` 只允许出现在 `vole-sys`，其余 crate 一律 `#![forbid(unsafe_code)]`。

### 5.2 依赖映射

crate 版本为 2026-07-29 核实值。

| Mole 现状 | Vole 方案 | 备注 |
|---|---|---|
| bubbletea + lipgloss | `ratatui` 0.30 + `crossterm` | Elm 架构 → 立即模式，`update.go` 1232 行需**重构**而非翻译 |
| gopsutil/v4 | `sysinfo` 0.39 + `rustix` | 负载用 `libc::getloadavg`；磁盘 I/O 速率 sysinfo 在 macOS 支持有限，需实测后决定是否走 IOKit |
| `ioreg`/`system_profiler`（电池/温度/GPU） | `objc2-io-kit` 0.3 + `core-foundation` 0.10 | `IOPSCopyPowerSourcesInfo`、`IOServiceMatching` |
| `plutil -extract` 27 + `-lint`/`-p` 4 | `plist` 1.10 | 普通 plist 文件，安全内化 |
| `defaults read/write/delete` 17 | v1 保留子进程 | cfprefsd 缓存层，见 [3.5](#35-外部命令依赖) |
| `sqlite3` 15 | `rusqlite`（bundled） | 只读打开。`immutable=1` 会**跳过 WAL**，见下方说明 |
| `osascript` 22（Finder 移废纸篓） | `trash` 5.2 | 走 `NSFileManager trashItem` |
| `mdfind` 26 | v1 保留子进程 | 可选后续走 MDQuery FFI |
| `lsregister`/`launchctl`/`tmutil`/`diskutil`/`security`/`spctl`/`pkgutil` | 保留子进程，藏在 `SysCommand` trait 后 | 无稳定 API；trait 让测试可桩 |
| `xxhash/v2` | `twox-hash` 2.1 | 扫描缓存键 |
| `du` 子进程 + 手写遍历 | `jwalk` 0.8 | Rust 在此能真正超过 Go |
| `find`/`fd` 子进程 | 自有并行遍历 | 顺带去掉 README 现在建议的 `fd` 依赖 |
| bash 参数解析 | `clap` derive | |
| `unix.Flock`/`RenameatxNp` | `rustix` 1.1（`flock`、`renamex_np`） | 比裸 `libc` 安全 |

#### SQLite 的 WAL 陷阱

早期版本把「`immutable=1` 只读打开」当作浏览器 DB 被锁的解法，这不完整。`immutable=1` 告诉 SQLite 该文件不会变化，于是它**跳过 WAL 与 shm 文件**。对一个正在运行的浏览器，最近的写入都还在 `-wal` 里，读出来的是过期快照——可能把仍在使用的缓存条目判成可删。

正确处理：

1. 打开前检查同目录是否存在 `<db>-wal` 且非空。
2. 存在则**不使用** `immutable=1`，改为普通只读（`mode=ro`）并接受可能失败。
3. 只读也失败（DB 被独占锁）则跳过该规则，记 `SkipReason::DbLocked`。
4. 任何情况下不得回退成「读到什么算什么」——过期数据比没有数据危险。

Phase 0.5 的 spike 需要实测一次：开着 Chrome 的情况下，这三条路径分别走到哪一条。

**Phase 0.5 实测（Chrome History，本机 2026-07-29）**：无 `-wal` 文件（仅有 `History-journal`）；`immutable=1` 可读（1764 行）；Chrome 运行中 `mode=ro` 报 `database is locked`。策略应按实际 journal 模式分支，不能假设所有浏览器 DB 都有 WAL。

### 5.3 并发与取消模型

**不引入 tokio。** 这里没有真正的异步 I/O，全部是 CPU 与 syscall 密集的文件系统遍历，async 只会增加复杂度。

模型：

- 主线程跑 `crossterm` 事件循环 + `ratatui` 渲染，固定帧率（约 30fps）。
- 扫描 / 清理在后台 `std::thread` 中跑，内部用 `jwalk` 的 rayon 线程池并行。
- 后台 → 主线程用 `crossbeam-channel` 发进度与结果增量，主线程 `try_recv` 非阻塞消费。
- 取消用 `Arc<AtomicBool>` token，遍历的每个目录边界与每次删除前检查一次。要求 Esc 到停止的可感知延迟 < 100ms。

这个决策会渗透进 `scan`、`ops`、`tui` 与 `vole-sys` 的全部接口形态，必须在 Phase 1 定型，Phase 2 落地验证，否则 Phase 3 会推翻 Phase 2 的代码。

### 5.4 错误处理与部分失败

清理的常态是「部分成功」：547 条规则里总有若干因权限、文件锁、路径消失而失败。

- 库 crate 用 `thiserror` 定义域错误；`vole-cli` 顶层用 `anyhow`。
- `Report` 聚合类型 `{ succeeded, skipped_with_reason, failed }` 与 `SkipReason` 枚举（`NeedsPrivilege`、`AppRunning`、`Whitelisted`、`DbLocked`、`PathVanished`、`TccDenied`）住在 `vole-proto`。
- `SkipReason` 的序列化字符串表示在 **Phase 4 结束时随协议一同冻结**（冻结时点见 [5.6](#56-前端边界协议与两阶段模型)），此后新增变体只能追加。桌面 app 会按这些字符串分类展示，改名等于破坏兼容。
- **硬性规则：任何跳过与失败都必须进入 `Report` 并在结束时呈现，禁止静默 `|| true`。** 这是「规则静默失效」这条风险唯一有效的缓解手段。

### 5.5 分发：签名与公证

Gatekeeper 拦的是带 `com.apple.quarantine` 扩展属性的可执行文件，主要来自浏览器下载。通过 `curl` 或包管理器落盘的文件通常不带该属性——这也是 Mole 的 `install.sh` 能工作的原因。

**以下两条早期版本写成了结论，Phase 0.5 已核实**：

- **Homebrew Cask**（预编译二进制）：2026-09-01 起官方 Tap 要求 codesign + notarize；`--no-quarantine` 将移除（[brew#20755](https://github.com/Homebrew/brew/issues/20755)）。
- **Homebrew Formula**（源码构建 / 本地 bottle）：不受 Cask 审计约束，**无强制公证**。
- CLI 若走 Formula 分发无需 $99/yr；若走 Cask 分发预编译 `vole` 则必须 Developer ID + 公证。
- ~~「两份 vole 用同一签名身份才能共享 TCC 授权」——方向上合理；Phase 0.5 本机 ad-hoc 读 Containers 未弹窗，**Developer ID 下的 cdhash 行为待 Phase 1 完整矩阵**。~~ → 2026-07-30 矩阵：Developer ID rebuild 会换 CDHash；同身份稳定的是**同一份已签名 Release 产物**，不是「每次 cargo build」。见 `docs/findings/2026-07-phase1-tcc-devid-matrix.md`。

**能确定的部分**：SwiftUI app 若要在 Mac 外分发，Developer ID 签名 + notarization 是硬要求；app bundle 内嵌的可执行文件必须随 bundle 一起签名与公证。签名身份与 [4.1](#41-tcc完全磁盘访问) 的 TCC 身份是同一件事的两面，因此仍然建议**尽早**走通签名流程（哪怕先用 ad-hoc），不要留到发布前。

上述两条待核实项归入 Phase 0.5 的 spike。

### 5.6 前端边界：协议与两阶段模型

桌面 app 用 SwiftUI，Swift 无法直接 link Rust crate，因此 app 与 vole 之间走**进程边界**。两者同为 GPL-3.0，所以这纯粹是技术选择。

#### 为什么选 sidecar 而不是 C ABI

C ABI（`vole-ffi` 编成 staticlib + cbindgen 生成头文件）技术上可行，但要跨 FFI 传流式进度回调和取消 token：函数指针的生命周期、跨语言线程安全、panic 不得越过 FFI 边界，全都要手工保证，且出错方式隐蔽。而 sidecar 侧 Swift 解析 NDJSON 是几十行的事，取消就是关 stdin 或发 SIGTERM。

**结论**：进度与取消的复杂度决定了选 sidecar。若将来出现必须同进程的需求（如实时性极高的 `status` 面板），再单独为那一条路加 FFI。

#### 不做 daemon

每个操作起一个短生命周期 sidecar 进程，做完即退。理由：macOS 上常驻 daemon 的生命周期管理、socket 权限、TCC 归属都是额外麻烦，而收益（跨操作复用状态）已经被磁盘上的扫描缓存覆盖了。

#### 两阶段模型：plan / apply

`clean` 必须拆成两阶段，因为 GUI 需要让用户勾选：

```
vole clean --plan --json-stream     → 输出候选集，不改动任何文件
vole clean --apply <plan-file>      → 只执行 plan 中被选中的条目
```

`Plan` 条目形如 `{ id, path, label, size, rule_id, skip_reason, dev, ino, mtime }`，`id` 在 plan 内稳定可寻址。后三个字段用于 apply 阶段的身份校验，见下。

这个拆分**不只是为 app**：`--dry-run` 从此等价于「只跑 plan 阶段」，产出的是可机械比对的计划而非人类可读文本，[7A](#a-双跑-diff主力) 的一致性测试因此直接消费同一份输出，不必再另写解析。

#### plan 的威胁模型

把 plan 落成文件、再交给一次独立的 apply 调用，引入了三个 Mole 原来不存在的攻击面。早期版本只写了一句「apply 要重新校验」，不足以指导实现。

**威胁一:plan 是不可信输入。** plan 文件可能被第三方进程改写、可能是攻击者构造的、也可能是用户手工编辑过的。**apply 绝不能把 plan 当作已授权的删除清单。** 具体要求:

- plan 里的 `path` 只是一个候选，apply 必须让它重新走完整的 Phase 4a 安全闸口与保护清单判定，判定结果与 plan 里写的任何东西无关。
- apply 必须重新执行规则匹配，确认该路径确实是某条规则在当前文件系统状态下会产出的候选。plan 里出现规则不可能产出的路径，一律拒绝并报错退出，而不是跳过——这是被篡改的信号，不是数据瑕疵。
- plan 中不存在「已批准」标记之类的信任字段。plan 唯一携带的额外信息是「用户选了哪些 id」。

**威胁二:TOCTOU。** plan 生成到 apply 执行之间存在时间窗口，路径可被换成 symlink、hardlink，或被替换成另一个对象。缓解：

- 遍历与删除全程使用相对目录 fd（`openat` 系列）配合 `O_NOFOLLOW`，不用绝对路径字符串重新解析。`rustix` 提供了这些原语。
- 比对 plan 中记录的 `(dev, ino, mtime)`：不匹配则跳过该条并记 `SkipReason::PathVanished`（语义扩展为「路径已变化」）。
- 拒绝跨设备：目标的 `dev` 与 plan 记录不一致时拒绝，防止路径被指向挂载的外部卷。
- 路径中**任何一段**是 symlink 即拒绝，不只检查末段。这是 symlink 攻击最常见的绕过点。

**威胁三:plan 过期。** 一份几小时前的 plan 反映的是过时的文件系统状态，即便每条都能通过校验，整体决策也可能不再合理（例如用户当时选中的目录现在已是活跃项目）。缓解：plan 带生成时间戳与 TTL（建议 15 分钟），超时 apply 拒绝并提示重新扫描。

**这三条不是可选的加固，而是 plan/apply 拆分的入场费。** 若 Phase 4d 无法完整实现，正确的应对是退回单阶段交互式 `clean`（扫描与删除在同一进程内、不落 plan 文件），而不是先上一个校验不完整的两阶段版本。

#### 事件流协议

NDJSON over stdout，一行一个事件：

```json
{"schema_version":1,"type":"progress","scanned":128340,"current":"~/Library/Caches"}
{"schema_version":1,"type":"candidate","id":"c-0421","path":"...","label":"Chrome cache","size":184320000}
{"schema_version":1,"type":"skipped","rule_id":"system-logs","reason":"NeedsPrivilege"}
{"schema_version":1,"type":"done","report":{"succeeded":412,"skipped":93,"failed":2}}
```

stdout 只放协议，日志一律走 stderr，避免污染。

[5.3](#53-并发与取消模型) 的并发模型不需要改动：后台线程经 `crossbeam-channel` 发出的增量，TUI 前端直接渲染，sidecar 前端序列化成 NDJSON 写 stdout。同一个事件流，两个消费端。

#### schema 版本与冻结时点

`schema_version` 从 1 起，破坏性变更递增，app 启动时校验。协议写进 `docs/protocol.md` 并与实现同步更新。

**冻结时点统一规定为 Phase 4 结束。** 早期版本在三处给了不一致的说法（「一旦发布即冻结」「Phase 1 冻结」「Phase 4 之后冻结」），以本节为准：

| 阶段 | 协议状态 |
|---|---|
| Phase 1 | 定型首版并写入 `docs/protocol.md`。**定型不等于冻结**——目的是让 Phase 2、3 的编排接口按协议长，而不是按 TUI 长 |
| Phase 2–3 | 可自由破坏性修改。桌面 app 尚不存在，唯一的消费者是一致性测试，改动代价近乎为零 |
| Phase 4 结束 | 冻结 v1。此后新增字段只能追加，`SkipReason` 变体只能追加，破坏性变更必须递增 `schema_version` |

### 5.7 废纸篓与「释放空间」的口径

移入废纸篓**不释放磁盘空间**，空间要到废纸篓被清空才真正回收。这件事必须在设计层面讲清楚，因为它同时影响用户预期、报告措辞和与 mole 的数值对齐。

Mole 的 `clean` 输出「Space freed: 95.5GB」，而 `mo analyze` 的删除走 Finder 移废纸篓。Vole 的默认策略也是走废纸篓（见第 8 节 Phase 4 验收第 5 条），因此不能沿用同一句措辞。

**口径规定**：报告必须把两类分开，且不得用「freed」描述废纸篓部分。

```
移入废纸篓   84.2 GB   （清空废纸篓后释放）
永久删除      11.3 GB   （已释放）
因权限跳过    12 条规则
```

对应地，`Report` 需要 `trashed_bytes` 与 `deleted_bytes` 两个字段而不是一个 `freed_bytes`。这正是 [三种契约](#三种契约必须分开) 里「Vole 字段集是 mole 的超集」的一个具体例子：一致性测试比对时，mole 的 `space_freed` 对齐到 `trashed_bytes + deleted_bytes` 之和，而两个分项是 Vole 独有的补充。

附带一个真实边界情况：废纸篓自身也是 mole 的清理目标之一（`clean_trash`）。「把文件移进废纸篓」与「清空废纸篓」在同一次 `clean` 中同时存在时，执行顺序决定结果。**规定：清空废纸篓的规则必须在所有移入废纸篓的操作之前执行**，否则会把用户本次刚移进去、还没来得及复核的文件一并永久删除。这条要落成一个显式的规则排序约束，而不是依赖规则文件的书写顺序。

### 5.8 健壮性基础设施

本节补的三项是早期版本完全遗漏的基础设施。它们不是可选加固——缺任何一项都会在真实机器上产生用户可见的故障。

#### 超时与挂起防护

Mole 有 `lib/core/timeout.sh` 394 行加 `timeouts.sh`，配 `core_timeout.bats` 与四个 expect 脚本，并且被 `lib/core/common.sh` 传递 source 进**每一次** clean 运行。它不是边角功能。

它存在的原因在 Rust 里一个都没消除：

- **网络挂载与停滞卷**：`stat`、`readdir` 在挂了的 NFS / SMB 卷上会无限阻塞。这与语言无关，是 syscall 层面的事。`analyze` 扫 `/Volumes` 时首当其冲。
- **保留的子进程**：`mdfind`、`lsregister`、`tmutil`、`diskutil`、`defaults` 都会挂。[3.5](#35-外部命令依赖) 决定保留它们，就得连它们的挂起风险一起接下来。

规定：

- `SysCommand` trait 的每个方法**必须**带超时参数，没有无超时的重载。超时后杀掉整个进程组（不只是直接子进程），对齐 Mole 用 `gtimeout` 的语义。
- 各类操作的默认超时值集中在一处配置（对应 Mole 的 `timeouts.sh`），不散落在调用点。
- 文件系统遍历用**看门狗线程**而非逐调用超时：单个 `readdir` 无法从外部取消，做法是主扫描线程定期上报进度，看门狗发现某个目录超过阈值无进展就把该子树标记为 `SkipReason::Timeout` 并跳过，不阻塞整体扫描。
- `SkipReason` 需要新增 `Timeout` 变体。
- 超时**必须**进 `Report`，与其他跳过同等对待。静默超时等于静默少清。

#### 信号处理与终端恢复

`bin/clean.sh` 装了 EXIT / INT / TERM 三个 trap，`lib/ui/menu_simple.sh` 还要保存并恢复外层 trap（有专门的 `menu_trap_restore.bats` 守着）。

对 TUI 程序这是硬需求：进入 alternate screen 并开 raw mode 之后，若进程异常退出而没恢复，用户的终端就坏了——看不到输入回显、光标消失，得 `reset` 才能用。

规定：

- 终端状态用 RAII guard 管理（`Drop` 里恢复），而不是在正常退出路径上手工调用。
- 装 panic hook：**先恢复终端，再打印 panic 信息**。顺序反了 panic 信息会显示在 alternate screen 里，随后被清屏吞掉，用户什么也看不到。
- SIGINT / SIGTERM 走统一处理：翻取消 token → 等后台线程收敛（带上限，比如 2 秒）→ 恢复终端 → 按对应退出码退出（130 / 143，对齐 Mole）。
- **SIGINT 到达时若正在执行删除，当前这一个文件操作必须完成再退出**，不能留下半删状态。取消检查点放在两次文件操作之间，不放在操作中间。
- sidecar 模式（`--json-stream`）没有终端状态要恢复，但仍需在退出前 flush 事件流并发一个 `{"type":"aborted"}`，否则前端会看到流无声中断而无法区分崩溃与正常取消。

#### 进程互斥

依赖表里列了 `rustix` 的 `flock`，早期版本却没说锁什么。两处需要：

| 锁 | 目的 |
|---|---|
| 配置与偏好文件写入 | 对齐 Mole `cmd/status/prefs.go` 的做法：两个 `status` 实例同时写会各自读到旧 map，后写者丢掉前者的新 key |
| **全局 `clean` 互斥** | 防止两个 `clean` 并发删除同一批路径。这是 Mole 没做但 Vole 应该做的——并发删除产生的竞态极难诊断，而正当场景不存在 |

`clean` 互斥用 `~/.cache/vole/clean.lock` 加 `flock(LOCK_EX | LOCK_NB)`，拿不到锁就直接报「另一个 vole clean 正在运行」并退出，不排队等待。锁必须是 `flock` 而非 pidfile——进程被 `kill -9` 时 `flock` 由内核自动释放，pidfile 会留下需要人工清理的陈旧锁。

`analyze` 与 `status` 是只读的，不需要互斥。

---

## 6. 规则模型

### 6.1 声明式 schema + 封闭策略集

初版文档提的「纯 TOML glob 列表」不够用，因为 [3.3](#33-规则不是纯声明式的) 显示存在大量过程性逻辑。修正后的模型是三层：

**第一层：声明式字段**（覆盖多数简单规则）

```toml
# rules/browser.toml
[[rule]]
id = "chrome-cache"
category = "browser"
label = "Chrome cache"
platform = ["macos"]
paths = [
  "~/Library/Caches/Google/Chrome/*/Cache",
  "~/Library/Caches/Google/Chrome/*/Code Cache",
]
impact = "Chrome 将在下次启动时重建缓存，不影响登录状态"
```

**第二层：封闭策略集**（覆盖 [3.3](#33-规则不是纯声明式的) 那四类过程性逻辑）

```toml
[[rule]]
id = "claude-code-old-versions"
label = "Claude Code old version"
paths = ["~/.local/share/claude/versions/*"]

[rule.strategy]
kind = "keep_newest_by_mtime"
keep = 1
env_override = "MOLE_AI_AGENTS_KEEP"

[rule.guards]
# 进程在跑则整条跳过
not_running = ["claude"]
# 保护 symlink 指向的目标；symlink 断裂则整条规则跳过（对齐 mole 语义）
protect_symlink_target = "~/.local/bin/claude"
on_broken_symlink = "skip_rule"
```

策略 `kind` 的封闭枚举：`all`（默认）、`keep_newest_by_mtime`、`keep_newest_by_version`、`older_than_days`、`keep_named`（如保留 `current` 目录）。

Guard 的封闭枚举：`not_running`、`protect_symlink_target`、`on_broken_symlink`、`requires_app_absent`、`min_free_space`。

**第三层：逃逸出口**

确实无法用上述表达的规则，用 `kind = "custom"` 指向一个按 id 注册的 Rust 函数：

```toml
[rule.strategy]
kind = "custom"
handler = "xcode_simulator_runtime_volumes"
```

**要求：逃逸出口不得超过全部规则的 5%（约 27 条）。** 超了说明策略集抽象错了，应回头补策略而不是继续加 custom。这条数字是本设计的一个可检验约束。

### 6.2 路径语义必须显式定义

`paths` 字段看起来简单，但如果不把语义写死，Rust 实现与 mole 的 bash glob 行为一定会出现细微分歧，而这类分歧正是删错文件的来源。以下每条都必须在 Phase 4b 落实并被测试覆盖：

| 语义 | 规定 | 与 bash 的差异风险 |
|---|---|---|
| `~` 展开 | 只在路径开头展开，取 `$HOME`。不支持 `~user` | bash 在更多位置展开 |
| `*` | 匹配单层目录内的任意字符，**不跨 `/`** | 与 bash glob 一致 |
| `**` | **不支持。** 需要递归的规则走策略而非通配 | 避免意外匹配整棵子树 |
| 隐藏文件 | `*` **不**匹配以 `.` 开头的条目，需显式写 `.` | 与 bash 默认 `dotglob=off` 一致 |
| 无匹配 | 视为该规则本次无候选，不报错 | bash 默认会把未匹配的 glob 原样保留，导致操作字面量路径——**这是必须规避的行为** |
| 大小写 | 匹配一律**大小写敏感**，即使卷是 APFS 大小写不敏感 | 大小写不敏感卷上 `Caches` 与 `caches` 是同一目录，规则若依赖大小写会静默失效，需在 fixture 中覆盖 |
| symlink | glob 展开**不跟随** symlink；末段是 symlink 时只作用于链接本身，不作用于目标 | bash 行为依上下文而异 |
| 路径规范化 | 在校验**之前**完成，且不得使用会解析 symlink 的规范化（不用 `canonicalize`） | 规范化顺序错会绕过保护清单 |

最后一条尤其重要：如果先做保护清单判定再规范化，`~/Library/../..` 这类路径可能先通过判定再逃出边界。

### 6.3 规则的过期与复核

即使不跟踪上游演进，规则本身也会随 macOS 与应用生态自然过期：路径改名、应用改换存储位置、缓存目录变成不可删的数据目录。**「独立发展」不等于「一次写完不管」。**

机制：

- 每条规则带 `last_verified = "2026-07"` 字段。
- 季度复核一次：抽样验证高影响规则（体积大或涉及数据目录的）路径是否仍然存在且语义未变，更新 `last_verified`。
- 超过 4 个季度未复核的规则，在 `--debug` 输出中标注，提示可信度下降。
- 若某条规则的目标目录从「缓存」变成了「数据」，属于安全事故级别的变更，应有一条快速下架路径（规则加 `disabled = true` 即可，无需改代码）。

`disabled` 字段同时是应急开关：线上发现某条规则误删，改一行数据即可停用，不需要发新版本二进制——前提是规则数据可以独立于二进制更新，这一点需要在 Phase 4b 决定（内嵌 + 允许用户目录覆盖，还是纯内嵌）。

### 6.4 为什么值得数据化

规则可被表驱动测试逐条驱动；加平台只是加 `platform` 值；review 一条新规则是 review 一份数据 diff 而不是读代码；547 条规则的移植进度可以被机械统计；出问题时可以靠 `disabled` 一行下架而不必发版。

---

## 7. 一致性测试策略

Mole 的 1157 个 bats 测试的价值在于**它们已经通过了 mole**。但如 [3.4](#34-bats-测试不是黑盒的) 所述，其中 172 个 clean 类用例依赖 monkey-patch shell 函数，无法直接指向 Rust 二进制。因此分三类处理。

### 7.0 执行环境：不能在开发机上跑

这是早期版本的一个严重遗漏。一致性测试要驱动两个**会删文件的真实程序**，且 mole 的很多规则路径并不完全受 `$HOME` 约束（系统级路径、`/private/var`、废纸篓、`defaults` 域）。在日常开发机上直接跑，等于把自己的机器当靶场。

**要求**：

- 在一次性 macOS 环境中执行。可选方案按代价排序：专用的一次性本地用户账户、macOS VM（Tart / UTM）、或专用物理测试机。
- **容器不可用。** macOS 的 TCC、系统路径、`launchctl`、废纸篓语义都无法在 Linux 容器里复现，用容器等于测了个假东西。
- harness 自身要有护栏：所有 fixture 限定在一个 `VOLE_TEST_ROOT` 下，harness 在每次调用前后断言该根之外没有任何 mtime 变化。护栏失败即中止整个测试运行，不是警告。
- 破坏性用例默认只跑 plan 阶段。真正执行 apply 的用例单独打标记，只在一次性环境里跑，且跑完即回滚快照。

CI 的处理：GitHub Actions 的 macOS runner 是一次性的，适合跑 plan 阶段与 B、C 两类。真正的 apply 用例留给本地一次性环境，不进 CI——这一点要写进 CI 配置的注释里，否则将来有人会顺手把它打开。

### A. 双跑 diff（主力）

构造一次性 `HOME` fixture 树，以该 `HOME` 分别运行 `mole clean --dry-run` 与 `vole clean --plan --json-stream`，比对候选集合。这是发现规则漂移最有效的手段。

**比对维度不只是路径。** 早期版本只写了「路径集合」，那会漏掉一整类分歧：

| 维度 | 为何必须比 |
|---|---|
| 路径 | 基本正确性 |
| 标签 | 标签错了用户就看不懂自己在删什么；也是定位是哪条规则出错的唯一线索 |
| 归属规则 | 同一路径被不同规则命中，说明规则边界画错了 |
| 体积 | 体积算法不一致（是否含硬链接、是否含目录本身、稀疏文件）会让报告数字对不上 |
| 跳过原因 | 「没选中」和「因 X 跳过」是两件事，混在一起会掩盖 guard 逻辑的 bug |

顺序**不比**——集合语义，两边排序不同不算分歧。但集合去重前要先断言两边都无重复项。

Vole 侧不需要为测试做任何特殊支持——[5.6](#56-前端边界协议与两阶段模型) 的 plan 阶段输出就是比对面。这是把 plan/apply 拆分提前到 v1 的附带收益。

**前置工程细节**：mole 的 `clean --dry-run` 只有人类可读输出。做法是给 `third_party` 里的 mole 快照打一个**仅测试用、不发布**的补丁，在 `safe_clean` 里加一个受环境变量控制的 JSONL 输出。这正是 bats 已经在用的覆盖点，改动极小，比写文本解析器可靠得多。

**补丁保真度本身需要被验证。** 打补丁意味着我们在改比对基准，如果补丁改变了 mole 的行为，整套一致性测试的地基就是歪的。要求：补丁只在 `safe_clean` 入口增加一条受环境变量守护的输出语句，不改动任何控制流；并且在补丁前后各跑一次 mole 完整的 bats 套件，确认 1157 个用例的结果无变化。这个验证放在 Phase 0。

### B. 从 bats 抽取 fixture 与期望（覆盖 clean 规则）

172 个 monkey-patch 用例的结构高度一致：`(fixture 树构造) → (期望的 路径|标签 集合，含正向与负向断言)`。把这两部分机械抽取成 JSON：

```json
{
  "id": "clean_dev_ai_agents_keeps_newest",
  "fixture": [
    { "mkdir": "~/.local/share/claude/versions/2.1.112", "mtime": "2026-04-17T08:29" },
    { "mkdir": "~/.local/share/claude/versions/2.1.114", "mtime": "2026-04-18T10:02" }
  ],
  "expect_selected": ["~/.local/share/claude/versions/2.1.112|Claude Code old version"],
  "expect_not_selected": ["~/.local/share/claude/versions/2.1.114"]
}
```

在 Rust 侧做表驱动测试，跑规则引擎、比对候选集合。**负向断言（`expect_not_selected`）比正向更重要**——它们编码的正是「不该删什么」，是多年 issue 换来的。

抽取本身应半自动化：写脚本解析 bats 的 `mkdir -p` / `touch -t` / `[[ "$output" == *...* ]]` 模式，人工校对残余。

### C. 安全语义的移植 + property test

`file_ops_*.bats`、`path_validation_fuzz.bats`、`core_safe_functions.bats`、`uninstall_safety.bats` 是安全语义核心，逐条移植成 Rust 集成测试。`tests/fuzz_corpus` 作为 `proptest` 种子语料。

核心不变量作为 property：**对任意随机路径输入，规则引擎产出的删除目标集合与保护清单的交集恒为空。**

---

## 8. 分阶段计划

排序原则：**先建立能证伪自己的测试能力，再从零风险的只读命令切入，最后碰删除逻辑。**

### Phase 0 — 法务、骨架、一致性框架（1 周）

- 替换 LICENSE 为 GPL-3.0；README 加 Mole 归属声明。
- vendor `third_party/mole-1.48.1/`，记录上游 commit SHA。
- workspace 骨架 + CI：`fmt`、`clippy -D warnings`、`test`、交叉编译 `aarch64-apple-darwin` 与 `x86_64-apple-darwin`。
- 给 mole 快照打测试专用的 `safe_clean` JSONL 输出补丁，并按 [7A](#a-双跑-diff主力) 的要求验证补丁保真度（打补丁前后 bats 套件结果一致）。
- 搭建 [7.0](#70-执行环境不能在开发机上跑) 的一次性测试环境与 `VOLE_TEST_ROOT` 护栏。

**验收**：CI 全绿；harness 能对同一 fixture 分别调用 `mole` 与 `vole` 并输出结构化 diff；护栏能在越界写入时中止运行（用一个故意越界的假用例验证护栏本身有效）。

### Phase 0.5 — 风险 spike（3 天，先于任何工期承诺）

第 10 节的估算建立在若干未验证假设上。用 3 天集中击穿最不确定的几项，再决定是否按该工期承诺。**这一阶段的产出是修正后的估算，不是代码。**

要验证的事项：

| 事项 | 方法 | 影响什么 |
|---|---|---|
| 规则移植的真实速率 | 挑 20 条**代表性**规则（覆盖四类策略、含至少 2 条 custom 候选）走完整流程：写 TOML、过 A 类 diff、过 B 类 fixture | Phase 4c 的 3–5 周区间，即全局最大不确定项 |
| 不可信 plan 的校验成本 | 实现 [plan 威胁模型](#plan-的威胁模型) 里 `openat` + `O_NOFOLLOW` + `(dev,ino,mtime)` 校验的最小版本 | Phase 4d 是否真能在 1 周内做完；若不能则退回单阶段 |
| SQLite WAL 行为 | 开着 Chrome 实测 [三条路径](#sqlite-的-wal-陷阱) 分别走到哪 | 浏览器类规则的可行性 |
| 废纸篓口径 | 实测 `trash` crate 的体积统计与废纸篓清空顺序 | [5.7](#57-废纸篓与释放空间的口径) 的报告设计 |
| TCC 与签名 | 跑 [4.1](#41-tcc完全磁盘访问) 测试矩阵的最小子集（未签名 vs ad-hoc，重编译是否重弹窗） | 开发迭代效率；是否需要立刻买 Developer ID |
| Homebrew 签名要求 | 查证 Homebrew 现行政策 | [5.5](#55-分发签名与公证) 的待核实项 |

**决策点**：若 20 条规则的实测速率外推后 Phase 4c 超过 6 周，或不可信 plan 校验明显超出 1 周，则在进入 Phase 1 之前先调整方案（缩减规则范围，或退回单阶段 `clean`），而不是带着已知偏差往下走。

### Phase 1 — 地基、协议定型与平台实测（3 周）

- 平台 trait：`Fs`、`Plist`、`Sqlite`、`Trash`、`SysCommand`、`Metrics`，`SysCommand` 支持注入桩。**每个方法必须带超时参数**，见 [5.8](#58-健壮性基础设施)。
- **超时基础设施**：集中的超时配置表、进程组级别的超时杀进程、遍历用的看门狗线程、`SkipReason::Timeout`。
- **进程互斥**：配置写入的 `flock`，以及 `clean` 的全局 `flock(LOCK_EX | LOCK_NB)` 互斥。
- `vole-core`：单位格式化（移植 `internal/units` 及其测试）、`whitelist` 配置读写、操作日志（对齐 `operations.log` 格式，支持 `MO_NO_OPLOG=1`）。
- 定型 [5.3](#53-并发与取消模型) 并发模型与 [5.4](#54-错误处理与部分失败) 的 `Report` 类型。
- **`vole-proto` 与 `ops` 骨架**：事件流枚举、`Plan`、`Report` 的 serde **定型**并写入 `docs/protocol.md`。注意定型不等于冻结，冻结在 Phase 4 末，见 [5.6](#56-前端边界协议与两阶段模型)。
- **TCC 实测**：按 [4.1](#41-tcc完全磁盘访问) 的完整测试矩阵执行（Phase 0.5 只跑了子集），结论写回该节。
- **`sysinfo` 磁盘 I/O 实测**：确认 macOS 上读写速率是否可用，否则改走 IOKit。
- CI 加依赖方向检查，固化 [5.1](#51-workspace-划分) 的分层规则。

**验收**：

1. `internal/units` 的 Go 测试 100% 移植通过。
2. 操作日志**写入**格式与 mole 一致。判据是反向验证：**vole 写出的 oplog 能被 `mo history --json` 正确解析**。这样不需要 vole 此时已有 `history` 子命令——它属于 Phase 5（早期版本在此处要求 `vole history --json` 字节一致，与 Phase 5 的排期矛盾）。
3. TCC 实测结论已文档化。
4. `docs/protocol.md` 与 `vole-proto` 一致，CI 依赖方向检查生效。

协议必须在此定型而不是等到有 app 时再补，否则 Phase 2、3 的编排接口会按 TUI 的需要长歪，将来加 sidecar 要回头改所有命令。

### Phase 2 — `status`（2.5 周）

只读、零破坏性风险，是 TUI 与并发模型的练兵场。

- `vole-cli::tui` 主题与基础组件（进度条、卡片、sparkline）。
- **信号处理与终端恢复**：终端状态的 RAII guard、先恢复终端再打印的 panic hook、SIGINT/SIGTERM 统一处理与 130/143 退出码。见 [5.8](#58-健壮性基础设施)。这些必须在第一个 TUI 命令里就做对，否则每个后续命令都会各自踩一遍。
- 指标采集：CPU（含分核）、内存、磁盘、网络、电池（IOKit）、GPU、进程、健康分。
- `--json` 与 `mo status --json` 字段口径对齐；检测到管道时自动切 JSON。
- **落地验证 5.6 的双消费端模型**：同一份指标事件流，分别喂给 TUI 与 `--json-stream`。`status` 是最简单的验证场景，比在 `analyze` 里第一次试要安全得多。

**验收**（修正为可执行的判据）：

1. **mole 的字段集是 Vole 的子集**（键名、嵌套结构、类型对同名字段一致）。不要求相等，理由见 [三种契约](#三种契约必须分开)。
2. **静态字段精确相等**：逻辑核数、总内存、磁盘总容量、机型、macOS 版本。
3. **瞬时字段只校验存在性与范围**：CPU/内存/磁盘使用率落在 `[0, 100]`，网络速率非负。不做跨进程数值比对——两次采样时间不同，本质不可对齐。
4. 健康分：把 `metrics_health_test.go`（346 行）里的输入输出对抽成 JSON fixture，Rust 侧表驱动测试全过。
5. **终端恢复**：三种异常退出路径（panic、SIGINT、SIGTERM）之后终端均可正常使用（回显、光标、raw mode 全部复原），且 panic 信息可见而非被清屏吞掉。用 expect 脚本自动验证，对齐 Mole 的 `timeout_tty_restore.exp` 思路。

### Phase 3 — `analyze`（3 周）

- `vole-core::scan`：`jwalk` 并行遍历、硬链接按 inode 去重、默认跳过 `/Volumes`、扫描缓存（对齐 `cache.go` 803 行的失效语义，键用 `twox-hash`）。按 [5.1](#51-workspace-划分) 的阈值，这里很可能是第一个达标拆成独立 crate 的 module。
- TUI：目录下钻、大文件榜、预览、移废纸篓。
- `--json` 对齐 `mo analyze --json`。

**验收**：

1. 同一目录树下 `vole analyze --json` 的 `total_size` / `total_files` 与 `mo analyze --json` 完全相等。体积口径需显式对齐：是否含目录 inode 自身、硬链接是否只计一次、稀疏文件按表观还是实占。
2. 性能：用 `conformance/fixtures/perf-tree`（脚本生成的固定合成树，约 50 万文件）做基准，`hyperfine --warmup 3 --runs 10` 取中位数。**不用真实 `$HOME` 也不做冷缓存测量**——macOS 上无法可靠清空 vnode cache，那样的数字不可复现。

   指标不只是墙钟时间。三项都要记录，且后两项是硬约束而非参考：

   | 指标 | 要求 |
   |---|---|
   | 墙钟时间（中位数） | 目标 ≤ mole 的 50%，未达成不阻塞发布，记为待优化 |
   | 峰值 RSS | **必须 ≤ mole**。扫描 50 万文件时内存膨胀是真实风险（`jwalk` 的并行缓冲） |
   | 交互延迟 | **TUI 帧间隔 p99 < 50ms**。并行度调高能压墙钟时间，但会抢 I/O 导致界面卡顿，这个权衡必须由指标而非手感来定 |

   50% 是目标不是承诺。若为达成它需要把并行度调到损害交互延迟或内存的程度，则以后两项为准，接受更慢的墙钟时间。
3. Esc 到扫描停止的延迟 < 100ms。

**注意**：本阶段主要工作量是把 Bubbletea 的 Elm 式 `update.go`（1232 行）重构成 ratatui 立即模式渲染循环，不是翻译。这是最容易被低估的一步。

### Phase 4 — `clean`（8–10 周，风险最高）

工期按 547 条规则而非 108 条估算。拆成四个子阶段：

**4a 安全闸口与应用保护层（2.5 周）** — 移植三部分：`validate_path_for_deletion`、`mole_delete`，以及 `app_protection.sh` + `app_protection_data.sh` 共 2357 行的保护判定与数据（见 [3.6](#36-clean-的真实依赖闭包)）。用 `fuzz_corpus` + `proptest` 攻击。**在此通过前不写任何规则。**

保护清单按 6.1 的思路数据化成 TOML，与规则数据同样可 diff、可测试。1766 行判定逻辑里真正的分支不多，主体是模式匹配，抽成数据后 Rust 侧的实现量远小于行数。

保护清单本身按 [6.1](#61-声明式-schema--封闭策略集) 的思路数据化成 TOML，与规则数据同样可 diff、可测试。1766 行的判定逻辑里真正的分支不多，主体是模式匹配，抽成数据后 Rust 侧的实现量远小于行数。

**4b 规则引擎与策略集（1.5 周）** — 实现 6.1 的三层模型与全部策略、guard，落实 6.2 的路径语义；用 C 类 property test 验证保护清单不可越过。

**4c fixture 抽取与规则移植（4–6 周）** — 半自动抽取 172 个 bats 用例成 JSON fixture；**首批移植 Top 100–150 条**（按释放空间排序），每批过 A 类双跑 diff 与 B 类表驱动测试。Phase 0.5 外推全量 547 条约 19.5 周，故 v1 收缩范围；其余在报告中提示继续用 Mole。

**4d plan/apply、交互与报告（1 周）** — 落地 [5.6](#56-前端边界协议与两阶段模型) 的两阶段与完整的 plan 威胁模型；`--dry-run` 等价于只跑 plan；`--whitelist` 交互管理；按 [5.7](#57-废纸篓与释放空间的口径) 的口径呈现 `Report`。

**验收**：

1. 全部 547 条规则在 fixture 树上，与 mole 的 dry-run 在 [7A](#a-双跑-diff主力) 的全部五个维度（路径、标签、归属规则、体积、跳过原因）一致。
2. 172 个抽取出的 bats 用例全过，负向断言零失败。
3. property test：任意随机路径输入下，删除目标与保护清单的交集恒为空。
4. `custom` 逃逸出口 ≤ 27 条（全部规则的 5%）。
5. 默认走废纸篓，permanent delete 需显式开关；报告按 [5.7](#57-废纸篓与释放空间的口径) 区分 `trashed_bytes` 与 `deleted_bytes`，不用「freed」描述废纸篓部分。
6. 清空废纸篓的规则排在所有移入废纸篓操作之前，有专门用例验证。
7. 因权限跳过的规则全部出现在报告中，无静默跳过，且不编造体积估算。
8. **plan 威胁模型的三类攻击各有用例且全部被拒绝**：
   - 篡改 plan 加入规则不可能产出的路径 → apply 报错退出（非跳过）；
   - plan 生成后把目标换成 symlink（末段与中间段各一例）→ 拒绝；
   - plan 超过 TTL → 拒绝并提示重新扫描。
9. [6.2](#62-路径语义必须显式定义) 的八条路径语义各有测试覆盖，含一条大小写不敏感卷上的用例。

### Phase 5 — 收口

交互菜单、`completion`、`history` 子命令；Developer ID 签名与 notarization；Homebrew formula 与 `install.sh`；`docs/protocol.md` 定稿。

**协议状态（2026-07-29）：** NDJSON / Plan / Report 已在 `docs/protocol.md` 标注 **FROZEN**（Phase 4 结束后冻结，见 [5.6](#56-前端边界协议与两阶段模型)）。History JSON 为独立附录，不属于 `StreamEvent`。

桌面 app 本身不在本计划范围内。Phase 5 结束时 app 所需的一切（`ops` 编排、`vole-proto` 协议、plan/apply、`--json-stream`、签名身份）均已就绪，app 可作为独立项目启动。

---

## 9. 风险登记

| 风险 | 影响 | 缓解 |
|---|---|---|
| 删错用户文件 | 灾难性，且会立刻毁掉项目信誉 | 4a 先做安全闸口再写规则；property test 保护清单不可越过；默认走废纸篓；dry-run 优先；规则可用 `disabled` 一行下架 |
| 一致性测试在开发机上误删真实文件 | 自伤，且会让人不敢再跑测试 | [7.0](#70-执行环境不能在开发机上跑) 的一次性环境 + `VOLE_TEST_ROOT` 护栏；apply 类用例不进 CI |
| plan 被篡改或 TOCTOU 导致越权删除 | 与「删错文件」同级，但攻击面是自己引入的 | [plan 威胁模型](#plan-的威胁模型) 三条缓解；4d 若做不完则退回单阶段而非降低校验 |
| 补丁后的 mole 不再是可信基准 | 整套一致性测试地基歪掉，且不易察觉 | 补丁只加一条守护输出、不改控制流；打补丁前后跑完整 bats 套件对比 |
| TCC 身份变化导致反复弹窗或读不到目录 | 开发迭代受阻；`clean` 静默少清 | Phase 0.5 先测最小子集，Phase 1 跑完整矩阵；尽早固定签名身份 |
| 规则随 macOS / 应用演进静默过期 | 清理效果衰减；极端情况下缓存目录变数据目录 | [6.3](#63-规则的过期与复核) 的 `last_verified` 与季度复核；`disabled` 应急开关 |
| 547 条规则的移植与验证超期 | 主要工期风险，4c 的 3–5 周区间就是它 | 分批移植，每批闭环验证；按第 10 节判据在 Phase 4 中期重估 |
| 规则模型抽象不足，`custom` 泛滥 | 数据化的好处全部丧失，退回 547 个 Rust 函数 | 5% 硬上限作为可检验约束；超限即回头补策略 |
| `analyze` 的 Elm → 立即模式重构被低估 | 延期 2–3 周 | Phase 2 用 `status` 先摸熟 ratatui 与并发模型 |
| 浏览器 SQLite 读到 WAL 之前的过期快照 | 比读不到更危险——可能把在用的条目判成可删 | [5.3 前的 WAL 处理三步](#sqlite-的-wal-陷阱)；失败进 `Report` 的 `DbLocked`，禁止当作「无可清理」 |
| crate 边界画错导致反复搬文件 | 隐性拖慢全程 | 起步只 4 个 crate；按明确阈值再拆；CI 固化依赖方向 |
| 网络挂载或子进程挂起导致 `clean`/`analyze` 永久卡死 | 用户看到程序假死，无法中断 | [5.8](#58-健壮性基础设施) 的超时基础设施：`SysCommand` 强制超时、遍历看门狗、`SkipReason::Timeout` |
| TUI 异常退出未恢复终端 | 用户终端不可用，需 `reset` | RAII guard + panic hook 先恢复再打印；Phase 2 用 expect 脚本自动验证三种退出路径 |
| 两个 `clean` 并发删除同一批路径 | 难诊断的竞态，且正当场景不存在 | `flock(LOCK_EX \| LOCK_NB)` 全局互斥，拿不到锁直接退出而非排队 |
| 继续漏算 Mole 的隐性依赖 | 工期已因此两次上调 | 第 10 节的修订轨迹显示偏差方向一致向上；Phase 0.5 spike 的一项产出就是核对 clean 依赖闭包是否还有遗漏 |
| 并发模型定得晚 | Phase 3 推翻 Phase 2 的代码 | Phase 1 定型，Phase 2 落地验证 |
| 协议按 TUI 需要长歪，将来加 sidecar 要改所有命令 | 返工涉及三个命令 | Phase 1 就定型 `vole-proto`；Phase 2 用 `status` 同时验证 TUI 与 `--json-stream` 两个消费端 |
| `schema_version` 冻结后发现设计错误 | app 兼容性破裂 | Phase 2、3 各有一次调整窗口（app 尚不存在，破坏无代价）；Phase 4 之后才真正冻结 |
| IOKit / objc2 的 unsafe 面 | 崩溃或 UB | unsafe 只在 `vole-sys`，其余 crate `#![forbid(unsafe_code)]` |
| 跨平台边界过度设计 | 白花工作量 | 只留 trait，不写假桩；非 macOS target `compile_error!` |

---

## 10. 工作量估算与止损判据

### 估算

3.5 万行 Bash + 1.4 万行非测试 Go 折算成 Rust 约 **14–20k 行**，加规则数据约 3–4k 行 TOML。

**估算的前提假设**（不成立则整表失效）：

- 单人开发，每周 4 个有效工作日投入（其余时间给意外与上下文切换）。
- 表中数字是**净开发时间，不含 buffer**。按经验应在总数上另加 20–30% 应对未知，即真实预期落在 20–23 周而非 16–18 周。
- 开发者已熟悉 Rust，但**未必**熟悉 ratatui 与 objc2/IOKit 绑定。
- 一次性测试环境已就绪（Phase 0 内完成）。

| 阶段 | 净工期 |
|---|---|
| Phase 0 骨架与 harness | 1 周 |
| Phase 0.5 风险 spike | 0.5 周 |
| Phase 1 地基、协议定型、超时与互斥、平台实测 | 3 周 |
| Phase 2 `status`（含信号与终端恢复） | 2.5 周 |
| Phase 3 `analyze` | 3 周 |
| Phase 4 `clean`（4a 含 2357 行保护层；4c 收缩至 Top 100–150 条） | 9–11 周 |
| **净合计（Phase 0–4）** | **19–21 周** |
| **含 25% buffer 的预期** | **24–26 周** |

Phase 0.5 spike 已校准：4c 全量外推不可行，收缩后 4c 回到 4–6 周；净合计上调 1 周反映 4c 工具化与采集脚本成本。

估算的修订轨迹（每一次都是漏算而非范围变化，值得记住这个偏差方向）：

| 版本 | 净工期 | 修正原因 |
|---|---|---|
| 初版 | 10–11 周 | 基于「108 条规则」的错误前提 |
| 第二版 | 15.5–17.5 周 | 规则实为 547 条；加桌面 app 的协议层 |
| 当前 | **18–20 周** | 漏算 `app_protection` 2357 行；漏算超时子系统、信号与终端恢复、进程互斥 |
| Phase 0.5 后 | **19–21 周** | 4c 全量外推 ~19.5 周 → v1 收缩至 Top 100–150 条；plan/apply 保留（TOCTOU 原型 2h） |

为桌面 app 增加的净成本约 **1 周**：Phase 1 加 0.5 周（协议定型与 `docs/protocol.md`）、Phase 4d 加 0.5 周（plan/apply 落地）。编排逻辑的抽取不计入，因为一致性测试本来就需要它。

Phase 0–3（只读的 `status` + `analyze`）净 **10 周**，含 buffer 约 12.5 周，能交付一个安全、比 mole 快、且不会删任何东西的可用工具。这是天然的止损点。

置信度：Phase 0–2 较可靠；Phase 3 取决于 ratatui 经验；Phase 4c 是最大不确定项，区间宽达 2 周，且这个区间本身就是 Phase 0.5 要收窄的对象。

### 止损判据

- **Phase 0.5 结束时**：若 20 条规则外推后 Phase 4c 超过 6 周，或不可信 plan 校验明显超出 1 周，先调整方案再进 Phase 1。
- **Phase 2 结束时**：若实际耗时超估算 50%（即超过 3 周），按同比例上调 Phase 3、4 并重新决定是否继续。
- **Phase 4c 中期**（约移植 200 条规则时）：按实测速率外推剩余工期。若外推总工期超 24 周，收缩方案是**只移植 Top 100 条按释放空间排序的规则**，其余在报告中提示「这些类别请继续用 mole」。
- **任何时刻**：若 `custom` 逃逸出口超过 5%，暂停移植，回头修策略集。
- **兜底**：Phase 0–3 完成即为可独立交付的产品。若 Phase 4 判定不划算，`vole` 作为只读工具发布，`clean` 继续用 mole，也是一个诚实且有价值的结果。

---

## 11. 开放问题

以下事项尚未决策，需要在对应阶段前确认。

1. **签名身份**。SwiftUI app 对外分发是硬要求；CLI 走 Homebrew **Formula** 无强制公证，**Cask** 需 Developer ID。**Phase 1 前申请 Developer ID（$99/yr）**。
2. **`sysinfo` 是否够用**。macOS 磁盘 I/O 速率支持待实测；不够则需自己写 IOKit 绑定，Phase 2 加约 3 天。
3. **`defaults` 的 17 次调用**。v1 保留子进程，但若 TCC 或性能实测显示有问题，需提前迁到 `CFPreferences*` API。
4. **规则数据能否独立于二进制更新**。纯内嵌（`include_str!`）最简单，但 `disabled` 应急开关就必须发版才能生效；允许用户目录覆盖则应急更快，代价是多一条不可信输入路径需要校验。**Phase 4b 前决定**，见 [6.3](#63-规则的过期与复核)。
5. **规则优先级排序依据**。Phase 4c 已触发收缩方案；「Top 100–150 条」按真实机器上实测的释放空间排序。**Phase 4c 启动前需有采集脚本。**
6. **vole 二进制如何随 app 分发**。内嵌进 app bundle（两者同为 GPL，法律上无障碍），还是要求用户先装 Homebrew 版？内嵌的好处是签名身份与 TCC 授权可控，代价是同一台机器上可能有两份 vole，版本不一致时协议兼容性靠 `schema_version` 兜。**需在 app 项目启动前决定，不影响 v1。**
7. **`status` 是否需要同进程 FFI**。实时面板走 sidecar 会有进程间延迟。若 Phase 2 实测发现 NDJSON 往返对刷新率有可感知影响，可能需要单独为 `status` 加一条 C ABI 路径。**Phase 2 实测后回答。**

### 已关闭的问题

- ~~`--json` schema 是否允许扩展~~ → 已决策为「mole 的字段集是 Vole 的子集」，见 [三种契约](#三种契约必须分开)。
- ~~Homebrew 是否要求签名与公证~~ → Cask 2026-09 起强制；Formula 豁免。见 [5.5](#55-分发签名与公证) 与 `docs/findings/2026-07-spike-platform.md`。
- ~~TCC 授权的粒度（最小子集）~~ → 本机 ad-hoc 未弹窗；完整矩阵与 Developer ID 行为留 Phase 1。见 `docs/findings/2026-07-spike-platform.md`。
