# Containers Stubs Orphan 设计

- 日期：2026-08-05（同日审阅修订：plan/apply 对 `orphaned-container-stubs` 豁免 `validate_path_for_deletion`/`should_protect`；carve-out 与 protect 双路径写死；identity+stub 重验；`--permanent` 不适用）
- 状态：待实现（设计已批准；审阅修订已并入）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/apps.sh` → `clean_orphaned_container_stubs` / `_remove_verified_container_stub`；B4 明确不做 Containers（现补这一刀）；system-services 的 fail-closed / OrphanDeps 模式；`data/protection.toml` `data_protected_bundles` 含 `com.macpaw.*`
- 包版本意图：能力扩展 → **`1.6.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 上交付与 Mole **同形窄刀** 的 container stub 清理：

- 只扫 `$HOME/Library/Containers`
- **硬编码 allowlist**（首发对齐 Mole：`com.macpaw.CleanMyMac*`、`*.com.macpaw.CleanMyMac*`）
- 仅删除「stub」：非 symlink 目录，且唯一子项为 `.com.apple.containermanagerd.metadata.plist`（无 `Data/`、无其它文件）
- apply 使用 **专用 carve-out**（`unlink` metadata + `rmdir`），**不**走 trash / 普通 `mole_delete`

**保护层冲突（审阅写死）**：`com.macpaw.*` 在 `data_protected_bundles` 中，`should_protect_path(Cleanup)` 会对 `~/Library/Containers/com.macpaw.*` 返回保护 → 既有 `validate_path_for_deletion` 在 **plan 入选**与 **apply 默认闸口**都会拒绝，导致规则永远空转。本期必须对该 `rule_id` 走 **显式豁免路径**（仅此规则、仅 allowlist+stub），**禁止**从 `protection.toml` 删除 `com.macpaw.*` 或放宽全局 Containers 保护。

**采纳路径**：clean custom 规则 + apply 专用删除，非独立子命令，非放宽全局 `should_protect_path`。

## 2. 问题与风险

1. **Containers 含用户数据**：误删会毁掉沙盒应用状态。必须硬 allowlist + stub 形状双重闸口；禁止泛扫整个 Containers。
2. **保护层冲突**：见 §1；Mole 用 carve-out 绕过 `safe_remove`；Vole 须同等显式、可测、可审计。
3. **TOCTOU**：plan→apply 之间目录可能长出 `Data/`；apply 必须重验 stub 形状 +（可选）目录 identity，失败则 skip。
4. **FDA**：部分环境无法列 `~/Library/Containers` → 须响亮降级，不静默空结果。
5. **TeamID 前缀**：`*.com.macpaw.CleanMyMac*` 可能**不**命中 `data_protected` 的 `com.macpaw.*`，若不走 carve-out 可能误入普通删除管线——**一律**强制 carve-out，禁止 fallback 到 `mole_delete`。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. custom + stub carve-out（推荐，已选）** | allowlist 扫描 + 专用 rm/rmdir + 规则级校验豁免 | 对齐 Mole；全局 protect 不变 | apply 双路径 |
| B. 放宽 protect / 删掉 `com.macpaw.*` | stub 进普通删除 | 实现简单 | 扩大清洁类 app 数据风险 |
| C. 仅发现硬跳过 | 对齐 system-services | 最安全 | 不兑现 Mole stub 清理价值 |

## 4. 产品行为

### 4.1 用户可见

```bash
vole clean --plan                 # 可含 container stub 候选
vole clean --apply <plan.json>    # 对 stub 走专用删除（非 trash；忽略 --permanent）
```

- `rule_id`：`orphaned-container-stubs`
- handler：`orphaned_container_stubs`
- label：`Orphaned container stub: <bundle_id>`
- **不 bump** `schema_version`
- 环境变量：本期不增；禁用走规则 `disabled = true`
- `--permanent`：**不影响**本规则（永远是 metadata unlink + rmdir，不进废纸篓也不「永久 rm -r」）

### 4.2 扫描根（写死）

| 根 | 内容 |
|---|---|
| `$HOME/Library/Containers` | 仅 allowlist glob 命中的顶层目录 |

**明确不扫**：Group Containers、Application Scripts、LaunchAgents、泛容器、无 allowlist 的 bundle。

### 4.3 Allowlist（写死，对齐 Mole 1.48.1）

| glob | 关联 app 探测路径 |
|---|---|
| `com.macpaw.CleanMyMac*` | `/Applications/CleanMyMac X.app` |
| `*.com.macpaw.CleanMyMac*` | `/Applications/CleanMyMac X.app` |

后续扩表须单独 design / 安全评审，禁止在实现中顺手扩张。

## 5. 判定流水线（plan select）

