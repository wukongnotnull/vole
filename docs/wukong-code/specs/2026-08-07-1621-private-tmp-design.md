# `/private/tmp` + `/private/var/tmp` 清理设计（system.sh 续刀）

- 日期：2026-08-07；已批准
- 依据：Mole `system.sh` temp dirs — probe `maxdepth 1 -type f -mtime +MOLE_TEMP_FILE_AGE_DAYS`；删除走 `safe_sudo_find_delete`（默认 maxdepth 5）
- 版本：**1.21.0**；规则 **526 → 527**

## 结论

- 一规则 `private-tmp`
- 目标：`/private/tmp/*`、`/private/var/tmp/*` 相对根 **深度恰好 1** 的普通文件（对齐 probe；故意严于 Mole 删除 maxdepth 5）
- 年龄 ≥7d（`MOLE_TEMP_FILE_AGE_DAYS`）；Privilege + apply 绑谓词 + `sudo -n` permanent
- remap：`VOLE_TEST_SYSTEM_LIBRARY=$ROOT/Library` → `$ROOT/private/tmp`、`$ROOT/private/var/tmp`

## 非目标

深层 tmp、idleassetsd `CFNetworkDownload_*.tmp`、交互提权；不打 tag
