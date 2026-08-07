# Icon Services 系统缓存清理设计（PrivilegeBackend / system.sh 第一刀）

- 日期：2026-08-07
- 状态：待实现（设计已批准）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/system.sh` → `safe_sudo_clean /Library/Caches/com.apple.iconservices.store`；用户域 `icon-services-cache` 已落地；Rosetta 1.12.0 Privilege exact 先例；`system.sh` 无 `safe_clean`（inventory 不可见），本刀显式移植单点
- 包版本意图：**`1.13.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 落地系统 Icon Services 缓存：

- **exact 路径**：`/Library/Caches/com.apple.iconservices.store`
- **PrivilegeBackend**：exact allow（`VOLE_TEST_SYSTEM_LIBRARY` → `$BASE/Caches/com.apple.iconservices.store`）
- **apply**：`sudo -n` **permanent**（无 unload）；无凭证 → `NeedsPrivilege`
- **无** arm64 门控；**无需** critical 豁免（`/Library/Caches/...` 本就不在 critical 整树）
- 与用户域 `icon-services-cache` 成对，互不改动

**采纳路径**：方案 A — exact Privilege allow + TOML `all` + apply 特权分支（绑 exact，防 rule_id 篡改绕过）。

## 2. 问题与风险

1. **`system.sh` 整模块是 sudo 面**：首刀只取可重建、单路径的 iconservices store，禁止扫 `/Library/Caches`。
2. **plan 篡改**：apply 必须 `is_icon_services_system_cache(path)`（或等价 exact 谓词）**且** allowlist，不得仅共享三树 allowlist（Rosetta Medium 教训）。
3. **废纸篓不可用**：提权写死 permanent。
4. **目录整体删除**：Mole 对该路径整目录 clean；授权后递归清空属预期。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. exact + Privilege（已选）** | 仿 Rosetta | 放行面最小 | 每路径再开刀 |
| B. 用户侧 CrashReporter 旁题 | 无 sudo | 不算 system.sh | 偏离本刀名义 |
| C. 泛 `/Library/Caches` find | 对齐 mole 宽扫 | — | **禁止** |

## 4. 产品行为

```bash
vole clean --plan
sudo -v   # 可选
vole clean --apply <plan.json>   # sudo -n → permanent；否则 NeedsPrivilege
```

- `rule_id`：`icon-services-system-cache`
- `category`：`user-devtools`（紧邻 `icon-services-cache`）
- `paths`：`["/Library/Caches/com.apple.iconservices.store"]`
- `strategy.kind`：`all`
- 规则数：**518 → 519**；**不 bump** schema
- 环境：`VOLE_TEST_SYSTEM_LIBRARY`、`VOLE_TEST_NO_AUTH`（既有）

## 5. 实现

### 5.1 Exact 谓词

`safety/critical.rs` 或旁侧（与 Rosetta 同放可维护性）：

```rust
pub const ICON_SERVICES_SYSTEM_CACHE_LIVE: &str =
    "/Library/Caches/com.apple.iconservices.store";

pub fn is_icon_services_system_cache(path: &str) -> bool
```

- normalize 后 == live，或 test mapped `$BASE/Caches/com.apple.iconservices.store`
- validate：**不必** early-ok（非 critical）；谓词供 privilege / apply 绑定

### 5.2 Privilege

`path_allowed_for_privilege`：Rosetta 判断之外，`is_icon_services_system_cache` → true。

`icon_services_system_cache_path()` / `icon_services_system_plan_candidates()`：存在则返回（**无** arm64 门控）。

常量：`ICON_SERVICES_SYSTEM_CACHE_RULE_ID = "icon-services-system-cache"`。

### 5.3 plan / apply

- plan：该 rule_id 用 mapped/live 候选（同 Rosetta remapping 模式 B），非 arm64 短路不适用
- apply：arm64 无关；`is_icon_services_system_cache` + allowlist + probe + permanent sudo；**不** unload

## 6. 覆盖说明

- coverage：标明 **Icon Services 系统缓存（sudo -n）已落地** / system.sh 首点
- 仍未移植保留：**交互提权 / 桌面特权助手**；可注 system.sh 其余路径仍未移植（可选短句）
- README：**519**

## 7. 非目标

- `/Library/Caches` 泛扫、DiagnosticReports 系统树、`/private/**`、Install macOS.app、TM
- 交互 sudo / 桌面 SMAppService
- 改用户域 `icon-services-cache`
- schema bump / 默认打 tag

## 8. 测试与安全

1. exact allow true；父目录 / 其它 Caches / 三树路径 false（相对本 rule）
2. apply：RecordingPrivilege 删 mapped；NoPrivilege → NeedsPrivilege
3. apply：`rule_id=icon-services-system-cache` + 三树路径 → **skip**（绑 exact）
4. `safety::property` 全绿
5. PR：**security-review**

## 9. 验收

1. plan/apply 可清该 store（有 sudo -n）
2. `/Library/Caches` 其它路径不可走本特权分支
3. 规则 519；版本 **1.13.0**（仓内 bump；默认不打 tag）

## 10. 实现后文档

- `docs/releases/v1.13.0.md`、findings
- 后续：DiagnosticReports 系统树 / `/private` 窄点 / 交互提权
