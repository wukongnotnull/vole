# idleassetsd CFNetworkDownload 清理设计（system.sh 续刀）

- 日期：2026-08-07；已批准
- 依据：Mole `clean_deep_system` — `find /private/var/folders -maxdepth 5 -type d -name com.apple.idleassetsd -path "*/T/*"`，再 `safe_sudo_find_delete` `CFNetworkDownload_*.tmp`（maxdepth 5、`MOLE_TEMP_FILE_AGE_DAYS`）
- 版本：**1.23.0**；规则 **528 → 529**

## 结论

- 一规则 `idleassetsd-cfnetwork-tmp`
- 目标：`/private/var/folders/**/T/com.apple.idleassetsd/**/CFNetworkDownload_*.tmp`（相对 idleassetsd 根深度 1..=5）
- 年龄 ≥7d；Privilege + apply 绑谓词 + `sudo -n`
- remap：`VOLE_TEST_SYSTEM_LIBRARY=$ROOT/Library` → `$ROOT/private/var/folders`

## 非目标

`/Library/Caches` idleassetsd、`*.code_sign_clone`、GPU Metal、交互提权；不打 tag
