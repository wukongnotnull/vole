# Vole M4：CLI 做全 · Mole 库存与安全面 Spike Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use wukong-code:executing-plans（本仓默认 inline）或 wukong-code:subagent-driven-development。Steps 用 checkbox（`- [ ]`）跟踪。
>
> **本里程碑纯 docs-only。** 禁止实现 `purge` / `installer` / `touchid` / `update` / `remove` / `hints` 产品行为；禁止 bump 到 `2.0.0`；禁止改 CLI/core 行为代码。

**Goal:** 完成产品 v2 续篇里程碑 **M4**：对 Mole 1.48.1 六命令（`purge` / `installer` / `touchid` / `update` / `remove`）+ `hints` 模块 + 别名/裸调用做库存与安全面 spike，划主路径 vs 长尾，核定规格 §3.1 命令面对照，并留下 §3.2 命令面核对闸门的可执行草案（stub/清单）；为 M5–M10 各命令专用 design 提供输入清单。

**Architecture:** 只读对照钉版 Mole 路由与脚本 → 写入 findings + 对照表 + 闸门 stub。不改产品代码、不升 MAJOR。闸门完整落地留给收口里程碑；M4 产出可跑的核对清单与 stub 脚本骨架，形态对齐 `scripts/inventory-mole-rules.py` 的「机械核对」精神。

**Tech Stack:** Markdown findings、shell/python stub（docs-only）、Mole 钉版 `third_party/mole-1.48.1`、既有 `crates/vole-cli` 只读对照（`clap` 命令枚举 / `--help`）。

**Design（权威规格）:** [`docs/wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md`](../specs/2026-08-08-2030-v2-cli-complete-design.md)

## Global Constraints

- 规格权威：[`docs/wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md`](../specs/2026-08-08-2030-v2-cli-complete-design.md)（产品 v2 CLI 做全续篇）
- Mole 钉版：`third_party/mole-1.48.1`；升级钉版另议，不盲跟上游
- 当前包版本可仍停在 **1.46.x**；**禁止**本里程碑 bump 到 `2.0.0`（`2.0.0` 锁定在 M5 `purge` 首发）
- **禁止实现**产品行为：`purge` / `installer` / `touchid` / `update` / `remove` / `hints` 模块；禁止新增顶层 `vole hints`
- `hints` 不是子命令（Mole 亦无 `mo hints`）；按 M6 交付为 `clean` 内只读提示
- 裸调用默认不联网：Vole 不跟进 Mole `check_for_updates`（规格 §6.5）
- 禁区保留：不删本地快照（apply）；不删 `/Library/Updates`、`/macOS Install Data`
- 合入 PR 用 **merge commit**（非 squash）
- 每个 Task 至少一次 commit（仅 docs / stub）；无源码行为变更则勿 `git add` 产品代码
- 任务用语：实现项 / 下一项（不用隐喻缩写）

## File map

| 文件 | 职责 |
|---|---|
| `docs/findings/2026-08-v2-m4-cli-complete-spike.md` | M4 总 findings：库存、安全面、主路径/长尾、§3.1 核定、后续 design 输入 |
| `docs/findings/2026-08-v2-m4-command-surface.md` | §3.1 命令面对照核定表（Mole 路由 → Vole 现状 → 处置里程碑） |
| `docs/findings/2026-08-v2-m4-gate-checklist.md` | §3.2 闸门核对清单（可执行步骤） |
| `scripts/check-command-surface.sh` | §3.2 闸门 stub：解析 Mole `mole` case 与 Vole `--help`/源码枚举，打印缺口（M4 可先 fail-open / dry 报告） |
| `docs/wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md` | 权威规格；Task 收口可回链本 plan（仅文档链接，不改决策） |
| `third_party/mole-1.48.1/mole` | CLI dispatch / 裸调用 `check_for_updates` |
| `third_party/mole-1.48.1/bin/purge.sh` | purge 入口与 flag |
| `third_party/mole-1.48.1/lib/clean/project.sh` | purge 发现/删除主逻辑 |
| `third_party/mole-1.48.1/lib/clean/hints.sh` | clean 内只读 hints |
| `third_party/mole-1.48.1/bin/installer.sh` | installer 扫描与 delete-plan |
| `third_party/mole-1.48.1/bin/touchid.sh` | PAM Touch ID |
| `third_party/mole-1.48.1/lib/manage/update.sh` | 自更新 + `check_for_updates` |
| `third_party/mole-1.48.1/lib/manage/remove.sh` | 自卸载 |
| `crates/vole-cli/src/main.rs` | 只读：当前 `Command` 枚举 / 别名缺口 |
| `crates/vole-cli/src/interactive.rs` | 只读：裸调用菜单（确认无联网更新） |

