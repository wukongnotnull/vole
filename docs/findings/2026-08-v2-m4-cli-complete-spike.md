# M4：CLI 做全 · Mole 库存与安全面 Spike

**日期**：2026-08-08  
**状态**：完成  
**Mole 钉版**：`third_party/mole-1.48.1`  
**规格**：[`2026-08-08-2030-v2-cli-complete-design.md`](../wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md)  
**计划**：[`2026-08-08-2051-v2-m4-cli-complete-spike.md`](../wukong-code/plans/2026-08-08-2051-v2-m4-cli-complete-spike.md)  
**命令面表**：[`2026-08-v2-m4-command-surface.md`](2026-08-v2-m4-command-surface.md)  
**闸门清单**：[`2026-08-v2-m4-gate-checklist.md`](2026-08-v2-m4-gate-checklist.md)

## 1. 结论

M4 **docs-only** 已完成：核定 Mole 1.48.1 命令面（§3.1）、六命令 + `hints` 主路径/长尾与安全面，留下 §3.2 闸门 stub（`scripts/check-command-surface.sh`）。当前包版本仍为 **1.46.0**；产品缺口为 `purge`/`installer`/`touchid`/`update`/`remove` 与三个别名。下一项实现：**M5 `purge` → 2.0.0**（建议同 PR 补别名）。本 spike **未**改产品行为、**未**空 bump MAJOR。

## 2. §3.1 核定摘要

见 [`2026-08-v2-m4-command-surface.md`](2026-08-v2-m4-command-surface.md)（已核定）。摘要：已达项含 clean/uninstall/optimize/analyze/status/history/help/version/裸调用菜单；缺口为五命令 + `optimise`/`analyse`/`completion`；豁免 `hints`/`whitelist`；裸调用不联网为故意差异。

## 3. 分命令库存

### 3.1 `purge`（→ M5）

**Mole 入口：** `bin/purge.sh` → `start_purge` / `perform_purge`（逻辑在 `lib/clean/project.sh` + `lib/clean/purge_shared.sh`）。  
**配置：** `~/.config/mole/purge_paths`；`--paths` → `lib/manage/purge_paths.sh` `manage_purge_paths`。  
**对照 bats：** `tests/purge.bats`、`purge_config_paths.bats`。

| 面 | Mole 事实 |
|---|---|
| Flag | `--dry-run`/`-n`、`--paths`、`--include-empty`、`--debug`、`--help`（**无**独立 `--apply`；TTY 确认后删除） |
| Targets | `MOLE_PURGE_TARGETS`：`node_modules` `target` `build` `dist` `venv` `.venv` `.pytest_cache`（7） |
| 默认搜索根 | `www` `dev` `Projects` `GitHub` `Code` `Workspace` `Repos` `Development` `Library/CloudStorage` `$HOME` `.codex/worktrees` `.claude/worktrees`（12；点目录仅显式） |
| 指标 | monorepo 4；project indicators 16（含 `package.json`/`Cargo.toml`/`.git` 等） |
| 年龄 / 深度 | `MIN_AGE_DAYS=7`；depth 默认 1–6 |
| 超时 | 扫描约 60s（`MO_PURGE_SCAN_TIMEOUT_SEC`）；activity total / size du 有界；超时 fail-closed |
| 删除 | 经 `mole_delete`；云同步项非交互跳过或需确认 |

**Vole 映射建议：** Mole dry-run ≈ `--plan`；交互确认删除 ≈ `--apply` + TTL/TOCTOU；增 JSON / `plan_out` / `--permanent`；配置名 `purge_paths`（或 `~/.config/vole/...` 等价）。

**主路径（建议进 M5 / 2.0.0）**

1. 发现项目根（默认搜索路径 + `$HOME/*/` 容器探测；点目录仅显式列表）
2. 按钉版 `PURGE_TARGETS` 匹配重建型产物
3. 年龄门槛（默认 7 天）与扫描/探测超时
4. `--plan` / `--apply` 两阶段；JSON；默认废纸篓
5. 删除走 `mole_delete_verified` + 保护层；`purge_paths` 配置
6. 菜单 + 补全；建议同 PR 交付 §3.3 别名

