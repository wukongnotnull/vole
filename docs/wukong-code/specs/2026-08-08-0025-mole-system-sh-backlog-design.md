# Mole `system.sh` 差距 backlog（CLI · 桌面延后）

- 日期：2026-08-08
- 状态：已批准（盘点文档）；**不开实现**，供后续选刀
- 依据：Mole `third_party/mole-1.48.1/lib/clean/system.sh`；Vole privilege 规则至 **1.27.0**；用户确认桌面 SMAppService 暂缓
- 范围：**仅** `system.sh` 收口；不含 purge / installer 子命令 / 宽开发者长尾 / 营销
- 更广路线图（uninstall / optimize 长尾、子命令与桌面边界、并行波次）：见 [`2026-08-08-0119-mole-parity-roadmap-design.md`](2026-08-08-0119-mole-parity-roadmap-design.md)

## 1. 结论

`clean_deep_system` **主清理链在 Vole 已基本对齐**（含特权 `sudo -n` 与 TTY `sudo -v`）。`system.sh` 余量只剩：

| 类别 | 项 | 建议 |
|---|---|---|
| **永不做** | `/Library/Updates`、`/macOS Install Data` | 与 Mole keep 一致；代码路径禁止删除 |
| **可选高风险删** | `clean_time_machine_failed_backups` | 单独 design；fail-closed；体积可能很大 |
| **报告向、不删** | `clean_local_snapshots` | Mole 仅提示 `tmutil listlocalsnapshots`；适合 `status`/`analyze`，不进 `clean --apply` 删 |
| **桌面** | SMAppService / PrivilegedHelper | **本 backlog 明确排除**（已延后） |

下一实现刀若继续走 system 线：优先 **TM 失败中备份**（唯一仍可删且未移植的 system 函数）；或先把 coverage「仍未移植」补上「Time Machine 失败备份 / 快照报告」以保持诚实。

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

### 3.1 `clean_time_machine_failed_backups`（唯一实质删缺口）

- **行为**：在 Time Machine 已配置、非运行中、状态可知时，扫描备份卷上 `*.inProgress` / `*.inprogress` 失败中目录并清理；网络卷跳过；探测失败 fail-closed。
- **风险**：误删进行中备份 → 数据不可恢复；依赖 `tmutil` / 卷枚举 / 文件系统类型。
- **依赖**：不宜默认并入日常 `clean`；需清晰 skip 文案与 privilege/路径闸口；本机 `/Volumes` 场景难 hermetic 测。
- **建议版本**：若做，单独 MINOR；强制 security-review；先写 dedicated design。

### 3.2 `clean_local_snapshots`（Mole 亦不删除）

- **行为**：`tmutil listlocalsnapshots /` 后 **只打印数量 + review 提示**，不 `tmutil deletelocalsnapshots`。
- **建议**：挂 `status` 或 `analyze` 提示行；**禁止** clean apply 删除本地快照（除非未来另开产品决策并二次批准）。

## 4. 非目标（本文件明确写出）

- 桌面 SMAppService / PrivilegedHelper（vole-macos）
- Mole `purge` / `installer` 子命令
- `/Library/Updates`、`/macOS Install Data` 删除
- 宽 `dev.sh` / `user.sh` 规则长尾盘点（另开文档）
- 打 Git tag / Homebrew 公证仪式

## 5. coverage 建议（诚实面）

当前「仍未移植」仅写桌面。system 收口后更诚实的二选一：

- **A.** 维持现状（桌面一句话）；TM 未写入 coverage，避免吓用户  
- **B.** 改为：`仍未移植：Time Machine 失败备份清理、本地快照报告、桌面 SMAppService…`

本 backlog **推荐 B**（若下一刀不做 TM，也至少在 findings 存档；coverage 可等开刀前再改，避免空头支票）。

## 6. 推荐选刀顺序（CLI · 无桌面）

1. **（可选）coverage 诚实化** — docs/小改，PATCH  
2. **TM 失败中备份** — 高风险 system 余刀；或先 spike design  
3. **本地快照报告** — status/analyze；低删风险  
4. （本文件范围外）宽 Mole 差距 / 发版仪式 / 营销  

## 7. 验收（本文档）

- [x] `clean_deep_system` 逐段打勾  
- [x] 永不做 Updates / Install Data 写死  
- [x] TM 失败备份与快照报告分列  
- [x] 桌面排除  

下一步：用户从 §6 选一刀 → 走 brainstorming/writing-plans（TM 必须单独 design）。
