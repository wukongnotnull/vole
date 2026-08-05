# System Services Orphan（可读子集）设计

- 日期：2026-08-05（同日审阅修订：无 sudo 存在性探测 fail-closed、package-managed 排除、PHT 父 app 判定 #1082、降级 reason 改 NeedsPrivilege、PHT 范围收紧、mdfind 定位对齐 Mole、发现优先契约）
- 状态：待实现（设计已批准；审阅修订已并入）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/apps.sh` → `clean_orphaned_system_services`；B4 用户域 orphan（`2026-08-05-1642-b4-orphaned-app-data-design.md`）；权限响亮提示（`2026-08-05-2122-apply-permission-loud-hint-design.md`）；`SECURITY_AUDIT.md` orphaned system-service 条款
- 包版本意图：能力扩展 → **`1.5.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 上交付与 Mole 主循环同形、但 **不调用 sudo** 的 **system services orphan** 扫描：对 `/Library/LaunchDaemons`、`/Library/LaunchAgents`、`/Library/PrivilegedHelperTools` 的**当前用户可读子集**产出 plan 候选。

Apply **不**实现真提权删除；命中系统路径闸口时走既有 `NeedsPrivilege`，并复用 / 补齐 clean apply 上的权限响亮提示（与 1.4.2 uninstall/optimize 同风格）。

**硬契约（发现优先，删除不承诺）**：本期产品价值 = **发现 + 指引**。系统路径候选进入 plan 即视为「预期 apply 会 `NeedsPrivilege` 跳过」；人读 plan 摘要与 coverage 文案必须明说「这些条目当前无法由 Vole 删除，删除请用 Mole 或未来提权能力」。禁止任何让用户误以为 trash 能清系统服务的措辞。

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
| `/Library/PrivilegedHelperTools` | 仅顶层**普通文件**（`-type f`、maxdepth 1；范围细则见 §5.2） |

**不扫**：`~/Library/LaunchAgents`、Containers / Group Containers、Application Scripts、任意需写 sudo 才列全的强制路径。

### 4.3 扫描权限（写死）

- **禁止**调用 `sudo` / `sudo -n` 做 find / PlistBuddy / 存在性探测
- 目录不可列、plist 不可读、PrivilegedHelper 项不可读 → **跳过该项**（fail-closed）
- 三树皆无法有效扫描或「权限导致零可读」→ 整规则降级：`StreamEvent::Skipped { reason: NeedsPrivilege }` + `PlanNotice` + coverage 追加中文。**不用 `TccDenied`**：`/Library` 列不出/root plist 读不了是权限/所有权问题，不是 FDA；文案指引 sudo/Mole 而非「完全磁盘访问」
- 部分可读：正常出候选 + **部分覆盖** notice

**无 sudo 存在性探测的 fail-closed 替代（对齐 Mole #1188 的意图）**：Mole 在 `-e` 失败时用 `sudo -n test -e` 重探（Intego 等 root-only 树会让无特权探测误报「二进制消失」）。Vole 无 sudo，必须用以下替代闸口，**任一命中即视为「二进制可能仍存在」→ 不算 orphan**：

1. `Program` 路径的**任一祖先目录**存在但**不可读/不可进入**（`EACCES`/`EPERM`）→ 视为存在
2. 存在性探测返回权限类错误（非 `ENOENT`）→ 视为存在
3. 只有当每级祖先均可进入、且终点明确 `ENOENT` 时，才允许判「缺失」

### 4.4 Apply

- 删除只经既有 `mole_delete_verified` / path validate；**不**新增特权删除通道
- `/Library/**` 预期大量 `NeedsPrivilege`
- Human stderr / `--json` `coverage_note`：沿用 `APPLY_PERMISSION_WARN`；若 clean apply 尚未接线，**本期补上**（与 uninstall/optimize 同逻辑）
- `--json-stream`：不额外刷中文

## 5. 判定流水线

以 Mole `_plist_is_orphaned` / PHT 扫描为**唯一权威顺序**；mdfind **只**出现在 protect pattern 的 `_system_service_app_exists` 与 PHT 的 `bundle_has_installed_app` 内，**不做**全局「basename reverse-DNS 必跑 mdfind」总闸。

