# `/Library/Caches` 临时文件清理设计（system.sh 续刀）

- 日期：2026-08-07；已批准（对齐 Mole 全树，含 com.apple）
- 依据：Mole `clean_deep_system` — `find /Library/Caches -maxdepth 5 -type f` 匹配 `*.cache`/`*.tmp`（`MOLE_TEMP_FILE_AGE_DAYS`）与 `*.log`（`MOLE_LOG_AGE_DAYS`），`safe_sudo_remove`
- 版本：**1.22.0**；规则 **527 → 528**

## 结论

- 一规则 `library-caches-temp`
- 目标：`/Library/Caches/**` 相对根深度 1..=5 普通文件，扩展名 `.cache` / `.tmp` / `.log`
- 年龄：`.cache`/`.tmp` ≥7d、`.log` ≥7d（分常量，当前值同为 7）；Privilege + apply 绑谓词 + `sudo -n`
- remap：`VOLE_TEST_SYSTEM_LIBRARY=$ROOT/Library` → `$ROOT/Library/Caches`

## 非目标

整目录 Icon Services store（另规则）、GPU Metal caches、`*.code_sign_clone`、交互提权；不打 tag
