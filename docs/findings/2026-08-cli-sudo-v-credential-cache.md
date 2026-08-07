# CLI sudo -v 凭证缓存（1.26.0）

## 行为

- `PrivilegeBackend::acquire_interactive` → `sudo -v`（stdin TTY + 非 `VOLE_TEST_NO_AUTH`）
- `ApplyPlanContext::privilege_acquire_attempted` 闩：整次 apply 至多一次
- 删除仍仅 `sudo -n` + 既有 allowlist

## 验证

- RecordingPrivilege / `ensure_privilege_ready` 单测
- 既有特权 apply 单测全绿
