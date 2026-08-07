# private-tmp（1.21.0）

Mole `system.sh` 对 `/private/tmp` 与 `/private/var/tmp`：probe 仅 maxdepth 1，但 `safe_sudo_find_delete` 默认可扫到深度 5。

Vole 本刀对齐 **probe**：仅相对根深度 1 的普通文件，年龄 ≥7 天（`MOLE_TEMP_FILE_AGE_DAYS`），apply 绑专用谓词 + `sudo -n` permanent。故意严于 Mole 删除面，避免深层临时树误伤。