**长尾**

- TTY 分页多选 UI（用 plan JSON 代替）
- 整棵 worktree「可删」判定（Mole AGENTS 禁止；只清产物）
- 未在钉版 targets 证明的扩张目录名
- sudo / 系统域；cloud 特殊交互的全量复刻

**安全面：** 禁止平行 `rm -rf`；gitignore/秘密不能当可删证据；与规格 §6.1–6.2 对齐。

### 3.2 `hints`（→ M6，非顶层命令）

**硬核定：** Mole `mole` 路由表**无** `hints`；由 `bin/clean.sh` `source lib/clean/hints.sh` 并在 clean 流程调用 `show_*_hint_notice`。**禁止**实现顶层 `vole hints`。

**对照 bats：** `tests/clean_hints.bats`。

| 家族 | 关键函数 | 备注 |
|---|---|---|
| 项目产物提示 | `probe_project_artifact_hints` / `show_project_artifact_hint_notice` | wall-clock `MOLE_TIMEOUT_HINT_SCAN_SEC`（默认 ~15s）；慢扫跳过 |
| 系统数据提示 | `show_system_data_hint_notice` | 大路径线索，只读 |
| LaunchAgent 提示 | `show_user_launch_agent_hint_notice` | 缺失 app 目标等；信任已有可执行 Program |
| 孤儿点目录 | `show_orphan_dotdir_hint_notice` | GUI app / claude plugin 归属探测 |
| quick purge 探针 | `load_quick_purge_hint_paths` / `is_quick_purge_project_root` / `record_project_artifact_hint` | 与 purge targets 复用精神 |

**主路径：** 只读；超时与浅扫预算；慢路径降级跳过，不阻塞 clean；可含 purge 快捷探针子集。  
**长尾：** 全量 Mole 文案/交互细节；任何第二套删除路径（禁止）。  
**安全面：** hints 不得调用删除漏斗；超时失败偏向「跳过提示」而非阻塞。

### 3.3 `installer`（→ M7）

**Mole 入口：** `bin/installer.sh`。  
**对照 bats：** `installer.bats`、`installer_fd.bats`、`installer_zip.bats`。

| 面 | Mole 事实 |
|---|---|
| 扫描根 | Downloads / Desktop / Documents / Public / Library/Downloads / Shared / Homebrew Caches / iCloud Downloads / Mail Downloads / Telegram Desktop 等（`INSTALLER_SCAN_PATHS`） |
| 深度 | `INSTALLER_SCAN_MAX_DEPTH_DEFAULT=2` |
| Plan | `build_installer_delete_plan` → 校验 → `execute_installer_delete_plan` |
| 删除 | `mole_delete`；失败可 `INSTALLER_EXIT_INCOMPLETE=3` |
| Flag | `--dry-run`/`-n` 等；TTY 分页选择 |

**主路径：** 扫描安装包 → plan → apply；immutable delete-plan 校验精神对齐；删除走安全漏斗；JSON；首批扫描根优先 Downloads/Desktop（+ 明确高价值根）。  
**长尾：** 全量冷门扫描根、TTY 分页选择器、zip/fd 边缘第二批。  
**安全面：** plan 不可信则 fail-closed；禁止绕过漏斗；incomplete cleanup 须可观测退出码/报告。

### 3.4 `touchid`（→ M8）

**Mole 入口：** `bin/touchid.sh`（`mole` `exec`）。  
**PAM：** 优先 `sudo_local`（`pam_tid.so`）；legacy `/etc/pam.d/sudo` 迁移/清理；行常量 `auth       sufficient     pam_tid.so`。  
**子命令形态：** `enable` / `disable` / `status`（及菜单）。  
**Dry-run：** `touchid_dry_run_enabled`；测试护栏对齐 `MOLE_TEST_NO_AUTH` / `MOLE_TEST_MODE`。