对 allowlist 展开的每个候选目录，**全部通过**才入选：

1. 是目录且 **不是** symlink  
2. 存在 `$dir/.com.apple.containermanagerd.metadata.plist` 且为普通文件  
3. `mindepth 1 maxdepth 1` 下除该 metadata 外 **无任何其它条目**（含 `Data/`）  
4. `_container_stub_app_exists` 同形：canonical app 路径 / `$HOME/Applications` / Setapp / Application Support Setapp；若 `is_reverse_dns_bundle_id(basename)` 则 mdfind fail-closed（不可用/超时 → 视为仍安装）；TeamID 前缀导致非 reverse-DNS 时 **跳过 mdfind**，仅靠文件系统 app 路径判定  
5. 白名单未命中  
6. home 经 `VOLE_TEST_HOME` / 既有测试 home 注入覆盖  

### 5.1 Plan 入选闸口豁免（审阅硬约束）

对 `rule_id == orphaned-container-stubs` 的候选：

- **不调用** `validate_path_for_deletion`（否则 `com.macpaw.*` 必被 `data_protected` 挡掉）  
- 仍须：白名单检查、`capture_plan_entry_identity`（目录级）、路径必须位于 `$HOME/Library/Containers/<name>` 一层（拒绝更深相对路径 / `..`）  
- 其它规则路径行为不变  

## 6. Apply 专用删除（carve-out）

在 `apply_plan` 中对该 `rule_id` **早分支**（对齐 `SYSTEM_SERVICES_RULE_ID` 分流风格，但是删除而非硬 skip）：

1. **禁止**调用 `mole_delete_verified` / `verify_plan_entry_for_apply`（后者内含 `validate_path_for_deletion`，会再次拒绝）  
2. 可选：`verify_plan_entry` **仅**做目录 identity TOCTOU（无 protect）  
3. **重验 stub 形状**（§5.1–3 + metadata 路径必须等于 `$dir/.com.apple.containermanagerd.metadata.plist`）；失败 → `Skipped(PathVanished)`  
4. `unlink` 仅该 metadata 文件  
5. `rmdir` 目录；若非空则失败并 skip（**禁止** `rm -r`）  
6. 记 oplog（最小）；deletion log 能接就接  
7. 成功 → `succeeded`；bytes 可为 0  

**禁止**：对该 `rule_id` fallback 到 trash / permanent delete。其它规则的 Containers 路径仍由保护层拒绝。

## 7. 权限降级

- `~/Library/Containers` 不可列 / 不可读 → 整规则 degrade：`Skipped(TccDenied)` + `PlanNotice` + 中文警告（指引 FDA；风格对齐 `ORPHAN_LIBRARY_WARN`）  
- 部分可读且 allowlist 无命中 → 正常空候选，不 degrade  

## 8. 覆盖说明

- 全局 coverage：标明 **container stubs（CleanMyMac allowlist）已落地**；仍未移植：**真 sudo 删除**、**Group Containers 泛清理**（勿再写「Containers stubs 未移植」）  
- README：Mole 对比句不再把 Containers stubs 当作「仅 Mole」全家桶要点  

## 9. 非目标

- Group Containers / TeamID 前缀泛清理  
- 扩展 allowlist 超 Mole 钉版列表  
- 从 `protection.toml` 移除 `com.macpaw.*`  
- 真 sudo  
- system-services 真删除（仍硬跳过）  
- 新 `SkipReason` 变体  
- stub 进废纸篓  

## 10. 测试与安全

- stub fixture → 入选；加 `Data/` → 不入选；symlink → 不入选  
- app 目录存在 / mdfind 命中 → 不入选  
- allowlist 外 `com.example.app` stub → 不入选  
- **`com.macpaw.*` stub 能进入 plan**（证明未死于 validate）；其它 Containers 路径仍被护  
- apply：成功 unlink+rmdir；apply 前写入额外文件 → skip 且目录保留  
- apply 不得调用 `mole_delete`（单测/钩子断言）  
- Containers 不可读 → degrade + warn  
- PR：**security-review** 必过（carve-out + 规则级 protect 豁免）  

## 11. 验收

1. plan 在 fixture 下可列出 `orphaned-container-stubs`（含受 `data_protected` 影响的 bare `com.macpaw.*` id）  
2. apply 真实删除 stub（非 trash），非 stub 不删  
3. 保护层仍拒绝其它 Containers 路径的普通删除；`com.macpaw.*` 仍在 protect 目录中  
4. coverage / README 反映已落地  
5. 发版 **1.6.0**  

## 12. 实现后文档

- `docs/releases/v1.6.0.md`、findings、Formula  
- 扩 allowlist / Group Containers 另开 design  
