# Vole：`worktree` 子命令设计

- 日期：2026-08-14 02:28
- 状态：已批准（会话逐节确认；Home 第 6 项为后续修订）
- 性质：vole 自有增值命令（Mole 钉版无此子命令）
- Mole 钉版：`third_party/mole-1.48.1`
- 对照：Mole AGENTS「Git worktree staleness is not decidable」；`vole purge` 只清产物、不删 checkout

## 1. 结论

交付顶层命令 **`vole worktree`**：盘点本机被遗忘的 Git worktree（官方 linked worktree + Agent 容器），启发式排序后由用户确认，将 **整棵 checkout 目录** 送入废纸篓，再注销 Git 登记。

编排在 `vole-core::ops`；`vole-cli` 薄前端。走 **purge / uninstall** 同一套 ProtoPlan 漏斗（`--plan` / `--apply` / TTY 多选），**不**走 clean 规则 TOML，**不**并入 `purge`。

硬约束：**不宣称 worktree 可安全删除。** 负向阻塞可证，正向「过期 / 可删」不可证。命令只排序、展示阻塞项、执行用户勾选。

本里程碑不改正式版本号、不发版。Home 菜单在 Mole 五项之后增加第 6 项 Worktree（数字键 6；1–5 含义不变）。

## 2. 产品决策（已钉）

| 项 | 选择 |
|---|---|
| 删除对象 | 整棵 checkout（目录 + 未提交改动 + 工作区文件），不是只清产物 |
| 发现范围 | 官方 Git worktree **和** Agent 容器 |
| 列表策略 | 全部列出，启发式把更像历史堆积的排前面；不隐藏 |
| 文件去向 | 进废纸篓，再 `git worktree prune` |
| 有阻塞项 | 仍可勾选；列表标红 |
| 仓库发现 | 复用 purge 搜索根 **+** 当前 cwd 所属仓库 |
| 命令形态 | 新顶层 `vole worktree`，plan/apply 漏斗 |
| 交互默认勾选 | **全不选**（与 purge 默认全选相反） |
| Home 菜单 | 第 6 项 Worktree（数字键 6）；前五项仍对齐 Mole |

## 3. CLI 面

| 调用 | 行为 |
|---|---|
| TTY 裸 `vole worktree` | 扫描 → 分页多选（默认全不勾选）→ `Proceed? [y/N]` → 废纸篓 → prune |
| 非 TTY，或 `--plan` / `--dry-run` / `-n` / `--json` / `--json-stream` / `--plan-out` | 只出候选，不删 |
| `--apply PLAN` | TTL + TOCTOU 后再删 |
| `--permanent` | 仅与 `--apply` 或交互确认一起：永久删，不进废纸篓 |

Shell 补全随 clap `Command::Worktree` 自动生成。

`scripts/check-command-surface.sh` 的 Mole required 集合 **不**加入 `worktree`。脚本增加正向探测：vole 源码枚举含 `Worktree`，且不把该命令当 Mole 缺口。`--enforce` 仍只强制 Mole required。

### 3.1 硬排除（plan 与 apply 都必须执行）

下列路径 **不进列表**；若被塞进 plan，apply **拒绝删除**（skip，不当成功）：

- 每个仓库的主工作区（该仓库 `.git` 为目录的那份 checkout；`git worktree list` 的 primary）
- 当前进程 cwd 所在的那棵 worktree（避免拆掉自己脚下）

### 3.2 Home 菜单

故意偏离 Mole 首页五项：这是 vole 增值入口，不是 Mole 缺口。

| 键 | 项 | 描述 |
|---|---|---|
| 1 | Clean | 不变 |
| 2 | Uninstall | 不变 |
| 3 | Optimize | 不变 |
| 4 | Analyze | 不变 |
| 5 | Status | 不变 |
| 6 | Worktree | `Remove leftover git worktrees` |

- `HOME_ITEMS` 从 5 扩到 6；`HomeCommand::Worktree`；`argv()` → `["worktree"]`
- 数字键 `1..=6`；光标下标 `0..=5`（六项）；`map_key` 接受 `'1'..='6'`
- Enter / `6` 与 CLI 裸 `vole worktree` 同一条交互（扫描 → 默认全不勾选 → 确认）
- **不**把 `purge` / `installer` 一并塞进 Home
- 测试 `home_items_match_mole_copy` 改为：前五项文案仍对齐 Mole，第六项为 Worktree
- `images/tui/home.png` 在实现时更新（不阻塞 spec）

## 4. 发现与合并