**主路径：** 状态查询 + 启用/禁用引导；优先 `sudo_local`；安全回滚路径在 M8 design 写死；`VOLE_TEST_NO_AUTH` 下可测。  
**长尾：** 真机授权演示、非 sudo PAM 扩展。  
**硬约束：** 验证路径**禁止**触发真 Touch ID / 交互 sudo 挂起（规格 §6.4）。

### 3.5 `update`（→ M9）

**Mole：** `lib/manage/update.sh` 被 `mole` **source**（非 exec），供菜单与 banner 同进程调用。  
**CLI：** `mole update [--force|-f] [--nightly]` → `update_mole`。  
**裸调用：** `""` 分支先 `check_for_updates`（写 cache / banner），再菜单——**Vole 故意不跟进**（规格 §6.5）。  
**对照 bats：** `tests/update.bats`；安装校验精神见 Mole AGENTS / `install.sh`（checksum/attestation fail-closed）。

| 面 | Mole 事实 |
|---|---|
| 检测 | GitHub latest + Homebrew outdated/info；nightly 比 commit |
| 安装形态 | `is_homebrew_install` / `is_homebrew_mole_path` vs 手动前缀（`resolve_mole_source_path` / `MOLE_ENTRY_SCRIPT`） |
| brew 路径 | 走 `update_via_homebrew`；nightly 对 brew 拒绝 |
| 成功判据 | 安装后二进制版本（非安装器 stdout） |
| 自愈 | `_update_self_heal_reinstall`（破损本地引导）；不得变成「校验失败仍继续」后门 |

**主路径：** 显式 `vole update` 自更新通道（非仅 brew 包装）；`--force` / `--nightly`（名称以 M9 design 为准）；brew 默认提示 `brew upgrade`，自更新须显式确认或 `--force`。  
**故意差异：** 裸调用**不联网**。  
**硬约束：** 校验失败 fail-closed；禁止静默降级未校验源码安装；发版资产（签名/公证）须能支撑通道。

### 3.6 `remove`（→ M10）

**Mole：** `lib/manage/remove.sh` `remove_mole`（sourced）。  
**CLI：** `mole remove [--dry-run|-n]`。

| 安装形态 | 探测要点 |
|---|---|
| Homebrew | `brew_mole_formula_installed` / `is_homebrew_install` → 提示/执行 `brew uninstall` |
| 手动前缀 | `command -v mole` + `/usr/local` `$HOME/.local` `/opt/local`；排除 Cellar symlink |
| Alias / 残留 | `mo` 同路径集合；非 Cellar 的 alias 文件 |

另清理自身 config/cache/logs（范围须在 M10 design 逐项写死）。`--dry-run` 只打印 Would remove。

**主路径：** `vole remove --dry-run` 预览；仅删本工具安装产物与自身配置；brew 提示 `brew uninstall`；删除走路径校验漏斗。  
**长尾 / 禁区：** 用户数据、oplog/审计（除非显式要求）、其它 brew 包。  
**耦合：** 与 M9 共享安装来源判定 API；规格允许合并为单里程碑。

## 4. 别名与裸调用

### 4.1 别名缺口（§3.3）

| Mole | Vole 1.46.0（`crates/vole-cli/src/main.rs`） | 处置 |
|---|---|---|
| `completion` | 仅 `Completions`（`completions`） | 补 `completion` visible_alias；保留 `completions` 主名 |
| `optimise` | 仅 `Optimize` | 补英式别名 |
| `analyse` | 仅 `Analyze` | 补英式别名 |

建议随 **M5** 同 PR 交付，使 `2.0.0` 即具备完整命令名兼容（规格 §3.3）。纳入 `--help` 与 shell 补全。

### 4.2 裸调用不联网（§6.5）