### 5.1 LaunchDaemon / LaunchAgent plist（顺序写死，全部通过才入选）

1. 白名单 / `should_protect` / `validate_path_for_deletion` 前置闸口 → 否则跳过  
2. filename 匹配 `com.apple.*` → 跳过；`bundle_id = filename 去 .plist`  
3. 以当前用户权限读取 `Program` 或 `ProgramArguments[0]`（顺序：先 `ProgramArguments:0` 后 `Program`，对齐 Mole）；读/解析失败或值非绝对路径 → 跳过（**禁止**把读失败当缺失）  
4. **存在性判定（§4.3 fail-closed 规则）**：二进制明确 `ENOENT` 才算缺失；权限类错误 / 祖先不可进入 → 视为存在  
5. **二进制存在时的分支（Mole #1082）**：
   - 存在且位于 `/Library/PrivilegedHelperTools/*` → 解析 helper bundle id，父 app **未安装**（`bundle_has_installed_app` 同形：安装扫描并集 + mdfind fail-closed）→ **仍算 orphan**；父 app 在 → 健康，跳过
   - 存在且不在 PHT → 健康，跳过
6. **缺失时的排除**：
   - `_is_package_managed_binary` 同形：缺失但位于 `/usr/local/{bin,sbin}`、`/opt/homebrew/{bin,sbin,opt/*/bin,opt/*/sbin}`、`/usr/{bin,sbin,libexec}`、`/bin`、`/sbin` → **不算 orphan**，跳过
   - 命中 `known_protect_patterns`（Mole 列表语义：pattern 匹配且 `_system_service_app_exists`（含 mdfind fail-closed）判 app 仍在 → 保护跳过；空 app_path 如 `homebrew.mxcl.*` → 无条件保护；pattern 匹配但 app 已消失 → 不保护，落入 orphan）
7. 以上全过 → orphan 候选

### 5.2 PrivilegedHelperTools（范围写死）

- 仅 `maxdepth 1` 的**普通文件**（对齐 Mole `-type f`；**不含** `.app`/bundle 目录）
- 扩展名黑名单（Mole #808 同形）：`.json/.cfg/.conf/.me2me_enabled/.log/.dat/.db/.xml/.yml/.yaml/.ini/.txt/.pid/.sock/.lock` → 跳过
- `bundle_id = filename 去 .plist 后缀`；`com.apple.*` → 跳过
- 先查 `known_protect_patterns`（filename 或 bundle_id 匹配均可）：app 仍在 → 保护
- 仅当 `bundle_id` 匹配 `^(com|org|net|io)\.` 且 `bundle_has_installed_app` 判**未安装** → 候选；其余一律跳过
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
- **审阅新增必测**：
  - 祖先目录存在但不可进入（chmod 000 fixture）→ 视为存在，不入选
  - 缺失但 package-managed 路径（如 `/opt/homebrew/bin/x`）→ 不入选
  - 二进制在 PHT 且存在、父 app 未安装 → **入选**（#1082 同形）；父 app 在 → 不入选
  - PHT 目录内 `.app` 目录 / 黑名单扩展名 / 非 `(com|org|net|io).` 前缀 → 不入选
  - 整规则降级 emit `NeedsPrivilege`（非 `TccDenied`）
- 不依赖 CI runner 上真实 `/Library/PrivilegedHelperTools` 有内容（Mole 已文档化 runner 空洞）；fixture 注入  
- PR 合并前：security-review（系统路径扫描）  

## 9. 验收

1. `vole clean --plan` 在 fixture/可控环境下可列出 `orphaned-system-services` 候选  
2. 不可读场景不静默「零孤儿」冒充干净：有 Skipped（`NeedsPrivilege`）/ notice / coverage 之一  
3. apply 对系统路径 skip reason 正确，人读可见权限提示；plan/coverage 文案兑现「发现优先，删除不承诺」契约  
3a. §8 审阅新增必测全部落地且绿  
4. `cargo test` 相关包绿；coverage 句不再声称 system services orphan「仍未移植」（改为可读子集已落地）  
5. 发版 **1.5.0** + release notes  

## 10. 实现后文档

- `docs/releases/v1.5.0.md`  
- findings + Formula（发版流程同 1.4.x）  
- 真 sudo 删除另开 design（本 spec 明确 defer）  
