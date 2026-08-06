# System Services Orphan 真删（CLI `sudo -n`）设计

- 日期：2026-08-06
- 状态：待实现（设计已批准）
- 依据：`2026-08-05-2150-system-services-orphan-design.md`（明确 defer 真 sudo）；Mole `apps.sh` `clean_orphaned_system_services`（`sudo -n` unload + 删除）；`mole_delete` 现网 `needs_sudo` → `sudo-not-implemented`；`apply_plan` 对 `SYSTEM_SERVICES_RULE_ID` 硬 skip；v2 产品目标「CLI 与桌面提权双轨」
- 包版本意图：能力扩展 → **`1.10.0`**（SemVer MINOR）

## 1. 结论

在 **CLI** 上为已落地的 `orphaned-system-services` 接通 **apply 真删**：

- plan **仍不调用 sudo**（继续「可读子集」扫描，与 1.5.0 契约一致）
- apply：去掉 rule_id 硬 skip；在 `sudo -n` 非交互凭证可用时 **permanent** 删除；否则 `NeedsPrivilege` + 中文提示（可先 `sudo -v` 缓存）
- 抽出 `PrivilegeBackend` trait：实现体为本期 `SudoNoninteractive`；桌面 `SMAppService` **只预留接缝，不落地**

**采纳路径**：方案 A — PrivilegeProbe + PrivilegedDelete；非整批移植 `system.sh`；非交互弹密码。

## 2. 问题与风险

1. **误删 LaunchDaemon / PHT**：计划篡改 + 提权删除面比用户域大一档。必须：绝对路径、禁 `..`、**前缀 allowlist**（仅三树）、apply 前形状/orphan 政策重验、identity TOCTOU。
2. **`sudo -n` 无凭证**：不得阻塞等密码；必须 skip + 响亮说明（对齐 Mole）。
3. **废纸篓不可用**：root 拥有路径无法稳进用户 Trash → 提权删除写死 **permanent**（即使用户默认 trash）。
4. **测试/CI**：无凭证环境不得误调真 sudo；`VOLE_TEST_NO_AUTH=1` / 测试 Backend 强制 blocked。
5. **扫描面不变**：本期**不**用 sudo 扩宽 plan 发现（仍可读子集）；避免 plan 阶段弹权限、也避免与「发现优先」文案打架后立刻承诺「列全」。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. PrivilegeBackend + sysorphan apply（已选）** | trait + `sudo -n`；去掉硬 skip | 接缝清晰；可接桌面；最小规则面 | trait 初建成本 |
| B. apply 内联 `Command::new("sudo")` | 无抽象 | 快 | 桌面双轨重写；难测 |
| C. 交互 `sudo`（可缓存） | 弹密码 | 覆盖高 | 偏离 Mole `-n`；脚本/CI 差 |

第一刀绑定 **system-services apply**（存量最大）；Rosetta / claude pending-uploads / `system.sh` 批移植另开 design。

## 4. 产品行为

### 4.1 用户可见

```bash
vole clean --plan                         # 不变：可读子集候选
sudo -v                                   # 可选：缓存凭证
vole clean --apply <plan.json>            # 有 -n 凭证 → permanent 真删；否则 NeedsPrivilege
vole clean --apply <plan.json> --permanent # 对特权条目与上同形（已是 permanent）
```

- `rule_id` 仍为现有 system-services 规则（**无新规则**）；规则数 **516 不变**
- **不 bump** `schema_version`
- human apply：特权条目成功时注明「已提权永久删除」；失败 skip 提示「需要非交互 sudo（可先执行 sudo -v）」

### 4.2 与 1.5.0 契约修订

| 项 | 1.5.0 | 1.10.0 |
|---|---|---|
| plan | 可读子集 | 不变 |
| apply | 硬 skip `NeedsPrivilege` | 条件真删（`sudo -n`） |
| 产品话术 | 发现优先、删除不承诺 | 发现仍可读子集；**有凭证则可删** |

## 5. 架构

### 5.1 `vole-core::privilege`

```rust
pub trait PrivilegeBackend: Send + Sync {
    fn probe_noninteractive(&self) -> bool;
    fn remove_permanent(&self, path: &Path) -> Result<(), PrivilegeError>;
    /// 尽力而为；失败不视为整体失败（对齐 Mole unload || true）
    fn launchctl_unload(&self, plist: &Path) -> Result<(), PrivilegeError>;
}

pub struct SudoNoninteractive;
pub struct NoPrivilege; // probe=false；测试默认
```

**`SudoNoninteractive` 命令约束（写死）**：