---

### Task 1: Findings 骨架 + §3.1 对照 stub

**Files:**
- Create: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`
- Create: `docs/findings/2026-08-v2-m4-command-surface.md`

**Interfaces:**
- Consumes: 规格 §3.1 / §5 M4；Mole `mole` case 路由；`crates/vole-cli/src/main.rs` `enum Command`
- Produces: findings 骨架章节标题；命令面 stub 表（后续 Task 填核定列）

- [ ] **Step 1: 从 Mole dispatch 抽出顶层路由清单**

```bash
cd /Users/wukong/Documents/vole
rg -n '^\s+"(optimize|clean|uninstall|analyze|status|purge|installer|touchid|completion|update|remove|help|version)' \
  third_party/mole-1.48.1/mole
# history 为 early dispatch：
rg -n 'history' third_party/mole-1.48.1/mole | head -20
```

Expected：命中 `optimize|optimise`、`clean`、`uninstall`、`analyze|analyse`、`status`、`purge`、`installer`、`touchid`、`completion`、`update`、`remove`、`help`/`--help`/`-h`、`version`/`--version`/`-V`；空分支调用 `check_for_updates`；`history` 在 `mole_dispatch_history_early`。

- [ ] **Step 2: 写命令面 stub**

创建 `docs/findings/2026-08-v2-m4-command-surface.md`，含表格列：`Mole 命令` | `Mole 实现` | `Vole 1.46.0` | `规格处置` | `M4 核定（待填）`。先填规格 §3.1 全部行（含豁免 `hints` / `whitelist`）。

- [ ] **Step 3: 写 findings 骨架**

创建 `docs/findings/2026-08-v2-m4-cli-complete-spike.md`，固定章节：

1. 结论  
2. §3.1 核定摘要（链到 command-surface）  
3. 分命令库存（purge / hints / installer / touchid / update / remove）  
4. 别名与裸调用  
5. 主路径 vs 长尾总表  
6. 安全面与禁区  
7. §3.2 闸门草案（链到 checklist + stub）  
8. 后续 design 输入清单（M5–M10）  
9. 明确未做（本 spike 无产品代码）

- [ ] **Step 4: Commit**

```bash
git add docs/findings/2026-08-v2-m4-cli-complete-spike.md \
        docs/findings/2026-08-v2-m4-command-surface.md
git commit -m "$(cat <<'EOF'
docs: scaffold M4 CLI-complete spike findings

EOF
)"
```

---

### Task 2: `purge` 库存与安全面（→ M5 输入）

**Files:**
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`（§3 purge 节）
- Read: `third_party/mole-1.48.1/bin/purge.sh`
- Read: `third_party/mole-1.48.1/lib/clean/project.sh`
- Read: `third_party/mole-1.48.1/lib/clean/purge_shared.sh`（若存在）
- Read: `third_party/mole-1.48.1/tests/purge.bats`、`purge_config_paths.bats`

**Interfaces:**
- Consumes: Mole purge flag / 发现 / 删除漏斗
- Produces: M5 主路径 vs 长尾草案 + design 必答问题清单

- [ ] **Step 1: 摘录入口与配置面**

阅读并在 findings 记录：

- Flag：`--dry-run`/`-n`、`--paths`、`--include-empty`、`--debug`、`--help`（无独立 `--apply`；交互确认后删除）
- 配置：`~/.config/mole/purge_paths`；`manage_purge_paths`（`lib/manage/purge_paths.sh`）
- 默认搜索根：`MOLE_PURGE_DEFAULT_SEARCH_PATHS` / `DEFAULT_PURGE_SEARCH_PATHS`
- 目标表：`MOLE_PURGE_TARGETS` / `PURGE_TARGETS`；`MIN_AGE_DAYS=7`；depth 1–6

验证命令：