- Mole：`""` → `check_for_updates` 再 `interactive_main_menu`
- Vole：`Command::None` → `interactive::run()`；菜单仅 spawn 本地子命令（status/clean/uninstall/optimize/history）；**无** GitHub/brew 版本探测
- **核定：** 故意差异，已达；收口闸门须静态断言 `interactive.rs` 无更新探测联网调用

## 5. 主路径 vs 长尾总表

| 能力 | 主路径 | 长尾 / 不做 |
|---|---|---|
| purge | 发现→targets→年龄/超时→plan/apply→废纸篓→JSON→配置 | TTY 多选；worktree 整树删除；未证明 target 扩张 |
| hints | clean 内只读探针子集 + 预算降级 | 顶层命令；第二套删除 |
| installer | 扫描→immutable plan→apply→漏斗 | 冷门扫描根；TTY 分页全量 |
| touchid | status/enable/disable；sudo_local；可测无真授权 | 真机演示；非 sudo PAM |
| update | 显式自更新；校验 fail-closed；brew 共存策略 | 裸调用联网检查（禁止） |
| remove | dry-run；三类安装形态；自身产物/配置 | 用户数据；其它 brew 包 |
| 别名 | completion/optimise/analyse | — |

## 6. 安全面与禁区

- 删除类命令（purge / installer / remove）必须走既有安全漏斗；禁止平行 `rm -rf`
- 不删本地快照（apply）；不删 `/Library/Updates`、`/macOS Install Data`
- `hints` 只读；`touchid` 验证禁真授权挂起；`update` 校验 fail-closed；裸调用不联网

## 7. §3.2 闸门草案

- 清单：[`2026-08-v2-m4-gate-checklist.md`](2026-08-v2-m4-gate-checklist.md)
- Stub：`scripts/check-command-surface.sh`（默认 report-only；`--enforce` 供收口 CI）
- M4 实测：report-only exit 0 并报告 `MISSING: optimise|analyse|completion|purge|installer|touchid|update|remove`；`--enforce` 非 0；`interactive.rs` 无更新探测标记 OK
- 完整强制留给收口里程碑

## 8. 后续 design 输入清单（M5–M10）

| 里程碑 | 专用 design 必答 |
|---|---|
| **M5 purge** | 1) Mole dry-run/确认 → Vole `--plan`/`--apply` 映射 2) `PURGE_TARGETS` 是否原样钉死 3) 年龄/深度/超时默认值 4) `purge_paths` 配置路径与格式 5) Plan `rule_id` 前缀与 schema 是否零变更 6) 云同步/非交互跳过策略 7) 菜单/补全条目 8) §3.3 别名是否同 PR |
| **M6 hints** | 1) 挂在 clean 哪一阶段 2) 预算秒数与降级语义 3) 首批探针子集（建议含 project artifact + quick purge） 4) 禁止顶层 `hints` 的测试 5) 与 purge 配置/targets 是否共享只读视图 |
| **M7 installer** | 1) 首批扫描根 2) immutable delete-plan 字段与校验 3) zip/pkg/dmg 范围 4) incomplete exit 语义 5) JSON/plan_out |
| **M8 touchid** | 1) PAM 路径注入点（可测） 2) enable/disable 回滚步骤 3) `VOLE_TEST_NO_AUTH` 契约 4) 禁止真授权挂起的回归方式 |
| **M9 update** | 1) Release 资产与校验（checksum/签名） 2) brew 提示 vs `--force` 3) nightly 是否支持 4) 成功判据=`vole --version` 5) 裸调用不联网回归 6) 与 remove 共享的安装来源 API |
| **M10 remove** | 1) 删除白名单逐项 2) dry-run 输出契约 3) brew/手动/alias 三类 4) oplog/审计是否保留 5) 是否与 M9 合并里程碑 |

## 9. 明确未做

- 无 `crates/` / `data/rules` 产品行为变更
- 无包版本 bump
- §3.2 完整 CI 强制留给收口；M4 仅 stub + 清单
