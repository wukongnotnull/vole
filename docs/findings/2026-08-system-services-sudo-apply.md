# System Services sudo -n apply（1.10.0）

为 `orphaned-system-services` 接通 CLI 非交互提权删除。

## 落点

- `vole-core::privilege`：`PrivilegeBackend`、三树 allowlist、`SudoNoninteractive` / `NoPrivilege` / `RecordingPrivilege`
- `mole_delete`：`needs_sudo` → Backend（先于 existence 检查）
- `apply_plan`：去掉硬 skip；allowlist → probe → unload → permanent

## 安全

- 参数分列 `sudo -n`；禁 shell 拼接
- allowlist 仅 `/Library/LaunchDaemons|LaunchAgents|PrivilegedHelperTools/`
- `VOLE_TEST_NO_AUTH` 永不真 sudo
- 提权路径不进废纸篓
