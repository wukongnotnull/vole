# System services orphan（可读子集）落地 findings

**日期**：2026-08-05  
**状态**：实现完成，待 PR / 发版  
**包版本**：**1.5.0**

## 做了什么

1. `vole-core::sysorphan`：fail-closed 存在性探测、launchd Program 解析、known_protect、三树扫描
2. custom handler `orphaned_system_services` + 规则 `orphaned-system-services`
3. degrade → `NeedsPrivilege` + `SYSTEM_SERVICES_WARN`（不提 FDA）
4. clean apply 接线 `APPLY_PERMISSION_WARN`
5. coverage 句更新：可读子集已落地；仍未移植真 sudo 删除 / Containers stubs

## 安全要点

- 禁止 sudo
- Intego 类 root-only 树：祖先不可进入 → 不当 orphan（#1188 意图）
- package-managed 缺失不当 orphan
- PHT 仅 `-type f` + 扩展名黑名单 + reverse-DNS 前缀
- 发现优先契约写死在 impact / release / warn 文案

## 下一步

- PR → security-review → CI → merge → tag `v1.5.0` → Formula