```bash
rg -n 'DEFAULT_PURGE|MOLE_PURGE|MIN_AGE|--dry-run|--paths|mole_delete|perform_purge|discover_project' \
  third_party/mole-1.48.1/bin/purge.sh \
  third_party/mole-1.48.1/lib/clean/project.sh \
  third_party/mole-1.48.1/lib/clean/purge_shared.sh 2>/dev/null | head -80
```

- [ ] **Step 2: 划主路径 vs 长尾（建议默认，供 M5 design 收紧）**

在 findings 写入建议划界（可微调但不得在本 spike 实现）：

**主路径（建议进 M5 / 2.0.0）**

1. 发现项目根（默认搜索路径 + `$HOME/*/` 容器探测；点目录容器仅显式列表）
2. 按 `PURGE_TARGETS` 匹配重建型产物（如 `node_modules`、`target`、`dist` 等钉版表）
3. 年龄门槛（默认 7 天）与超时/浅扫预算
4. Vole 两阶段：`--plan` / `--apply`（Mole 的 dry-run ≈ plan；交互删除 ≈ apply）
5. JSON / `plan_out`；默认废纸篓；可 `--permanent`
6. 删除走既有 `mole_delete_verified` + 保护层；`purge_paths`（或等价）配置
7. 菜单项 + shell 补全；建议同 PR 交付 §3.3 别名（`completion`/`optimise`/`analyse`）

**长尾（coverage / 继续用 Mole）**

- TTY 分页多选 UI（用 plan JSON 代替）
- 广域 custom / 与 AGENTS「worktree 本体删除」禁令冲突的任何「整树判定可删」
- 未在钉版 `PURGE_TARGETS` 证明的扩张目标
- sudo / 系统域产物

- [ ] **Step 3: 安全面要点**

记录：删除必须经 `mole_delete`；禁止平行 `rm -rf`；与规格 §6.1–6.2 对齐；工作区/gitignore 秘密不可当「可删」证据（对照 Mole AGENTS worktree 条款）。

- [ ] **Step 4: Commit**

```bash
git add docs/findings/2026-08-v2-m4-cli-complete-spike.md
git commit -m "$(cat <<'EOF'
docs: M4 purge inventory and main-path boundary

EOF
)"
```

---

### Task 3: `hints` 模块库存（→ M6 输入；非顶层命令）

**Files:**
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`（hints 节）
- Read: `third_party/mole-1.48.1/lib/clean/hints.sh`
- Read: `third_party/mole-1.48.1/bin/clean.sh`（source 与调用点）
- Read: `third_party/mole-1.48.1/tests/clean_hints.bats`

**Interfaces:**
- Consumes: hints 探针函数与 clean 挂载点
- Produces: M6 主路径子集 +「禁止 `vole hints`」核定

- [ ] **Step 1: 确认非顶层命令**

```bash
rg -n 'hints' third_party/mole-1.48.1/mole || true
rg -n 'source .*hints|show_.*hint|probe_project' third_party/mole-1.48.1/bin/clean.sh | head -40
```

Expected：`mole` 路由表无 `hints`；`clean.sh` source `lib/clean/hints.sh` 并调用 `show_*_hint_notice` / `probe_*`。

- [ ] **Step 2: 列出主路径提示族**

在 findings 记录至少这些家族（对照 bats）：

- `probe_project_artifact_hints` / `show_project_artifact_hint_notice`（含 wall-clock 预算、慢扫跳过）
- `show_system_data_hint_notice`
- `show_user_launch_agent_hint_notice`
- `show_orphan_dotdir_hint_notice`
- quick purge 探针：`load_quick_purge_hint_paths` / `is_quick_purge_project_root`

- [ ] **Step 3: 划界**

**主路径：** 只读；超时与浅扫预算；慢路径降级跳过，不阻塞 clean；可含 purge 快捷探针子集。  
**长尾：** 全量 Mole 提示文案/交互；第二套删除路径（禁止）。  
**硬约束：** 禁止顶层 `vole hints`。

- [ ] **Step 4: Commit**

```bash
git add docs/findings/2026-08-v2-m4-cli-complete-spike.md
git commit -m "$(cat <<'EOF'
docs: M4 hints module inventory for clean-only delivery