两条来源，按规范化绝对路径去重。只扫约定根，**禁止**把整个 `$HOME` 当无界根深扫。

### 4.1 仓库从哪来

1. 与 `purge` 同一套搜索根：
   - 若存在 `~/.config/vole/purge_paths`（或测试注入 `VOLE_PURGE_PATHS`）：使用所列路径
   - 否则默认 Mole `MOLE_PURGE_DEFAULT_SEARCH_PATHS` 中**存在**的目录（`www` / `dev` / `Projects` / `GitHub` / `Code` / `Workspace` / `Repos` / `Development` / `Library/CloudStorage` 等）
   - 另：`$HOME/*/` 一层，通过 purge 的「项目容器」启发式者纳入
2. 再加上当前 cwd 所属 Git 仓库（从 cwd 向上找 `.git`）
3. 在这些根下找 Git 仓库：目录含 `.git`（文件或目录）；跳过 `node_modules` 及 `purge` 产物 basename；深度有上限（与 purge 扫描深度同量级，默认 maxdepth 6）
4. 点目录容器仅显式列表，不把任意 `~/.*` 当根

### 4.2 来源 A：Git 登记

对每个已发现仓库执行 `git worktree list --porcelain`（有超时）。主工作区与 cwd worktree 丢掉。

登记了但目录已经不存在的，保留为形态 `stale-registration`：体积 0，选中后只 prune、不删文件。

这条能找到加在搜索根之外的 linked worktree（例如 `/tmp/...`）。

### 4.3 来源 B：Agent 容器文件系统

仅当目录存在才扫，子项须看起来像 checkout（含 `.git` 文件或目录）：

- `$HOME/.codex/worktrees/*`
- `$HOME/.claude/worktrees/*`
- 每个已发现仓库下的 `.worktrees/*`、`.claude/worktrees/*`

已在来源 A 出现的路径合并，不重复。未挂在任何已发现仓库 `worktree list` 上的，记为 `orphan-dir`。若其 `.git` 文件含 `gitdir:`，apply 时用该指针反查主仓并 prune。

本里程碑 **不**发现 Conductor 式更深容器，也 **不**通配任意 `~/foo/worktrees`。

### 4.4 合并后的条目形态

| 形态 | `rule_id` | 含义 | 选中后 |
|---|---|---|---|
| linked | `worktree:linked` | 已登记且目录还在 | 目录进废纸篓，再 `git worktree prune` |
| stale-registration | `worktree:stale` | 已登记、目录没了 | 只 prune |
| orphan-dir | `worktree:orphan-dir` | 容器里有目录，未挂在已发现仓库的 list 上 | 目录进废纸篓；能反查主仓则 prune |

扫描和 `git` 调用都有超时；超时则跳过该仓库或容器（fail-closed），不当成可删。

## 5. 启发式排序和阻塞项

列表包含所有非硬排除条目：**只排序、不隐藏。** UI 与 JSON **不得**出现 `safe`、`deletable`、或「可安全删除」文案。

### 5.1 阻塞项（只展示，不阻止勾选）

在该 worktree 目录内探测；超时或失败记为 `status-unknown`（fail-closed，不当成干净）：

| 标记 | 证据 |
|---|---|
| `dirty` | `git status --porcelain` 非空 |
| `ignored-keep` | `--ignored` 中出现、且 basename **不在** `PURGE_TARGETS` 里的条目（例如 `.env`） |
| `unpushed` | 该 worktree 上 `git log HEAD --not --remotes` 非空（detached 也用这条，不用 `--branches`） |
| `locked` | `git worktree list` 标明 locked |
| `status-unknown` | git 超时或失败 |

`stale-registration` 不是阻塞项，是形态；没有 checkout 可丢。

禁止把下列信号当作「过期 / 可删」证据：`git status` 干净、分支已合并、远端分支已删、目录 mtime 超过 N 天。

### 5.2 排序（越像遗忘越靠前）

1. 形态：`stale-registration`、`orphan-dir` 先于普通 `linked`
2. 年龄：该 worktree 最后一次 commit 时间；没有则退回 checkout **根目录** mtime。越旧越前。不用子树 mtime（APFS 不向上冒泡）
3. detached HEAD 先于有名字的分支
4. 同一档里体积大的优先
5. 阻塞项 **不**参与排序，只染色

交互列表每行：路径、体积、年龄、HEAD（detached / 分支名）、来源（`git` / `cursor` / `codex` / `claude`）、红色阻塞标记。没有绿色「安全」标记。