- 一律 `Command` 参数分列：`sudo`, `-n`, `true` / `/bin/rm`, `-rf`, `--`, `<abs-path>` / `launchctl`, `unload`, `<plist>`
- **禁止** `sh -c` / 字符串拼接路径
- `remove_permanent` 调用前由调用方完成 allowlist + 政策重验；Backend 内再断言：`path.is_absolute()`、规范化后仍在允许前缀内，否则 `PrivilegeError::Refused`

**允许前缀（写死）**：

- `/Library/LaunchDaemons/`
- `/Library/LaunchAgents/`
- `/Library/PrivilegedHelperTools/`

测试 fixture 可用 `PrivilegeBackend` mock，或 `NoPrivilege`。

### 5.2 `mole_delete`

今日 `needs_sudo == true` 分支返回 `sudo-not-implemented`。改为：

1. `test_no_auth()` → 保持 `SudoBlockedTestMode`
2. `backend.probe_noninteractive()` 为 false → 映射为 apply 层 `NeedsPrivilege`（或专用 `MoleDeleteError::SudoUnavailable`）
3. true → `backend.remove_permanent`（本刀特权路径不走用户 Trash API）

`MoleDeleteOptions` 增加或注入 `&dyn PrivilegeBackend`（由 apply 传入；默认 `SudoNoninteractive`；测试 `NoPrivilege`）。

### 5.3 `apply_plan`（system-services）

1. **删除** `SYSTEM_SERVICES_RULE_ID` 硬 skip 块
2. 对该 rule 专用流水线（在普通 `mole_delete_verified` 之前或替换其 needs_sudo 路径）：
   - `recheck_system_service_entry`（复用/抽出 select 闸口：形状、非 Apple、非 package-managed、orphan 判定依赖与 plan 时一致；失败 → `PathVanished`）
   - identity TOCTOU
   - `probe`；失败 → `NeedsPrivilege` + 既有权限响亮 hint（文案改为可含「sudo -v」）
   - LaunchDaemon/Agent：`launchctl_unload` 尽力而为
   - `needs_sudo=true` + permanent 删除

普通用户域规则：**零变化**（`needs_sudo=false`）。

### 5.4 明确不改

- plan 扫描不用 sudo 扩面
- `protection.toml` / `should_protect_path`（系统路径本就不靠用户域 protect）
- uninstall / optimize 的 sudo 长尾
- Rosetta、claude pending-uploads
- 桌面 helper 二进制

## 6. 覆盖说明

- `coverage_note`：标明 **system services orphan（可读子集 plan + sudo -n apply 真删）已落地**
- 仍未移植改为：**Rosetta `/Library`、claude pending-uploads、交互式提权、无凭证时的完整 /Library 扫描、桌面特权助手**
- 去掉「真 sudo 删除」整项未移植措辞（或改为「除 system-services 外的真 sudo 长尾」——若保留「真 sudo」字样，必须写明「仅 system-services apply」以免过度宣称）
- **推荐定稿句**：`仍未移植：其它 sudo/系统路径（如 Rosetta \`/Library\`、claude pending-uploads）、交互提权 / 桌面特权助手。`
- README：Mole 对比句可注明 system-services 可在已缓存 sudo 下删除

## 7. 非目标

- 交互密码 / `sudo` 无 `-n`
- 提权路径进废纸篓
- SMAppService / vole-macos helper
- plan 阶段 `sudo -n find` 扩扫描
- Rosetta、claude pending-uploads、`system.sh` 批规则
- uninstall/optimize sudo 任务
- 新 `SkipReason`（沿用 `NeedsPrivilege`）
- schema bump / 新 clean 规则 id

## 8. 测试与安全

1. Mock Backend：probe true → remove 被调用且路径在 allowlist
2. probe false → apply skip `NeedsPrivilege`，**零** remove 调用
3. 路径含 `..` / 非三树前缀 → Refused，不调用 sudo
4. `VOLE_TEST_NO_AUTH=1` → 永不执行真 `sudo`
5. 非 system-services 条目行为与 1.9.0 一致
6. 可选 `#[ignore]` 集成测：本机已 `sudo -n true` 时对 fixture 只读树（慎用；默认关）
7. PR：**security-review 必过**（命令注入、allowlist、硬 skip 移除后的篡改 plan 面）

## 9. 验收

1. 有非交互 sudo 凭证时，可读 orphan 候选可被 permanent 删除
2. 无凭证时响亮 `NeedsPrivilege`，行为安全
3. plan 仍无 sudo；规则数 516；发版 **1.10.0**
4. coverage / README 反映「system-services 真删已落地」，不夸大其它 sudo 面

## 10. 实现后文档

- `docs/releases/v1.10.0.md`、findings、Formula
- 后续刀：Rosetta / claude pending-uploads / plan 扩扫描（可选）/ 桌面 PrivilegeBackend