EOF
)"
```

---

### Task 4: `installer` 库存与安全面（→ M7 输入）

**Files:**
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`
- Read: `third_party/mole-1.48.1/bin/installer.sh`
- Read: `third_party/mole-1.48.1/tests/installer.bats`、`installer_fd.bats`、`installer_zip.bats`

**Interfaces:**
- Produces: 扫描根、immutable delete-plan、主路径 vs 长尾

- [ ] **Step 1: 摘录扫描根与 plan/execute**

```bash
rg -n 'Downloads|Desktop|build_installer_delete_plan|execute_installer_delete_plan|immutable|--dry-run|mole_delete' \
  third_party/mole-1.48.1/bin/installer.sh | head -60
```

记录：扫描根列表（Downloads/Desktop/iCloud/Telegram 等）；`build_installer_delete_plan` → 校验 → `execute_installer_delete_plan`；dry-run；incomplete-cleanup 退出语义。

- [ ] **Step 2: 划界**

**主路径：** 扫描安装包 → plan → apply；immutable delete-plan 校验精神；删除走安全漏斗；JSON。  
**长尾：** 全量冷门扫描根、TTY 分页选择器、fd/zip 边缘（可第二批）。

- [ ] **Step 3: Commit**

```bash
git add docs/findings/2026-08-v2-m4-cli-complete-spike.md
git commit -m "$(cat <<'EOF'
docs: M4 installer inventory and delete-plan safety notes

EOF
)"
```

---

### Task 5: `touchid` 库存与安全面（→ M8 输入）

**Files:**
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`
- Read: `third_party/mole-1.48.1/bin/touchid.sh`

**Interfaces:**
- Produces: PAM 路径、`sudo_local` 优先、测试闸门要求

- [ ] **Step 1: 摘录 PAM 与 dry-run / 测试护栏**

```bash
rg -n 'pam_tid|sudo_local|MOLE_TEST_NO_AUTH|touchid_dry_run|enable|disable' \
  third_party/mole-1.48.1/bin/touchid.sh | head -50
```

记录：优先 `sudo_local` + `pam_tid.so`；legacy `/etc/pam.d/sudo` 迁移；dry-run；回滚路径需在 M8 design 写死。

- [ ] **Step 2: 划界与硬约束**

**主路径：** 状态查询 + 启用/禁用引导（高对齐 Mole）；`VOLE_TEST_NO_AUTH` / mock 下可测。  
**长尾：** 真机授权交互演示、非 sudo PAM 扩展。  
**硬约束：** 验证路径禁止触发真 Touch ID / 交互 sudo 挂起。

- [ ] **Step 3: Commit**

```bash
git add docs/findings/2026-08-v2-m4-cli-complete-spike.md
git commit -m "$(cat <<'EOF'
docs: M4 touchid PAM inventory and test-no-auth constraints

EOF
)"
```

---

### Task 6: `update` 自更新库存（→ M9 输入）

**Files:**
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`
- Read: `third_party/mole-1.48.1/lib/manage/update.sh`
- Read: `third_party/mole-1.48.1/mole`（`update` 分支与裸调用）
- Read: `third_party/mole-1.48.1/tests/update.bats`（若存在）

**Interfaces:**
- Produces：检测→下载→校验→安装；brew 共存；fail-closed；与裸调用分流

- [ ] **Step 1: 摘录公开 API 与关键安全不变量**

```bash
rg -n '^(check_for_updates|update_mole|_update_self_heal|is_homebrew_install|get_latest_version)' \
  third_party/mole-1.48.1/lib/manage/update.sh
rg -n 'checksum|attestation|fail|--force|--nightly|install\.sh|version' \
  third_party/mole-1.48.1/lib/manage/update.sh | head -40
```

记录：`mole update [--force|--nightly]`；`check_for_updates` 仅裸调用路径；Homebrew vs 手动前缀；校验失败 fail-closed（对照 Mole AGENTS / install.sh）；成功以安装后版本为准。

- [ ] **Step 2: 划界**

**主路径：** 显式 `vole update` 自更新通道（非仅 `brew upgrade` 包装）；`--force` / `--nightly`（名称以 M9 design 为准）；brew 管理时默认提示 `brew upgrade`。  
**长尾 / 差异：** **不**在裸调用联网检查（规格硬差异）。  
**硬约束：** 禁止静默降级到未校验源码安装；自愈路径不得引入「校验失败仍继续」。

