# Rosetta `/Library` 更新包清理设计（PrivilegeBackend 第二刀）

- 日期：2026-08-07
- 状态：待实现（设计已批准）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/user.sh` → `clean_apple_silicon_caches` → `safe_clean /Library/Apple/usr/share/rosetta/rosetta_update_bundle`（仅 `uname -m == arm64`）；用户域 `rosetta-2-user-cache` 已落地；inventory 最后一条 `unported_all`；`is_critical_deletion_path` 整树拦 `/Library/Apple/`；PrivilegeBackend 现仅三树叶级；system-services sudo apply（1.10.0）为 Privilege 先例
- 包版本意图：**`1.12.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 落地系统域 Rosetta 更新包清理：

- **exact 路径**：`/Library/Apple/usr/share/rosetta/rosetta_update_bundle`（规范化后精确匹配，禁止 `/Library/Apple/**` 泛放）
- **arm64 门控**：运行时 `uname -m == arm64` 才 plan 入选 / apply 继续（对齐 Mole；测试可注入）
- **critical 豁免**：仿 `is_coresymbolicationd_cache`，仅该 exact（及可选 trailing `/` 归一后）
- **PrivilegeBackend**：在三树之外增加 **exact allow**（`VOLE_TEST_SYSTEM_LIBRARY` 时映射到 `$BASE/Apple/usr/share/rosetta/rosetta_update_bundle`）
- **apply**：`sudo -n` **permanent**（无 unload）；无凭证 → `NeedsPrivilege`

**采纳路径**：方案 A — exact allowlist + critical 豁免 + 特权 apply。

## 2. 问题与风险

1. **`/Library/Apple` 整树 critical**：Mole 自身 `_mole_is_critical_deletion_path` 亦拦该树，真实 `safe_remove` 会拒——库存仍保留候选。Vole 须 **单点豁免**，禁止放宽整树。
2. **提权误删面**：plan 篡改指向其它 `/Library/Apple/...` 时，allowlist + validate 均须拒绝。
3. **Intel / Rosetta 翻译进程**：`uname -m` 为 `x86_64` 时跳过（与 Mole 一致）；不得用编译期 `cfg!(target_arch)` 顶替（通用二进制 / 翻译运行会误判）。
4. **废纸篓不可用**：系统路径提权删除写死 permanent（同 1.10.0）。
5. **目录 vs 文件**：Mole 对该路径 `safe_clean`；目标可能是目录。删除递归清空属授权清理；形状仍须 exact（不可误删父级 `rosetta/`）。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. exact allow + critical 豁免（已选）** | 单路径规则 + Privilege exact + validate carve-out | 放行面最小；对齐 sysorphan 模式 | 每增一系统路径再改 allowlist |
| B. custom handler | 专用 select/recheck | 可扩 | 首刀偏重 |
| C. 放宽 `/Library/Apple/` critical | 泛豁免 | — | **禁止** |

## 4. 产品行为

```bash
# Apple Silicon 原生进程
vole clean --plan                 # 路径存在则可含 rosetta-2-cache
sudo -v                           # 可选缓存凭证
vole clean --apply <plan.json>    # sudo -n → permanent；否则 NeedsPrivilege

# Intel 或 x86_64 翻译进程：plan 不含该规则候选
```

- `rule_id`：`rosetta-2-cache`
- `category`：`user-devtools`（紧邻现有 `rosetta-2-user-cache`）
- `paths`：`["/Library/Apple/usr/share/rosetta/rosetta_update_bundle"]`
- `strategy.kind`：`all`
- 规则数：**517 → 518**；**不 bump** `schema_version`
- 环境变量：
  - 既有：`VOLE_TEST_SYSTEM_LIBRARY`、`VOLE_TEST_NO_AUTH`
  - 新增（仅测）：`VOLE_TEST_FORCE_UNAME_M=arm64|x86_64`（注入门控；未设则真实 `uname -m`）

## 5. 实现

### 5.1 Critical 豁免

`crates/vole-core/src/safety/critical.rs`：

```rust
pub fn is_rosetta_update_bundle(path: &str) -> bool
```

- `normalize_policy_path` 后等于 live exact，或（测试）`$VOLE_TEST_SYSTEM_LIBRARY/Apple/usr/share/rosetta/rosetta_update_bundle`
- `validate_path_for_deletion`：在 `is_critical_deletion_path` 检查前 early-ok（同 coresymbolicationd）

### 5.2 Privilege allowlist

`path_allowed_for_privilege`：

1. 保持现有三树叶级逻辑
2. **另或** exact Rosetta bundle（live / test remap）
3. 绝对路径、禁 `..`；**不是**前缀树下任意叶

### 5.3 arm64 门控

小模块函数（如 `privilege` 或 `ops` 旁）：

```rust
pub fn is_arm64_host() -> bool
```

- 默认：`uname -m` trim == `arm64`
- `VOLE_TEST_FORCE_UNAME_M` 覆盖
- **plan**：规则 `rosetta-2-cache` 展开前若 `!is_arm64_host()` → 零候选
- **apply**：同 rule 若 `!is_arm64_host()` → skip（`PathVanished` 或既有等价；防跨机 plan）

实现落点优先：`ops/plan.rs` 对该 `rule_id` 短路；或 `expand_rule` 包装。禁止改全局 platform 语义。

### 5.4 规则 + Apply

- TOML 追加规则
- `apply_plan`：对 `ROSETTA_CACHE_RULE_ID`（常量）走：allowlist → arm64 recheck → probe → `needs_sudo: true` permanent（**不** launchctl unload）
- plan 扫描**仍不** sudo（路径对用户不可读则自然空；本期不扩扫描权限）

## 6. 覆盖说明

- coverage：标明 **Rosetta `/Library` update bundle（arm64 + sudo -n）已落地**
- 仍未移植改为：**交互提权 / 桌面特权助手**（去掉 Rosetta `/Library`）
- README：规则 **518**

## 7. 非目标

- 交互密码 sudo / 提权进废纸篓
- 桌面 `SMAppService` PrivilegeBackend
- 其它 `/Library/Apple/**` 目标
- 用户域 `rosetta-2-user-cache` 行为变更
- plan 阶段 sudo 扩扫描
- schema bump / 默认打 tag 发版

## 8. 测试与安全

1. exact path：`is_critical` 为 true，但 `is_rosetta_update_bundle` + validate Ok
2. `/Library/Apple/usr/share/rosetta`（父目录）、`…/rosetta_update_bundle/extra`、`/Library/Apple/other` → validate 拒绝 / allowlist false
3. `path_allowed_for_privilege`：Rosetta exact true；三树回归不变
4. arm64：force arm64 → 可入选；force x86_64 → 空；apply 在 x86_64 上跳过
5. RecordingPrivilege：probe true → remove 被调；NoPrivilege → NeedsPrivilege
6. `safety::property` 全绿
7. PR：**security-review**（放行面仅 exact bundle + 既有三树）

## 9. 验收

1. Apple Silicon 上 plan/apply 可清该 bundle（有 `sudo -n` 时）
2. 非 arm64 零候选
3. `/Library/Apple` 其它路径仍不可删
4. coverage / README 518；版本 **1.12.0**（仓内 bump；**默认不打 tag**）

## 10. 实现后文档

- `docs/releases/v1.12.0.md`、findings
- 后续刀：交互提权 / 桌面 PrivilegeBackend / `system.sh` 批