来源判定：路径落在 `~/.codex/worktrees` → `codex`；`~/.claude/worktrees` 或 `<repo>/.claude/worktrees` → `claude`；`<repo>/.worktrees` → `cursor`；其余来自 `git worktree list` → `git`。

## 6. Plan 契约

- `schema_version`：**不 bump**
- `ttl_secs`：`900`
- `rule_id`：仅 `worktree:linked` / `worktree:orphan-dir` / `worktree:stale`
- `id`：`worktree:{kind}:{canonical-path}`（稳定；apply 用 path 定位目录）
- `label`：`repo:<absolute-main-repo-path>` + 空格 + 展示文案（形态、HEAD、来源、阻塞项）。apply 解析此前缀；失败则 skip。不新增必填 PlanEntry 字段、不 bump schema。
- 可选增加 `PlanEntry.blockers: Vec<String>`：`#[serde(default, skip_serializing_if = "Vec::is_empty")]`；没有该字段的旧 JSON 仍可反序列化。`stale` 条目 blockers 为空。
- **禁止**增加 `safe` / `deletable` 字段或同义字段
- 有目录的条目：plan 阶段 `capture_plan_entry_identity`（`dev` / `ino` / `mtime`）
- `stale` 条目：`dev` / `ino` / `mtime` 为 0；apply 只认「路径仍缺失」

`coverage_note`（plan 级，可选）诚实记录：未扫 Conductor 容器、超时跳过的仓库数、不宣称可安全删除。

## 7. Apply 漏斗

独立 `apply_worktree_plan` / `apply_worktree_proto_plan`。只接受 `rule_id` 前缀 `worktree:`；其它前缀整份 plan 拒绝或逐条 skip（钉死：**逐条 skip 非 `worktree:` 条目，并计入 skipped**；空前缀或未知 kind 同样 skip）。

- 互斥：`try_lock_worktree()` → `try_lock_config("worktree")`
- oplog：`command = "worktree"`
- 删除：**只**走 `mole_delete_verified`。禁止平行 `rm -rf`、禁止 `git worktree remove`、禁止 `git branch -d`
- 默认 `DeleteMode::Trash`；`--permanent` → Permanent
- schema / TTL 校验同 purge：过期返回明确错误，提示 `vole worktree --plan` 重扫

### 7.1 linked / orphan-dir

1. 再次硬排除：主工作区、cwd 所在 worktree → skip
2. 从 `label` 解析 `repo:<abs>`；失败 → skip
3. `validate_path_for_deletion` + Cleanup `AppProtection` + `(dev, ino, mtime)` TOCTOU
4. 身份变化 → skip（目录被替换或修改）
5. `mole_delete_verified`
6. 在主仓执行 `git worktree prune`（有超时）
7. 若 prune 后仍登记且 plan 时为 `locked`：用户已确认，执行 `git worktree unlock <path>` 再 prune 一次
8. prune 仍失败：文件已进废纸篓、登记还在 → 该条记 **partial / failed**（登记失败不得标 succeeded），**不**从废纸篓自动搬回

### 7.2 stale-registration

无目录：不做文件 TOCTOU、不调用 `mole_delete`。确认路径仍不存在后，在主仓 `git worktree prune`。若路径在 apply 时又出现了 → skip（不当 stale）。

### 7.3 废纸篓捞回

从废纸篓捞回目录后 **不再是** linked worktree。这是既定限制，`--help` 用一句说明即可。不宣称能恢复登记。

## 8. 与 `purge` 的分工

| | `purge` | `worktree` |
|---|---|---|
| 删什么 | 可重建产物（`node_modules` / `target` 等） | checkout 目录 + 注销登记 |
| 默认勾选 | 全选 | 全不选 |
| 判决 | 年龄 + basename 白名单 | 无正向判决，只排序 + 阻塞项 |
| 搜索 | 已含部分 agent 容器，只匹配产物 basename | 匹配 worktree checkout 本身 |

本命令 **不清** worktree 内部产物。用户要收产物空间继续用 `vole purge`。

## 9. 安全边界和明确不做

### 9.1 硬约束

- 不输出「可安全删除 / safe / deletable」
- 不删主工作区，不删 cwd 所在 worktree；apply 再查一次
- 不把整棵 `$HOME` 当根深扫；点目录容器只扫已点名的那些
- 不把「status 干净」或「分支已合并」当过期证据
- 删除只走 `mole_delete_verified`；登记只走 `git worktree prune`（locked 且用户已确认时才 `unlock`）
- 不把 worktree 本体塞进 `purge` / `clean` 规则