- [ ] **Step 3: Commit**

```bash
git add docs/findings/2026-08-v2-m4-cli-complete-spike.md
git commit -m "$(cat <<'EOF'
docs: M4 self-update inventory and bare-call no-network split

EOF
)"
```

---

### Task 7: `remove` 自卸载库存（→ M10 输入）

**Files:**
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`
- Read: `third_party/mole-1.48.1/lib/manage/remove.sh`

**Interfaces:**
- Produces：三类安装形态、`--dry-run`、与 update 共享判定

- [ ] **Step 1: 摘录删除范围**

```bash
rg -n 'remove_mole|Homebrew|dry_run|alias|completion|config|cache|Cellar' \
  third_party/mole-1.48.1/lib/manage/remove.sh | head -60
```

记录三类：**Homebrew**、**手动前缀**、**shell alias / 补全残留**；`--dry-run`/`-n`；配置/缓存清理边界。

- [ ] **Step 2: 划界**

**主路径：** `vole remove --dry-run` 预览；仅删本工具安装产物与自身配置；brew 提示 `brew uninstall`；删除走路径校验漏斗。  
**长尾：** 用户数据、oplog/审计（除非显式要求）、其它 brew 包。  
**耦合：** 与 M9 共享安装来源判定；若 design 显示高耦合可合并里程碑（规格允许）。

- [ ] **Step 3: Commit**

```bash
git add docs/findings/2026-08-v2-m4-cli-complete-spike.md
git commit -m "$(cat <<'EOF'
docs: M4 self-remove inventory and install-shape shared notes

EOF
)"
```

---

### Task 8: 别名 + 裸调用不联网核定

**Files:**
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`
- Modify: `docs/findings/2026-08-v2-m4-command-surface.md`
- Read: `crates/vole-cli/src/main.rs`、`interactive.rs`
- Read: `third_party/mole-1.48.1/mole`

**Interfaces:**
- Produces：§3.3 别名缺口清单；裸调用差异写死为可接受/必做

- [ ] **Step 1: 核对 Vole 别名缺口**

```bash
rg -n 'visible_alias|optimise|analyse|completion|Completions|enum Command' \
  crates/vole-cli/src/main.rs
rg -n 'check_for_updates|http|github|update' crates/vole-cli/src/interactive.rs || true
```

Expected（1.46.0）：有 `Optimize`/`Analyze`/`Completions`；**无** `optimise`/`analyse`/`completion` 别名；交互菜单无联网更新。

- [ ] **Step 2: 写入核定**

- 别名：`completion` → `completions`；`optimise` → `optimize`；`analyse` → `analyze`；建议随 **M5** 一并交付使 `2.0.0` 即具备完整命令名兼容（规格 §3.3）
- 裸调用：Vole 直接进菜单，**不**跟进 `check_for_updates`（已达差异，收口闸门须断言「无子命令时不发起版本检查网络请求」——至少文档/静态核对）

- [ ] **Step 3: Commit**

```bash
git add docs/findings/2026-08-v2-m4-cli-complete-spike.md \
        docs/findings/2026-08-v2-m4-command-surface.md
git commit -m "$(cat <<'EOF'
docs: M4 alias gaps and bare-call no-network ratification

EOF
)"
```

---

### Task 9: §3.2 命令面核对闸门 — 形态草案 + stub

