# System Services Orphan（可读子集）设计

- 日期：2026-08-05
- 状态：待实现（设计已批准）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/apps.sh` → `clean_orphaned_system_services`；B4 用户域 orphan（`2026-08-05-1642-b4-orphaned-app-data-design.md`）；权限响亮提示（`2026-08-05-2122-apply-permission-loud-hint-design.md`）；`SECURITY_AUDIT.md` orphaned system-service 条款
- 包版本意图：能力扩展 → **`1.5.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 上交付与 Mole 主循环同形、但 **不调用 sudo** 的 **system services orphan** 扫描：对 `/Library/LaunchDaemons`、`/Library/LaunchAgents`、`/Library/PrivilegedHelperTools` 的**当前用户可读子集**产出 plan 候选。

Apply **不**实现真提权删除；命中系统路径闸口时走既有 `NeedsPrivilege`，并复用 / 补齐 clean apply 上的权限响亮提示（与 1.4.2 uninstall/optimize 同风格）。

**采纳路径**：clean 内建 custom 规则（对齐 B4），非独立子命令。

## 2. 问题与风险

1. **误删高危**：LaunchDaemon / PrivilegedHelper 误判会破坏第三方 updater / VPN / 输入法。必须 fail-closed（读不到 ≠ 二进制缺失；Spotlight 失败 ≠ 未安装）。
2. **覆盖不全**：无 `sudo -n` 时大量 root 拥有 plist 不可读；必须响亮声明「可读子集」，避免用户以为已对齐 Mole 完整扫描。
3. **Apply 体验**：多数候选 apply 会 `NeedsPrivilege`——这是预期，不是 bug；产品价值在 **发现 + 指引**，删除能力仍归 Mole / 未来真 sudo 设计。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. clean custom 规则（推荐，已选）** | `orphaned-system-services` handler | 复用保护层 / 废纸篓 / oplog / 不可信 plan | apply 多跳过 |
| B. 只读 inventory 子命令 | 独立 CLI | 心智隔离 | 再造协议与菜单成本 |
| C. 近字节级 + `sudo -n` 探测 | 对齐 Mole 扫描 | 覆盖率高 | 偏离「本期不真 sudo」；交互/CI 复杂 |

## 4. 产品行为

### 4.1 用户可见

```bash
vole clean --plan                 # 可读范围内可含 system-services orphan 候选
vole clean --apply <plan.json>    # 系统路径预期 NeedsPrivilege + 响亮提示
```

- `rule_id`：`orphaned-system-services`
- label 示例：`Orphaned LaunchDaemon: com.example.helper`
- 环境变量：本期 **不增**；禁用走规则 `disabled = true`
- **不 bump** `schema_version`

### 4.2 扫描根（写死）

| 根 | 内容 |
|---|---|
| `/Library/LaunchDaemons` | `*.plist` |
| `/Library/LaunchAgents` | `*.plist` |
| `/Library/PrivilegedHelperTools` | 顶层文件 / helper bundle（maxdepth 对齐 Mole） |

**不扫**：`~/Library/LaunchAgents`、Containers / Group Containers、Application Scripts、任意需写 sudo 才列全的强制路径。

### 4.3 扫描权限（写死）

- **禁止**调用 `sudo` / `sudo -n` 做 find / PlistBuddy
- 目录不可列、plist 不可读、PrivilegedHelper 项不可读 → **跳过该项**（fail-closed）
- 三树皆无法有效扫描或「权限导致零可读」→ 整规则降级：`StreamEvent::Skipped { reason: TccDenied }` + `PlanNotice` + coverage 追加中文
- 部分可读：正常出候选 + **部分覆盖** notice

### 4.4 Apply

- 删除只经既有 `mole_delete_verified` / path validate；**不**新增特权删除通道
- `/Library/**` 预期大量 `NeedsPrivilege`
- Human stderr / `--json` `coverage_note`：沿用 `APPLY_PERMISSION_WARN`；若 clean apply 尚未接线，**本期补上**（与 uninstall/optimize 同逻辑）
- `--json-stream`：不额外刷中文

## 5. 判定流水线

### 5.1 LaunchDaemon / LaunchAgent plist

全部通过才入选：

1. 白名单 / `should_protect` / `validate_path_for_deletion` 前置闸口 → 否则跳过  
2. label / filename 匹配 `com.apple.*` → 跳过  
3. 以当前用户权限读取 `Program` 或 `ProgramArguments[0]`；失败 → 跳过（**禁止**把 PlistBuddy/解析失败当缺失）  
4. 值为绝对路径且目标 **不存在**  
5. 命中 `known_protect_patterns`（Mole 列表语义：关联 app 仍在 → 保护；空 app_path 如 `homebrew.mxcl.*` → 无条件保护）→ 跳过  
6. reverse-DNS bundle id 的 mdfind 回退：Spotlight 不可用 / 超时 / 非零 → **视为仍安装**（B4 同形）；明确空且 Spotlight 可用 → 可继续  

### 5.2 PrivilegedHelperTools

- 能列到的 helper：若判定所属 app **未安装**（安装扫描并集 + mdfind fail-closed）且非保护列表 → 候选  
- 列/读失败 → 跳过  

### 5.3 年龄

**不加** mtime 年龄闸（Mole 该函数无此闸）。

### 5.4 执行预算

- plist 解析与 mdfind 有超时；每次 plan mdfind 上限与 B4 同量级（复用预算常量或并列常量，实现计划写死数字）  
- 超限的 id 视为仍安装  

## 6. 覆盖说明文案（方向）

- 全局 `coverage_note`：标明 **system services orphan（可读子集）已落地**；仍未移植：**真 sudo 删除**、**Containers stubs**  
- 降级追加句（定稿实现时写死常量，风格对齐 `ORPHAN_LIBRARY_WARN`）：说明无 sudo、结果可能不全、完整清理请用 Mole 或未来提权能力  

## 7. 非目标

- 真 sudo / 特权助手删除  
- Containers / Group Containers stubs  
- 用户域 `~/Library/LaunchAgents` 作为本规则删除目标  
- `purge` / installer  
- 协议 `SkipReason` 新变体  

## 8. 测试与安全

- 单元 / fixture：缺失 Program 可读 plist → 入选；`com.apple.*`、known_protect、读失败 → 不入选；部分根不可读 → notice / 非整盘误报  
- 不依赖 CI runner 上真实 `/Library/PrivilegedHelperTools` 有内容（Mole 已文档化 runner 空洞）；fixture 注入  
- PR 合并前：security-review（系统路径扫描）  

## 9. 验收

1. `vole clean --plan` 在 fixture/可控环境下可列出 `orphaned-system-services` 候选  
2. 不可读场景不静默「零孤儿」冒充干净：有 Skipped / notice / coverage 之一  
3. apply 对系统路径 skip reason 正确，人读可见权限提示  
4. `cargo test` 相关包绿；coverage 句不再声称 system services orphan「仍未移植」（改为可读子集已落地）  
5. 发版 **1.5.0** + release notes  

## 10. 实现后文档

- `docs/releases/v1.5.0.md`  
- findings + Formula（发版流程同 1.4.x）  
- 真 sudo 删除另开 design（本 spec 明确 defer）  
