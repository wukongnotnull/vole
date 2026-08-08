# M4：§3.1 命令面对照核定

**日期**：2026-08-08  
**状态**：骨架（Task 1）；核定列在 Task 8/10 填完  
**Mole 钉版**：`third_party/mole-1.48.1`  
**规格**：[`2026-08-08-2030-v2-cli-complete-design.md`](../wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md) §3.1  
**计划**：[`2026-08-08-2051-v2-m4-cli-complete-spike.md`](../wukong-code/plans/2026-08-08-2051-v2-m4-cli-complete-spike.md)

## 对照表

| Mole 命令 | Mole 实现 | Vole 1.46.0 | 规格处置 | M4 核定 |
|---|---|---|---|---|
| `clean` | `bin/clean.sh` | ✅ `Clean` | 已达 | 待填 |
| `uninstall` | `bin/uninstall.sh` | ✅ `Uninstall` | 已达 | 待填 |
| `optimize` / `optimise` | `bin/optimize.sh` | ✅ 无 `optimise` 别名 | 补别名 §3.3 | 待填 |
| `analyze` / `analyse` | `bin/analyze.sh` | ✅ 无 `analyse` 别名 | 补别名 §3.3 | 待填 |
| `status` | `bin/status.sh` | ✅ | 已达 | 待填 |
| `history` | early dispatch → `bin/history.sh` | ✅ | 已达 | 待填 |
| `completion` | `bin/completion.sh` | ⚠️ 仅 `completions` | 补别名 §3.3 | 待填 |
| `help` / `--help` / `-h` | `show_help` | ✅ clap | 已达 | 待填 |
| `version` / `--version` / `-V` | `show_version` | ✅ clap | 已达 | 待填 |
| 裸调用 → 菜单 | `check_for_updates` 后菜单 | ✅ 菜单；**无**联网检查 | 已达；故意不跟进 §6.5 | 待填 |
| `purge` | `bin/purge.sh` + `lib/clean/project.sh` | ❌ | **M5** | 待填 |
| `installer` | `bin/installer.sh` | ❌ | **M7** | 待填 |
| `touchid` | `bin/touchid.sh` | ❌ | **M8** | 待填 |
| `update` | `lib/manage/update.sh`（sourced） | ❌ | **M9** 自更新 | 待填 |
| `remove` | `lib/manage/remove.sh`（sourced） | ❌ | **M10** 自卸载 | 待填 |

## 豁免（不计入「⊇」顶层判据）

| 项 | 说明 | M4 核定 |
|---|---|---|
| `hints` | Mole：`lib/clean/hints.sh`，由 `clean.sh` source；**无** `mo hints`。Vole 按 **M6** 做 clean 内只读提示；**禁止**顶层 `vole hints` | 待填 |
| `whitelist` | Mole：optimize/clean 内交互 `manage_whitelist`；Vole 已有 `clean --whitelist*`。能力已有、形态不同，可接受差异 | 待填 |

## Mole 路由抽取（Task 1）

来自 `third_party/mole-1.48.1/mole` `main()` case + `mole_dispatch_history_early`：

`optimize|optimise`, `clean`, `uninstall`, `analyze|analyse`, `status`, `purge`, `installer`, `touchid`, `completion`, `update`, `remove`, `help|--help|-h`, `version|--version|-V`, `history`（early）, `""` → `check_for_updates` + 交互菜单。