**Files:**
- Create: `docs/findings/2026-08-v2-m4-gate-checklist.md`
- Create: `scripts/check-command-surface.sh`
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`（§7 链到本 Task）

**Interfaces:**
- Consumes: Task 1–8 核定的 Mole 必覆盖命令集合（减豁免）
- Produces: 可执行清单 + stub 脚本；完整 CI 强制留给收口

**Mole 必覆盖集合（减豁免后，stub 内硬编码初版）：**

```text
clean uninstall optimize optimise analyze analyse status history
completion completions help version
purge installer touchid update remove
```

豁免不要求顶层：`hints`、`whitelist`（形态差异见规格 §3.1）。

- [ ] **Step 1: 写核对清单**

`docs/findings/2026-08-v2-m4-gate-checklist.md` 含步骤：

1. 从 `third_party/mole-1.48.1/mole` 解析路由（含 early `history`、别名）  
2. 从 `vole --help` 或 `crates/vole-cli/src/main.rs` 解析 Vole 顶层命令+别名  
3. 断言集合覆盖（差集为空）  
4. 断言无顶层 `hints`  
5. 断言裸调用路径无 `check_for_updates` 等价联网（静态：`interactive.rs` 无更新探测调用）  
6. 收口时改为 CI 失败即红；M4 stub 可 `--report-only`

- [ ] **Step 2: 写 stub 脚本**

创建可执行 `scripts/check-command-surface.sh`：

```bash
#!/usr/bin/env bash
# M4 stub for §3.2 command-surface gate.
# Full enforcement lands at closeout; this script reports gaps.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOLE="$ROOT/third_party/mole-1.48.1/mole"
VOLE_MAIN="$ROOT/crates/vole-cli/src/main.rs"
REPORT_ONLY=1
[[ "${1:-}" == "--enforce" ]] && REPORT_ONLY=0

required=(
  clean uninstall optimize optimise analyze analyse status history
  completion purge installer touchid update remove
)

# Mole routes from case arms + early history
mole_routes=$(
  {
    rg -o '"[a-z]+"( \| "[a-z]+")*' "$MOLE" | tr -d '"' | tr '|' '\n' | tr -d ' '
    echo history
  } | sort -u
)

# Vole: clap command idents + note missing aliases via source grep
vole_cmds=$(
  {
    rg -n '^\s+(Clean|Uninstall|Optimize|Status|Analyze|History|Completions)\b' "$VOLE_MAIN" \
      | sed -E 's/.*(Clean|Uninstall|Optimize|Status|Analyze|History|Completions).*/\1/' \
      | tr '[:upper:]' '[:lower:]'
    # completions ↔ completion
    rg -q 'Completions' "$VOLE_MAIN" && echo completions
    rg -qi 'visible_alias.*"completion"|alias.*"completion"' "$VOLE_MAIN" && echo completion
    rg -qi 'optimise' "$VOLE_MAIN" && echo optimise
    rg -qi 'analyse' "$VOLE_MAIN" && echo analyse
    # future commands
    rg -qi '\bPurge\b' "$VOLE_MAIN" && echo purge
    rg -qi '\bInstaller\b' "$VOLE_MAIN" && echo installer
    rg -qi '\bTouch[Ii]d\b' "$VOLE_MAIN" && echo touchid
    rg -qi '\bUpdate\b' "$VOLE_MAIN" && echo update
    rg -qi '\bRemove\b' "$VOLE_MAIN" && echo remove
  } | sort -u
)

echo "=== Mole routes (sample) ==="
echo "$mole_routes" | tr '\n' ' '; echo
echo "=== Vole cmds/aliases (detected) ==="
echo "$vole_cmds" | tr '\n' ' '; echo
echo "=== Required coverage gaps ==="
gaps=0
for c in "${required[@]}"; do
  if ! printf '%s\n' "$vole_cmds" | grep -qx "$c"; then
    echo "MISSING: $c"
    gaps=$((gaps + 1))
  fi
done

# Negative: top-level hints must not exist as Command
if rg -n '^\s+Hints\b' "$VOLE_MAIN" >/dev/null; then
  echo "UNEXPECTED: top-level Hints command"
  gaps=$((gaps + 1))
fi

if [[ "$gaps" -gt 0 ]]; then
  echo "gaps=$gaps (expected during M4–M9; closeout must be 0)"
  [[ "$REPORT_ONLY" -eq 1 ]] && exit 0
  exit 1
fi
echo "OK: command surface covers required set"
exit 0
```

- [ ] **Step 3: 跑 stub（report-only）**

```bash
chmod +x scripts/check-command-surface.sh
./scripts/check-command-surface.sh
```

Expected：打印若干 `MISSING: purge|installer|touchid|update|remove|optimise|analyse|completion` 等；exit 0（report-only）。  
`--enforce` 当前应非零（证明 stub 能失败）。

```bash
./scripts/check-command-surface.sh --enforce ; echo exit=$?
```

Expected：`exit=` 非 0。

- [ ] **Step 4: Commit**

```bash
git add docs/findings/2026-08-v2-m4-gate-checklist.md \
        scripts/check-command-surface.sh \
        docs/findings/2026-08-v2-m4-cli-complete-spike.md
