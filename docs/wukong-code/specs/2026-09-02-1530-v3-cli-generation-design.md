# Vole 产品 v3：CLI 代际规格（体验 / 增值 / 分发）

- 日期：2026-09-02 15:30
- 状态：已批准（会话 brainstorming：轨 2+3+4 · 并行 A · 结构 A · §1–§3 ok）
- 性质：代际规格；**本文件不开实现**、不 bump 包版本、不发版、不开桌面 PR
- 基线：`main` @ **2.19.1**；Mole 钉版 `third_party/mole-1.48.1`
- 文档结构：一份代际规格（本文件）+ 日后三份独立实施计划（本文件不写 plan 正文）
- 对照：[`2026-07-30-1900-v2-product-goals-design.md`](2026-07-30-1900-v2-product-goals-design.md)；[`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md)；[`2026-08-14-0228-worktree-design.md`](2026-08-14-0228-worktree-design.md)；[`2026-08-08-2143-v2-m6-clean-hints-design.md`](2026-08-08-2143-v2-m6-clean-hints-design.md)

## 1. 结论

产品 **v3** 是 CLI 代际，不是包 MAJOR。Mole 家庭桶对齐与 TUI 主路径已在 **2.19.1** 收口之后，北极星改为：把 vole 做成「更好用的 CLI + 自有增值 + 现有渠道可找到」。

三轨并行，**谁先合入谁先发 MINOR**，不绑一次大发版：

| 轨 | 交付 |
|---|---|
| **2 体验** | hints 长尾、可发现性、status 已接线键抛光；人读文案诚实 |
| **3 增值** | 新顶层 `vole agent`；Home 第 7 项 |
| **4 分发** | README / 安装 / tap Formula / 文档截图与命令面对齐 |

桌面（原选项 1 / vole-macos 追上 CLI）**本代际不排期**。禁止注册 `vole hints` / `Command::Hints`。包版本继续 SemVer MINOR（从当前 **2.19.1** 起）；**不因「v3」发 `3.0.0`**。

成功标准不定「三轨同时完工」。每一轨有一个可发版切片合入 `main` 即算该轨起步成功。

## 2. 已锁定决策

| 项 | 结论 |
|---|---|
| 代际名称 | 产品 v3（CLI） |
| 北极星 | Mole 对齐已完成后，把 vole 做成「更好用的 CLI + 自有增值 + 现有渠道可找到」 |
| 推进方式 | 并行 A：三轨真正并行，谁先合入谁先发 MINOR，不绑大发版 |
| 文档结构 | 结构 A：一份代际规格（本文件）+ 日后三份独立实施计划 |
| 轨 2 深度 | 安全长尾 + 文案：hints 未做项、可发现性、status 已接线键抛光；不做 analyze 删除、不做动画 cat |
| 轨 3 第一条 | Agent 工作区卫生；新顶层 `vole agent`；`worktree` 仍只管整棵 Git checkout |
| 轨 3 Home | 第 7 项 `Agent`，数字键 `7`，exec 式 `["agent"]`；1–6 不变 |
| 轨 4 深度 | 现有渠道抛光；**不冲** Homebrew core |
| 桌面 | 原选项 1 / vole-macos 追上 CLI：**本代际不排期** |
| 包版本 | 产品话术「v3」= 代际；从 **2.19.1** 起继续 SemVer MINOR；不因「v3」发 `3.0.0` |
| `schema_version` | 默认不 bump；本文件不授权 bump。轨 3 若发现协议缺字段，另开补丁 design |
| 删除 | 只走既有保护 + 废纸篓漏斗；禁止第二条删除路径 |
| `hints` | 仍挂在 `clean` 只读提示；**禁止** `vole hints` / `Command::Hints` |
| 本文件授权 | 写规格；**不开实现**、不 bump 包版本、不发版、不开桌面 PR |

## 3. 目标与非目标

### 3.1 北极星

Mole 对齐已完成后，把 vole 做成「更好用的 CLI + 自有增值 + 现有渠道可找到」。桌面另说。

### 3.2 必做（三轨并行，谁先合谁先发 MINOR）

| 轨 | 目标 |
|---|---|
| 2 体验 | hints 长尾、可发现性、status 已接线键抛光；人读文案诚实 |
| 3 增值 | 新顶层 `vole agent`：Agent 容器 / 会话 / 缓存残留 → 确认 → 废纸篓；Home 第 7 项 |
| 4 分发 | README / 安装 / tap Formula / 文档截图与命令面对齐 |

### 3.3 明确不做（本代际）

- 桌面追上 CLI（轨 1 / vole-macos）；本代际不排期、不开桌面 PR
- analyze 删除、analyze Space 多选删、动画 cat、冲 Homebrew core
- 扩 `worktree` 去管非 checkout
- `clean --apply` 删本地快照
- 删 `/Library/Updates`、`/macOS Install Data`
- 第二条删除路径
- 无必要不 bump `schema_version`
- 禁止注册 `vole hints` / `Command::Hints`
- `optimize --whitelist`、status 的 `k` / `c` 键与动画 cat（T8 未交付项本代际继续不做）

### 3.4 成功标准

不定「三轨同时完工」。每一轨有一个可发版切片合入 `main` 即算该轨起步成功。代际文档只约束共享规则，不绑大发版。

「可发版切片」钉死为：该轨至少一条用户可见行为（或轨 4 的文档 / Formula / 截图）已合入 `main`，且能按仓惯例单独打一版 MINOR（是否真的打 tag / 建 Release **仍须另问用户**，本文件不授权发版）。

## 4. 三轨范围与命令契约

### 4.1 轨 2 — CLI 体验（不改删除语义）

#### hints 长尾

仍挂在 `clean` 只读提示，依据 [`2026-08-08-2143-v2-m6-clean-hints-design.md`](2026-08-08-2143-v2-m6-clean-hints-design.md) §2.2 长尾。**禁止**顶层 `vole hints`。

本轨补齐 M6 未做的两条探针，并赋予稳定 `kind`（写入既有可选 `hints` 数组；空则省略；**不 bump** `schema_version`）：

| `kind` | Mole 对照 | 行为 |
|---|---|---|
| `launch_agents` | `show_user_launch_agent_hint_notice` | LaunchAgent / MachServices / bundle 归属只读提示 |
| `orphan_dotdirs` | `show_orphan_dotdir_hint_notice` | GUI app / claude plugin 点目录只读提示 |

超时、权限、IO 失败 → 跳过该探针，**不堵** clean 的 plan / apply。墙钟预算沿用 M6：默认 **15s**（`VOLE_TIMEOUT_HINT_SCAN_SEC`）；超预算记 skip / truncated，有命中标 partial。无命中且未 skip → **不打印**；仅当该探针 `scan_skipped` 时打一行 skip。`--apply` 仍不跑 hints。

人读用简洁 ASCII，不复刻 Mole spinner / 全量彩色图标。

#### 可发现性

`H Helper` / `--help` / README 的命令清单以**当时已合入 `main` 的顶层命令**为准，必须列全、不得漏已发布命令。

`agent` 的处理钉死如下，避免把规格里的命令面登记写成「已经发布」：

- **本规格**预先登记 `vole agent`（实现前即可引用该命令名）。
- **轨 3 合入前**：`H Helper` / `--help` **不得**出现 `agent`（二进制里还没有该子命令）；README 五语 **不得**把 `agent` 写成已发布、已可用、或 Features 表已勾选。
- **轨 3 合入后**：`--help` / `H Helper` 必须列出 `agent`；README 由轨 4 跟一版，写成已发布。

#### status 抛光

只整理**已经接线**的 footer / 人读文案，去掉过时或误导措辞。不接 `k` / `c`，不做动画 cat。footer **禁止**虚标未接线键。

#### 轨 2 不做

analyze 删除 / Space 多选删、`optimize --whitelist`、动画 cat。

### 4.2 轨 3 — `vole agent`

新顶层命令；vole 自有增值（Mole 钉版无此子命令）。`scripts/check-command-surface.sh` 的 Mole required 集合 **不**加入 `agent`。脚本增加正向探测：vole 源码枚举含 `Command::Agent`，help 文本含 `agent`，且不把该命令当 Mole 缺口——与 [`worktree`](2026-08-14-0228-worktree-design.md) 同模式。`--enforce` 仍只强制 Mole required。

| 项 | 契约 |
|---|---|
| 命令 | 新顶层 `vole agent`；不进 Mole required 集合 |
| 对象 | Agent 容器 / 会话 / 缓存残留；**不是**整棵 Git checkout（那是 `worktree`） |
| 漏斗 | 与 purge / uninstall / worktree 同一套 ProtoPlan：`--plan` / `--apply` / TTY 多选 |
| 默认勾选 | **全不选**（与 worktree 同，与 purge 相反） |
| 去向 | 默认废纸篓；`--permanent` 仅配合 `--apply` 或交互确认 |
| 硬话术 | **不宣称可安全删除**；负向阻塞可标，正向「过期」不可证 |
| Home | 第 7 项 `Agent`，数字键 `7`，exec 式启动 `["agent"]`；1–6 不变 |
| 发现范围 | 约定根（Cursor / Codex / Claude），**禁止**整盘 `$HOME` 深扫 |
| 硬排除 | 当前 cwd 所在目录；仍被 `worktree` 当作 checkout 的路径不进 `agent` 列表（去重，不双删） |
| 编排 | `vole-core::ops`；`vole-cli` 薄前端；**不**进 clean TOML |

#### 4.2.1 CLI 面

| 调用 | 行为 |
|---|---|
| TTY 裸 `vole agent` | 扫描 → 分页多选（默认全不勾选）→ `Proceed? [y/N]` → 废纸篓 |
| 非 TTY，或 `--plan` / `--dry-run` / `-n` / `--json` / `--json-stream` / `--plan-out` | 只出候选，不删 |
| `--apply PLAN` | TTL + TOCTOU 后再删 |
| `--permanent` | 仅与 `--apply` 或交互确认一起：永久删，不进废纸篓 |

Shell 补全随 clap `Command::Agent` 自动生成。

#### 4.2.2 发现范围（封闭允许表）

只扫下列**已点名**根；目录不存在则跳过。禁止把整个 `$HOME` 当无界根深扫，禁止通配任意 `~/foo/agents`，本代际不发现 Conductor 式更深容器，也 **不**扫 OpenCode 或其他未点名产品。

| 根 | 扫什么 | 不扫 |
|---|---|---|
| `$HOME/.cursor/` | 会话、缓存、非 checkout 容器残留 | `worktrees/` 及其中任何 Git checkout |
| `$HOME/.codex/` | 同上 | `worktrees/` 及其中任何 Git checkout |
| `$HOME/.claude/` | 同上 | `worktrees/` 及其中任何 Git checkout |
| 已被 purge / worktree 搜索根发现的仓库下 `<repo>/.cursor/`、`<repo>/.claude/` | 非 checkout 残留 | `<repo>/.worktrees/`、`<repo>/.claude/worktrees/` 及任何含 `.git` 的 checkout |

「会话 / 缓存 / 容器残留」钉死为这三类，全部进同一张候选表，用 `rule_id` 区分，不拆成三个子命令：

| 形态 | `rule_id` | 含义 |
|---|---|---|
| 容器残留 | `agent:container` | 约定根下的工作区 / 项目容器目录，且 **不是** Git checkout |
| 会话残留 | `agent:session` | 对话 / transcript / 会话状态文件或目录 |
| 缓存残留 | `agent:cache` | 可重建的缓存目录或文件 |

具体 basename / 子路径表由轨 3 实施计划按上表穷举；本代际不得把允许根扩到三家之外。子路径枚举不得改写本节的对象边界、硬排除或去重规则。

扫描与计量总墙钟默认 **15s**（`VOLE_TIMEOUT_AGENT_SCAN_SEC`）；单根超时 **2s**。超时则跳过该根，fail-closed，不当成可删。禁止无界 `du`。

#### 4.2.3 硬排除（plan 与 apply 都必须执行）

下列路径 **不进列表**；若被塞进 plan，apply **拒绝删除**（skip，不当成功）：

1. **cwd**：候选规范化绝对路径等于当前进程 cwd，或 cwd 落在该候选内部（删掉会拆掉自己脚下）。
2. **worktree checkout**：`vole worktree` 会列为 `linked` / `orphan-dir` 的路径（含 `.git` 文件或目录的 checkout）。同一规范化绝对路径只许一个命令认领：**checkout 归 `worktree`，其余归 `agent`**。
3. 既有删除保护层拒绝的路径（`validate_path_for_deletion`、Cleanup `AppProtection` 等）；apply 不得绕过。

#### 4.2.4 列表、话术、阻塞项

- 只排序、不隐藏；启发式把更像堆积的排前面（体积大、mtime 旧优先；阻塞项不参与排序，只染色）。
- UI 与 JSON **不得**出现 `safe`、`deletable`、或「可安全删除」。
- 负向阻塞可标（例如权限未知、目录被占用、探测超时 → `status-unknown`，fail-closed）。
- 禁止把「mtime 超过 N 天」「目录很干净」当作「过期 / 可删」证据。

#### 4.2.5 Plan / apply

- `schema_version`：**不 bump**（本文件不授权）。复用现有 Plan / 事件字段。
- `ttl_secs`：`900`（与 worktree 同档）。
- `id`：`agent:{kind}:{canonical-path}`（稳定；apply 用 path 定位）。
- `rule_id`：仅 `agent:container` / `agent:session` / `agent:cache`。
- 独立 `apply_agent_plan`：只接受 `rule_id` 前缀 `agent:`；其它前缀 **逐条 skip** 并计入 skipped。
- 删除：**只**走既有 `mole_delete_verified`。禁止平行 `rm -rf`。
- 默认 `DeleteMode::Trash`；`--permanent` → Permanent。
- 互斥锁：`try_lock_agent()` → `try_lock_config("agent")`（与 worktree 同形）。
- oplog：`command = "agent"`。
- 有目录的条目：plan 阶段 `capture_plan_entry_identity`；apply 做 TOCTOU，身份变化 → skip。

#### 4.2.6 Home 第 7 项

故意继续偏离 Mole 首页五项：第 6 项 Worktree 已是增值先例，第 7 项同样是增值入口，不是 Mole 缺口。README 必须写明（由轨 4 在第 7 项落地后跟）。

| 键 | 项 | 描述 |
|---|---|---|
| 1 | Clean | 不变 |
| 2 | Uninstall | 不变 |
| 3 | Optimize | 不变 |
| 4 | Analyze | 不变 |
| 5 | Status | 不变 |
| 6 | Worktree | 不变（`Remove leftover git worktrees`） |
| 7 | Agent | `Remove leftover agent data` |

- `HOME_ITEMS` 从 6 扩到 7；`HomeCommand::Agent`；`argv()` → `["agent"]`
- 数字键 `1..=7`；光标下标 `0..=6`；`map_key` 接受 `'1'..='7'`
- Enter / `7` 与 CLI 裸 `vole agent` 同一条交互（扫描 → 默认全不勾选 → 确认）
- **不**把 `purge` / `installer` 一并塞进 Home
- 测试：前五项文案仍对齐 Mole，第六项仍为 Worktree，第七项为 Agent
- `images/tui/home.png` 由轨 4 在第 7 项合入后更新（不阻塞轨 3 代码合入）

### 4.3 轨 4 — 现有渠道

- README **五语**同步：`README.md`、`README.zh-CN.md`、`README.zh-TW.md`、`README.ja.md`、`README.ko.md`
- 安装 AI prompt、tap Formula 版本 / sha、`docs/releases/`、TUI 截图与 Home 7 项对齐
- **不冲** Homebrew core
- 不改公证 / 签名流水线语义（Developer ID / notary 步骤与现 workflow 保持一致；本轨只跟版本号、sha、文档）
- 轨 4 只动文档与 Formula，不进 `vole-core`
- Formula 的 version / sha 变更仍走既有发版确认规则；本文件不授权打 tag 或建 GitHub Release

轨 4 可随时开工（修正过时截图、安装步骤、五语文案）。命令面尚未落地的能力按 §4.1 诚实规则书写，不得预写成已发布。

### 4.4 共享接口（三轨都遵守）

- 编排在 `vole-core::ops`，UI 在 `vole-cli`
- 删除只走既有保护 + 废纸篓漏斗
- 不 bump `schema_version`（除非轨 3 实现时发现协议缺字段——另开补丁 design，本文件不授权）
- 冲突文件串行：`home_menu_state.rs`、`check-command-surface.sh`、`coverage.rs`、README

## 5. 架构与并行规则

```
vole-cli          薄前端：Home / TTY 多选 / 人读 hints / status footer
vole-core::ops    编排：agent plan/apply、hints 探针、既有删除漏斗
vole-proto        复用现有 Plan / 事件；默认不 bump schema_version
vole-sys          仅复用已有 trash / 路径 / 权限原语；不新开特权通道
```

- `agent` 复用 worktree / purge 的 ProtoPlan + TTL / TOCTOU apply，**不**进 clean TOML 规则。
- hints 仍是 `clean` 计划后的只读模块，失败 / 超时静默或一条 skip，永不挡 apply。
- 轨 4 只动文档与 Formula，不进 `vole-core`。

### 5.1 并行规则

1. **协议与删除漏斗冻结**：不 bump `schema_version`；不新开第二条删除路径。
2. **命令面先登记再实现**：新子命令 / Home 项以本规格为准再开实现 PR，避免 2 和 3 同时改 `home_menu_state.rs`。
3. **4 可随时开工**：文档、安装、tap、社区不堵 2/3；只在命令面或已确认发版的版本号变更时跟一次。
4. **发版按 MINOR 切片**：一轨合入一版，不把 2+3+4 捆成一次大发版。是否发版另问。
5. **冲突文件串行**：下表同一时刻只允许一轨改该文件。

### 5.2 文件锁

| 文件 / 面 | 谁改 | 规则 |
|---|---|---|
| `home_menu_state.rs` | 轨 3 主改（第 7 项） | 轨 2/4 不抢；文案由轨 3 落地后轨 4 跟截图 |
| `check-command-surface.sh` | 轨 3 | `agent` 正向探测，不进 Mole required |
| `coverage.rs` | 轨 2（hints）与轨 3（agent） | 分段追加字符串，禁止互删 |
| README / 五语 | 轨 4 主改 | 2/3 合入后轨 4 跟一版，不预写未落地命令为「已发布」 |

`coverage.rs` 允许两轨先后改，但不得在同一 PR 互相覆盖对方新加的 coverage 句；后合并的一方 rebase 后只追加、不删对方段落。

## 6. 风险

1. **`agent` 与 `worktree` 重叠**：同一路径只许一个命令认领；checkout 归 worktree，其余归 agent。
2. **误删会话数据**：默认全不选 + 不宣称可安全删 + 硬排除 cwd。
3. **hints 超时拖慢 clean**：沿用 M6 墙钟预算，超则 skip。
4. **Home 偏离 Mole 五项**：已有第 6 项先例；第 7 项是有意增值，README 写明。

## 7. 验收

文档级验收（实施以各轨 plan 为准）：

- 三轨可独立开 PR、独立 MINOR，互不阻塞。
- `vole agent --plan` 在无 TTY 下只出候选、不删；TTY 默认全不选。
- `worktree` 行为与列表契约不变。
- clean hints 长尾不出现 `Command::Hints`。
- status 无虚标未接线键。
- README / Helper 不把未合入的 `agent` 写成已发布（轨 3 合入前不列；合入后再改）。
- 不 bump `schema_version`；不冲 Homebrew core；不开桌面 PR。

本文件（规格自身）验收：

- 无 TBD / TODO / 占位符
- 产品「v3」与包 `2.x` MINOR 关系写死
- 桌面不在本代际主路径写死
- 结构 A（一份规格 + 三份日后 plan）写死

## 8. 与既有文档关系

| 文档 | 关系 |
|---|---|
| [`2026-07-30-1900-v2-product-goals-design.md`](2026-07-30-1900-v2-product-goals-design.md) | v2 北极星是 CLI 补齐 Mole。该代际已完成。本文件开启 **v3**，不再以「追上 Mole 命令面」为成功标准。v2 写的「产品话术 ≠ 包 MAJOR」继续有效：v3 也不发 `3.0.0`。 |
| [`2026-08-08-2030-v2-cli-complete-design.md`](2026-08-08-2030-v2-cli-complete-design.md) | v2 续篇曾写「不另起 v3」。该句约束的是 **v2 做全当时** 的命名，不阻止 Mole 对齐完成后再开新代际。本文件显式开启产品 v3。`hints` 仍禁止顶层命令，本文件继承。 |
| [`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) | 权威盘点当时写「默认下一项实现：无」。对 **Mole 对齐缺口** 仍然成立：v3 不再从 Mole 路由表找下一项。v3 做的是体验长尾、自有增值与分发，不是再扩 Mole required 集合。 |
| [`2026-08-14-0228-worktree-design.md`](2026-08-14-0228-worktree-design.md) | `worktree` 契约全部保留：整棵 checkout、默认全不选、不宣称可安全删、Home 第 6 项。v3 **不**扩 worktree 管非 checkout；`agent` 认领其余 Agent 残留，路径去重见 §4.2.3。 |
| [`2026-08-08-2143-v2-m6-clean-hints-design.md`](2026-08-08-2143-v2-m6-clean-hints-design.md) | M6 已交付主路径（project artifacts + system data）。本代际轨 2 只补 §2.2 长尾两条；挂载点、只读、超时跳过、禁止 `Command::Hints` 全部继承。 |
| [`2026-08-09-2303-tui-t4-interactive-closeout-design.md`](2026-08-09-2303-tui-t4-interactive-closeout-design.md) | T4 已把 analyze 删除键、status 动画 cat / `k` / `c`、`optimize --whitelist` 标为有意不做。v3 轨 2 **继续不做** 这些项；只抛光已接线 footer。 |
| [`2026-08-10-1514-tui-t5-home-menu-roadmap-design.md`](2026-08-10-1514-tui-t5-home-menu-roadmap-design.md) | T5 钉过 Mole 五项首页。Worktree 已把 Home 扩到 6 项。v3 再扩到 7 项 Agent；1–6 含义与文案不变。这是有意偏离 Mole 五项，不是 Mole 缺口。 |
| [`2026-08-10-2012-tui-t8-status-polish-optimize-whitelist-design.md`](2026-08-10-2012-tui-t8-status-polish-optimize-whitelist-design.md) | T8 曾计划动画 cat + `k` / `c` + `optimize --whitelist`，当时未作为本代际必做。v3 **明确不做** 这些项；「status 抛光」仅限已接线键文案。 |
| [`2026-07-30-semver-policy-design.md`](2026-07-30-semver-policy-design.md) | 兼容新能力走 MINOR。本代际各轨切片按 MINOR 发；不因话术「v3」升 MAJOR。 |

## 9. 下一步

本文件已写入并提交，**待用户 review 本规格文件**。

用户确认规格无需再改之后，再按 writing-plans 拆 **三份** 独立实施计划（轨 2 / 轨 3 / 轨 4 各一份）。此刻：

- **不写** plan 正文
- **不开** 实现
- **不 bump** 包版本
- **不发版**
- **不开** PR
- **不碰** vole-macos
