# Rosetta `/Library` update bundle（1.12.0）

落地 Mole `safe_clean /Library/Apple/usr/share/rosetta/rosetta_update_bundle`（仅 arm64）。

## 落点

- `is_rosetta_update_bundle`：exact critical 豁免（+ `VOLE_TEST_SYSTEM_LIBRARY` 映射）
- `path_allowed_for_privilege`：exact allow（三树叶级不变）
- `is_arm64_host` / `VOLE_TEST_FORCE_UNAME_M`：plan 与 apply 门控
- 规则 `rosetta-2-cache`；apply：`sudo -n` permanent，无 unload

## 安全

- 禁止放宽 `/Library/Apple/**`
- 父目录与嵌套子路径拒绝
- 无凭证 → `NeedsPrivilege`