git commit -m "$(cat <<'EOF'
docs: draft §3.2 command-surface gate stub for closeout

EOF
)"
```

---

### Task 10: 核定 §3.1 + 总表收口 + 规格回链

**Files:**
- Modify: `docs/findings/2026-08-v2-m4-command-surface.md`（填「M4 核定」列）
- Modify: `docs/findings/2026-08-v2-m4-cli-complete-spike.md`（结论、总表、design 输入）
- Modify: `docs/wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md`（§9 或 §5 增一行链到本 plan / findings；**不改**已锁定决策）

**Interfaces:**
- Consumes: Task 2–9
- Produces: 「§3.1 已核定」声明；M5–M10 design 输入清单完整

- [ ] **Step 1: 填完对照表核定列**

对 §3.1 每一行写：`已达` / `缺口→Mx` / `豁免（理由）` / `故意差异（裸调用不联网）`。须覆盖 `remove`、别名、裸调用。

- [ ] **Step 2: 写后续 design 输入清单（每命令 5–8 条必答）**

至少覆盖：

| 里程碑 | 必答输入（摘要） |
|---|---|
| M5 purge | plan/apply 映射；targets 表来源；年龄/深度；purge_paths 配置位置；JSON schema 复用；别名是否同 PR |
| M6 hints | 挂载点（clean 何阶段）；预算秒数；探针子集；禁止顶层命令的测试 |
| M7 installer | 扫描根首批；immutable plan 字段；zip/pkg 范围 |
| M8 touchid | PAM 文件路径注入；回滚；`VOLE_TEST_NO_AUTH` 契约 |
| M9 update | 发布资产/校验；brew 提示 vs `--force`；禁止裸调用联网的回归 |
| M10 remove | 删除白名单；与 update 共享 API；oplog 是否保留 |

- [ ] **Step 3: 规格回链**

在规格 §9「下一步」或 §5 表注增加指向：

- 本 plan：`docs/wukong-code/plans/2026-08-08-2051-v2-m4-cli-complete-spike.md`
- findings：`docs/findings/2026-08-v2-m4-cli-complete-spike.md`

- [ ] **Step 4: 静态验收（无产品代码）**

```bash
# 无意外产品实现
git diff main --stat | rg -n 'crates/|data/rules' && echo 'UNEXPECTED product diff' && exit 1 || echo 'ok no product paths required'
# stub 仍可 report
./scripts/check-command-surface.sh
# 版本未 bump
rg -n '^version = ' Cargo.toml
```

Expected：`version = "1.46.0"`（或当前 1.46.x）；stub report-only 成功。

- [ ] **Step 5: Commit**

```bash
git add docs/findings/2026-08-v2-m4-command-surface.md \
        docs/findings/2026-08-v2-m4-cli-complete-spike.md \
        docs/wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md
git commit -m "$(cat <<'EOF'
docs: ratify M4 command-surface matrix and design inputs

EOF
)"
```

---

## Self-review

1. **Spec coverage:**  
   - §5 M4 docs-only spike → 本计划全部 Task  
   - §3.1 对照核定 → Task 1 + Task 8 + Task 10  
   - §3.2 闸门形态草案 → Task 9（完整落地标收口）  
   - 六命令 + hints + 别名 + 裸调用 → Task 2–8  
   - 禁止实现 / 禁止空 bump 2.0.0 → Global Constraints + Task 10 Step 4  
2. **Placeholder scan:** 无 TBD；主路径/长尾均有具体条目；stub 含完整脚本正文。  
3. **一致性:** 命令顺序与规格一致（purge→hints→installer→touchid→update→remove）；`hints` 始终非顶层；`2.0.0` 仅指向 M5。

---

Plan complete and saved to `docs/wukong-code/plans/2026-08-08-2051-v2-m4-cli-complete-spike.md`. Two execution options:

**1. Subagent-Driven** — 每 Task 新 subagent，Task 间复审

**2. Inline Execution** — 本会话按 executing-plans 跑（**本仓默认**）

按仓库习惯默认 **Inline**，且用户已指示写好后立刻执行：默认选择 **2**，从 Task 1 连续跑完 Task 10，然后进入 M5（专用 design → plan → 实现）。