### 9.2 本里程碑明确不做

- 自动删、定时删、无确认的 `--yes` 清全部
- `git worktree remove` / `git branch -d`（不删分支）
- 清 worktree 内 `node_modules` 等产物（那是 `vole purge`）
- Conductor 式更深容器、任意 `~/foo/worktrees` 通配发现
- 宣称废纸篓捞回后能恢复成 linked worktree
- 改 Mole 钉版行为或要求 Mole 命令面对齐 `worktree`
- 把 `purge` / `installer` 塞进 Home（只加 Worktree 第 6 项）
- 修改正式版本号或发版

## 10. 文件落点

```
crates/vole-core/src/ops/worktree_plan.rs
crates/vole-core/src/ops/worktree_apply.rs
crates/vole-core/src/ops/mod.rs
crates/vole-core/src/mutex.rs                 # try_lock_worktree
crates/vole-proto/src/plan.rs                 # 可选 blockers 字段
crates/vole-cli/src/worktree.rs
crates/vole-cli/src/main.rs                   # Command::Worktree
crates/vole-cli/src/tui/home_menu_state.rs    # 第 6 项
crates/vole-cli/src/tui/home_menu.rs          # 数字键 6
crates/vole-cli/tests/worktree_cli.rs
scripts/check-command-surface.sh              # 正向探测；不加入 Mole required
README.md / README.zh-CN.md 等                # Features 表加 Worktree
```

Git 调用通过 `std::process::Command` 跑本机 `git`（macOS CI 有 git）。测试注入：`GIT_EXECUTABLE` 或 PATH stub 可在单测里替换；生产默认 `git`。

## 11. 测试与验收

### 11.1 单元（`vole-core`）

- 发现合并：同一路径 linked 不与 orphan 重复
- 主工作区与 cwd worktree 硬排除
- 排序稳定：stale/orphan 先于 linked；同形态更旧更前；detached 先于 named branch
- `apply_worktree_plan` 对非 `worktree:` 条目 skip
- 身份变化 skip
- 主仓 path 在 apply 时若已是 primary / cwd → skip
- `stale` 只 prune、不删文件；路径又出现则 skip
- 序列化：plan JSON 不含 `safe` / `deletable`；`blockers` 为空时省略

### 11.2 CLI

- temp HOME + 临时 git 仓库 `git worktree add` 出额外 checkout
- `vole worktree --plan --json` 含该条目，JSON 无 `safe`
- 交互门控单测：TTY 裸调用进多选；`--plan` / `--json` 不进多选；默认预选为空（与 purge 相反）
- `--apply` + `MOLE_TEST_TRASH_DIR`：目录进入测试废纸篓，之后 `git worktree list` 不再包含它
- 负向：主工作区不出现在 plan；把主仓 path 写入 plan 后 apply 拒绝
- `vole worktree --help` 退出 0，含废纸篓 / 不宣称可安全删除的说明
- `./scripts/check-command-surface.sh --enforce` 仍通过；help 含 `worktree`
- Home：`HOME_ITEMS.len() == 6`；数字键 6 启动 Worktree；1–5 仍启动原 Mole 五项；前五项 title/description 不变

### 11.3 文档阶段验收

- [x] 命令面 / 发现合并 / 排序阻塞 / 删除漏斗 / 安全边界已钉死
- [x] 与 `purge` 分工明确
- [x] 不宣称可安全删除
- [x] Home 第 6 项 Worktree；不改 Mole required 集合；不绑发版号

## 12. 实现备注

- 超时常量：复用 `vole-sys` / 现有 git-less 扫描超时量级；每个 `git` 子进程必须有 wall-clock 上限，禁止无界 `du` / 无界 `git status`
- 体积：对 checkout 根做 timeout-bounded 计量；失败则 size=0 仍可列出
- `orphan-dir` 的 `.git` 为目录（独立克隆而非 worktree）时：仍可当目录删除（用户已确认），但 **不要** 对无关仓库执行 prune；仅当 `gitdir:` 指向已发现主仓的 `worktrees/` 下才 prune
- 主工作区判定：`git rev-parse --git-common-dir` 与 `--git-dir` 相同且 `.git` 为目录；或 porcelain 的 `bare`/`worktree` 字段按 Git 文档解析。钉死用 `git worktree list --porcelain` 的第一份（list 中主 worktree 在最前）+ 路径与 `git rev-parse --show-toplevel` 相同者为主工作区
