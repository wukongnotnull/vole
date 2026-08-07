# Icon Services 系统缓存（1.13.0）

Mole `system.sh` 首点：`/Library/Caches/com.apple.iconservices.store`。

## 落点

- `is_icon_services_system_cache` exact 谓词（+ test remap）
- Privilege allow + plan candidates（无 arch 门控）
- apply：绑定 exact + `sudo -n` permanent（防 rule_id 篡改走三树）

## 安全

- 不放宽 `/Library/Caches/**`
- 无 unload；无交互 sudo
