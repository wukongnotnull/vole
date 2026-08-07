# Time Machine 失败中备份清理设计

- 日期：2026-08-08
- 状态：待用户审阅 spec → writing-plans
- 依据：Mole `clean_time_machine_failed_backups` / `tm_is_running`（`third_party/mole-1.48.1/lib/clean/system.sh`）；`MOLE_TM_BACKUP_SAFE_HOURS=48`；system.sh backlog §3.1
- 包版本意图：**1.28.0**（MINOR）；规则 **532 → 533**

## 1. 结论

新增 clean 规则 **`tm-failed-backups`**（默认启用）：

- 在 Time Machine **已配置**、备份 **未运行且状态可知**、本地备份卷可扫时，选入年龄 **≥ 48 小时** 的 `*.inProgress` / `*.inprogress` 目录
- 范围对齐 Mole 全量：**HFS `Backups.backupdb`** + **已挂载** `*.backupbundle` / `*.sparsebundle` 内同形目录
- **apply**：`tmutil delete <path>`（参数分列）；失败 → skip + 提示手动，**不**自动 `sudo`，**不**走 `sudo -n rm`
- plan **永不**调用 `tmutil delete`

**采纳路径**：方案 A — 自定义 handler + 可注入 `TmDeps`；非 analyze-only。

## 2. 问题与风险

1. **误删进行中备份**：`tmutil status` Running=1 **或无法判定** → 整规则零候选（fail-closed）。
2. **mtime 失败被当成「很旧」**：stat 失败 / 0 epoch → **keep**（对齐 Mole）。
3. **安全窗过短**：`MOLE_TM_BACKUP_SAFE_HOURS = 48` 写死为常量 `TM_BACKUP_SAFE_HOURS`。
4. **网络卷误扫**：nfs/smbfs/afpfs/cifs/webdav/**unknown** 跳过。
5. **sudo rm 扩大面**：禁止；仅 `tmutil delete`。
6. **协议**：不 bump `schema_version`。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. custom + `tmutil delete`（已选）** | 门控/选入/apply 一体 | 对齐 Mole；可测 | 外部工具依赖 |
| B. analyze 报告 only | 零删除 | 安全 | 不兑现 coverage 删项 |
| C. sudo rm allowlist | 硬删 | 少依赖 tmutil | 偏离 Mole；危险 |

## 4. 产品行为

```bash
vole clean --plan …     # 只读扫描；可进入 plan
vole clean --apply …    # 对条目执行 tmutil delete；TM 正在跑则整规则/重判 skip
```

- 默认纳入日常 clean（有门控）
- 本地快照报告 **不在本刀**
- stderr / PlanNotice：Running 或 status unknown 时可一句「Time Machine cleanup skipped (…)」

## 5. 门控与选入（顺序写死）

1. `tmutil` 存在，否则空  
2. `/Library/Preferences/com.apple.TimeMachine`（或 `defaults read` 等价）`AutoBackup` ∈ {0,1}，否则空  
3. `tmutil destinationinfo`：超时/失败/含 `No destinations configured` → 空  
4. volumes 根（默认 `/Volumes`）不存在 → 空  
5. `tm_is_running`：**Running** 或 **Unknown** → 空（可记 notice）  
6. 枚举 volumes：目录、非 symlink；跳过明显本机根别名；有 `Backups.backupdb` 或 `.MobileBackups` → 候选卷  
7. `df -T`（或等价）：网络 FS 与 **unknown** → skip 该卷  
8. **HFS 路径**：`{vol}/Backups.backupdb` 下 maxdepth ≤3、类型目录、名匹配 `*.inProgress`|`*.inprogress`  
9. **Bundle 路径**：卷上 `*.backupbundle`/`*.sparsebundle`；用 `hdiutil info` 解析已挂载点；在挂载点下同 8  
10. mtime 可读且 >0；`(now - mtime) ≥ 48h`；目录 size > 0  
11. → plan：`rule_id=tm-failed-backups`，path=绝对路径  

测试：`VOLE_TEST_VOLUMES`（或注入 volumes_root）+ FakeTmDeps；生产不读测试 env。

## 6. Apply

1. 形状 allowlist（见 §7）+ 仍是目录  
2. 重验：running/unknown → skip；age &lt; 48h 或 mtime 坏 → skip  
3. `tmutil delete --` 不需要；Mole 为 `tmutil delete "$path"` → `Command::new("tmutil").arg("delete").arg(path)`  
4. 成功 → report；失败 → skip + 提示（可继续用 Mole / 手动 sudo）  
5. **禁止** `PrivilegeBackend::remove_permanent` / `mole_delete` 对本规则

## 7. Allowlist / 形状

路径必须同时满足：

- 绝对路径、无 `..`  
- 某一段为 `Backups.backupdb` **或** 位于已记录的 bundle 挂载前缀下  
- 叶目录名以 `.inProgress` / `.inprogress` 结尾（大小写两种 Mole 已覆盖）  
- maxdepth 语义：相对 backupdb 或挂载根深度 ≤3  

越界如 `/tmp/foo.inProgress` → apply 拒绝。

## 8. 覆盖说明

- coverage：去掉「Time Machine 失败中备份清理」未移植  
- 仍未移植：本地快照报告、桌面 SMAppService / 特权助手  
- README 成熟度可提一句 TM failed backups（可选）

## 9. 测试与安全

1. Fake：无 tmutil / AutoBackup 坏 / no destination / Running / Unknown → 零条目  
2. ≥48h fixture inProgress → 进 plan；&lt;48h → 不进  
3. 网络 FS / unknown → 不进  
4. apply：delete 被调用一次；运行中重判 → 零 delete  
5. 越界 path + 正确 rule_id → 不删  
6. PR：**security-review** 必过  

## 10. 验收

1. 本机有陈旧 inProgress、TM 空闲时 plan 含该项；apply 后目录消失（或 tmutil 报错则 skip）  
2. 备份进行中：计划为空或 apply skip  
3. 版本 **1.28.0**；规则 533；默不打 tag  

## 11. 下一步

用户批准本文件 → `writing-plans` → `docs/wukong-code/plans/…-tm-failed-backups.md`。
