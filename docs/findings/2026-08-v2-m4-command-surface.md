# M4：§3.1 命令面对照核定

**日期**：2026-08-08  
**状态**：已核定（M4 Task 8/10）  
**Mole 钉版**：`third_party/mole-1.48.1`  
**规格**：[`2026-08-08-2030-v2-cli-complete-design.md`](../wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md) §3.1  
**计划**：[`2026-08-08-2051-v2-m4-cli-complete-spike.md`](../wukong-code/plans/2026-08-08-2051-v2-m4-cli-complete-spike.md)

## 对照表

| Mole 命令 | Mole 实现 | Vole 1.46.0 | 规格处置 | M4 核定 |
|---|---|---|---|---|
| `clean` | `bin/clean.sh` | ✅ `Clean` | 已达 | **已达** |
| `uninstall` | `bin/uninstall.sh` | ✅ `Uninstall` | 已达 | **已达** |
| `optimize` / `optimise` | `bin/optimize.sh` | ✅ 无 `optimise` | 补别名 §3.3 | **缺口→建议 M5**（别名） |
| `analyze` / `analyse` | `bin/analyze.sh` | ✅ 无 `analyse` | 补别名 §3.3 | **缺口→建议 M5**（别名） |
| `status` | `bin/status.sh` | ✅ | 已达 | **已达** |
| `history` | early → `bin/history.sh` | ✅ | 已达 | **已达** |
| `completion` | `bin/completion.sh` | ⚠️ 仅 `completions` | 补别名 §3.3 | **缺口→建议 M5**（别名） |
| `help` / `--help` / `-h` | `show_help` | ✅ clap | 已达 | **已达** |
| `version` / `--version` / `-V` | `show_version` | ✅ clap | 已达 | **已达** |
| 裸调用 → 菜单 | `check_for_updates` 后菜单 | ✅ 菜单；无联网检查 | 已达；不跟进 §6.5 | **已达（故意差异）** |
| `purge` | `bin/purge.sh` + `project.sh` | ❌ | **M5** | **缺口→M5** |
| `installer` | `bin/installer.sh` | ❌ | **M7** | **缺口→M7** |
| `touchid` | `bin/touchid.sh` | ❌ | **M8** | **缺口→M8** |
| `update` | `lib/manage/update.sh` | ❌ | **M9** | **缺口→M9** |
| `remove` | `lib/manage/remove.sh` | ❌ | **M10** | **缺口→M10** |

## 豁免（不计入「⊇」顶层判据）

| 项 | 说明 | M4 核定 |
|---|---|---|
| `hints` | Mole 无 `mo hints`；`clean` 内只读模块；Vole **M6**；禁止顶层 `vole hints` | **豁免成立**；M6 交付模块 |
| `whitelist` | Mole 交互 manage；Vole 已有 `clean --whitelist*` | **豁免成立**（可接受形态差异） |

## Mole 路由抽取

来自 `third_party/mole-1.48.1/mole`：

`optimize|optimise`, `clean`, `uninstall`, `analyze|analyse`, `status`, `purge`, `installer`, `touchid`, `completion`, `update`, `remove`, `help|--help|-h`, `version|--version|-V`, `history`（early）, `""` → `check_for_updates` + 菜单。
