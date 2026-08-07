# Mole `system.sh` 差距 backlog（CLI · 桌面延后）

- 日期：2026-08-08（修订：同日本地快照报告合入后更新）
- 状态：已批准（盘点文档）；**不开实现**（除已合并项外），供后续选刀
- 依据：Mole `third_party/mole-1.48.1/lib/clean/system.sh`；Vole 至 **1.29.0**（含本地快照报告）；用户确认桌面 SMAppService 暂缓
- 范围：**仅** `system.sh` 收口；不含 purge / installer 子命令 / 宽开发者长尾 / 营销
- 更广路线图（uninstall / optimize 长尾、子命令与桌面边界、并行波次）：见 [`2026-08-08-0119-mole-parity-roadmap-design.md`](2026-08-08-0119-mole-parity-roadmap-design.md)

## 1. 结论

`clean_deep_system` **主清理链在 Vole 已对齐**（含特权 `sudo -n` 与 TTY `sudo -v`）。`system.sh` 可删函数中的实质删缺口 **`clean_time_machine_failed_backups` 已落地（1.28.0）**；报告面 **`clean_local_snapshots` 已落地（1.29.0）**。余量：

| 类别 | 项 | 状态 |
|---|---|---|
| **永不做** | `/Library/Updates`、`/macOS Install Data` | 与 Mole keep 一致；代码路径禁止删除 |
| **已落地** | `clean_time_machine_failed_backups` | 规则 `tm-failed-backups`；PR #67 / 1.28.0 |
| **已落地** | `clean_local_snapshots`（仅报告） | `status`/`analyze` 提示；PR #70 / 1.29.0；**不**进 clean apply 删 |
| **桌面** | SMAppService / PrivilegedHelper | **本 backlog 明确排除**（已延后） |

本文件 system 线可删/报告余量已收口；下一刀转向 parity roadmap **W2 并行池**。

## 2. `clean_deep_system` 对照表

对照 Mole `clean_deep_system` 段落顺序：

| Mole 段落 | Vole 规则 / 状态 |
|---|---|
| `/Library/Caches` `*.cache`/`*.tmp`/`*.log` 年龄扫 | `library-caches-temp` ✅ |
| `/private/tmp` + `/private/var/tmp` | `private-tmp` ✅ |
| `/Library/Logs/DiagnosticReports` 年龄叶 | `diagnostic-reports-system` ✅ |
| `/private/var/log` | `private-var-log` ✅ |
| Adobe / CreativeCloud / adobegc | `adobe-system-logs` ✅ |
| **Keep** `/Library/Updates`、`/macOS Install Data` | 明确永不做 ✅（与 Mole 一致） |
| Install macOS\*.app（age/SWU/版本/运行） | `install-macos-apps` ✅（1.27.0） |
| `*.code_sign_clone`（*/X/*，跳 EDR） | `code-sign-clone` ✅ |
| iconservices.store | `icon-services-system-cache` ✅ |
| GPU metal*/gpuarchiver（*/C/*，stale，跳 EDR） | `gpu-metal-caches` ✅ |
| idleassetsd `CFNetworkDownload_*.tmp` | `idleassetsd-cfnetwork-tmp` ✅ |
| `/private/var/db/diagnostics`（+ tracev3 30d） | `private-var-db-diagnostics` ✅ |
| DiagnosticPipeline | `private-var-db-diagnostic-pipeline` ✅ |
| powerlog | `private-var-db-powerlog` ✅ |
| MemoryLimitViolations | `private-var-db-memory-limit-violations` ✅ |

另：`rosetta-2-cache` 在 system 相关特权面已落地（非上表同函数，但属 `/Library` 系统特权清理）。

## 3. `system.sh` 其它函数

### 3.1 `clean_time_machine_failed_backups`（已落地）

- **行为**：在 Time Machine 已配置、非运行中、状态可知时，扫描备份卷上 `*.inProgress` / `*.inprogress` 失败中目录并清理；网络卷跳过；探测失败 fail-closed。
- **Vole**：规则 `tm-failed-backups`；计划 [`../plans/2026-08-08-0057-tm-failed-backups.md`](../plans/2026-08-08-0057-tm-failed-backups.md)；发版 **1.28.0** / PR #67。
- **删除语义**：仅 `tmutil delete`；不经 `PrivilegeBackend::remove_permanent`。

### 3.2 `clean_local_snapshots`（已落地 · 仅报告）

- **行为**：`tmutil listlocalsnapshots /` 后 **只打印数量 + review 提示**，不 `tmutil deletelocalsnapshots`。
- **Vole**：`status` / `analyze` 提示行；proto 可选 `local_snapshots`；发版 **1.29.0** / PR #70。
- **约束**：**禁止** clean apply 删除本地快照（除非未来另开产品决策并二次批准）。

## 4. 非目标（本文件明确写出）

- 桌面 SMAppService / PrivilegedHelper（vole-macos）
- Mole `purge` / `installer` 子命令
- `/Library/Updates`、`/macOS Install Data` 删除
- 宽 `dev.sh` / `user.sh` 规则长尾盘点（见 parity roadmap W2c）
- 打 Git tag / Homebrew 公证仪式

## 5. coverage 诚实面（已对齐）

当前 coverage「仍未移植」主要为桌面 SMAppService / 特权助手（TM 失败备份与本地快照报告均已从该句移除，与 1.28.0 / 1.29.0 一致）。

## 6. 推荐选刀顺序（CLI · 无桌面）

1. ~~TM 失败中备份~~ **已完成**（1.28.0）
2. ~~本地快照报告~~ **已完成**（1.29.0）
3. **W2 并行池**（推荐）— 见 [`2026-08-08-0119-mole-parity-roadmap-design.md`](2026-08-08-0119-mole-parity-roadmap-design.md) §3：W2a① / W2b① / W2c 窄规则任选首刀
4. （本文件范围外）发版仪式 / 营销

## 7. 验收（本文档）

- [x] `clean_deep_system` 逐段打勾
- [x] 永不做 Updates / Install Data 写死
- [x] TM 失败备份与快照报告分列；二者均 **已标落地**
- [x] 桌面排除
- [x] coverage 叙事与 1.28.0 / 1.29.0 一致

下一步：从 §6 第 3 项（W2 并行池）→ brainstorming / writing-plans。
