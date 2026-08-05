# Containers Stubs Orphan 设计

- 日期：2026-08-05
- 状态：待实现（设计已批准）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/apps.sh` → `clean_orphaned_container_stubs` / `_remove_verified_container_stub`；B4 明确不做 Containers（现补这一刀）；system-services 的 fail-closed / OrphanDeps 模式
- 包版本意图：能力扩展 → **`1.6.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 上交付与 Mole **同形窄刀** 的 container stub 清理：

- 只扫 `$HOME/Library/Containers`
- **硬编码 allowlist**（首发对齐 Mole：`com.macpaw.CleanMyMac*`、`*.com.macpaw.CleanMyMac*`）
- 仅删除「stub」：非 symlink 目录，且唯一子项为 `.com.apple.containermanagerd.metadata.plist`（无 `Data/`、无其它文件）
- apply 使用 **专用 carve-out**（`unlink` metadata + `rmdir`），**不**走 trash / 普通 `mole_delete`（后者被 Containers 保护一刀切拒绝）

**采纳路径**：clean custom 规则 + apply 专用删除，非独立子命令，非放宽全局 `should_protect_path`。

## 2. 问题与风险

1. **Containers 含用户数据**：误删会毁掉沙盒应用状态。必须硬 allowlist + stub 形状双重闸口；禁止泛扫整个 Containers。
2. **保护层冲突**：`should_protect_path` 罩住 `~/Library/Containers`，普通删除管线会拒绝。Mole 已用专用 carve-out；Vole 必须同样显式、可测、可审计，不能「悄悄放宽 protect」。
3. **TOCTOU**：plan→apply 之间目录可能长出 `Data/`；apply 必须重验 stub 形状，失败则 skip（`PathVanished` 或等价）。
4. **FDA**：部分环境无法列 `~/Library/Containers` → 须响亮降级，不静默空结果。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. custom + stub carve-out（推荐，已选）** | allowlist 扫描 + 专用 rm/rmdir | 对齐 Mole；保护层不变 | apply 双路径 |
| B. 放宽 protect 走 trash | stub 进普通删除 | 实现简单 | 扩大 Containers 攻击面 |
| C. 仅发现硬跳过 | 对齐 system-services | 最安全 | 不兑现 Mole stub 清理价值 |

## 4. 产品行为

### 4.1 用户可见

```bash
vole clean --plan                 # 可含 container stub 候选
vole clean --apply <plan.json>    # 对 stub 走专用删除（非 trash）
```

- `rule_id`：`orphaned-container-stubs`
- handler：`orphaned_container_stubs`
- label：`Orphaned container stub: <bundle_id>`
- **不 bump** `schema_version`
- 环境变量：本期不增；禁用走规则 `disabled = true`

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

## 5. 判定流水线

对 allowlist 展开的每个候选目录，**全部通过**才入选：

1. 是目录且 **不是** symlink  
2. 存在 `$dir/.com.apple.containermanagerd.metadata.plist` 且为普通文件  
3. `mindepth 1 maxdepth 1` 下除该 metadata 外 **无任何其它条目**（含 `Data/`）  
4. `_container_stub_app_exists` 同形：canonical app 路径 / `$HOME/Applications` / Setapp / Application Support Setapp；reverse-DNS 则 mdfind fail-closed（不可用/超时 → 视为仍安装）  
5. 白名单未命中  
6. （plan 期）可经注入的 `VOLE_TEST_HOME` 根覆盖 home

## 6. Apply 专用删除（carve-out）

新增窄函数（名实现时定），语义对齐 `_remove_verified_container_stub`：

1. **重验** §5 形状（含 symlink / 额外条目 / metadata 路径必须等于 `$dir/.com.apple.containermanagerd.metadata.plist`）  
2. `unlink` 仅该 metadata 文件  
3. `rmdir` 目录；若非空则失败并 skip（**禁止** `rm -r`）  
4. 记 oplog / deletion log（若既有接口允许；最小：oplog）  
5. 成功计入 `succeeded`；不增加有意义的 trash bytes（可 0）

**禁止**：对该 `rule_id` 调用 `mole_delete_verified` / trash。

其它规则的 Containers 路径仍由保护层拒绝（行为不变）。

## 7. 权限降级

- `~/Library/Containers` 不可列 / 不可读 → 整规则 degrade：`Skipped(TccDenied)` + `PlanNotice` + 中文警告（指引 FDA；风格对齐 `ORPHAN_LIBRARY_WARN`）  
- 部分可读且 allowlist 无命中 → 正常空候选，不 degrade

## 8. 覆盖说明

- 全局 coverage：标明 **container stubs（CleanMyMac allowlist）已落地**；仍未移植：**真 sudo 删除**、**Group Containers 泛清理**（若仍列 Containers stubs 须改为已落地）  
- README：Mole 对比句不再把 Containers stubs 当作「仅 Mole」全家桶要点（可改为真 sudo / purge / installer）

## 9. 非目标

- Group Containers / TeamID 前缀清理  
- 扩展 allowlist 超 Mole 钉版列表  
- 真 sudo  
- system-services 真删除（仍硬跳过）  
- 新 `SkipReason` 变体  

## 10. 测试与安全

- stub fixture → 入选；加 `Data/` → 不入选；symlink → 不入选  
- app 目录存在 / mdfind 命中 → 不入选  
- allowlist 外 `com.example.app` stub → 不入选  
- apply：成功 unlink+rmdir；apply 前写入额外文件 → skip 且目录保留  
- Containers 不可读 → degrade + warn  
- PR：**security-review** 必过（carve-out 高敏）

## 11. 验收

1. plan 在 fixture 下可列出 `orphaned-container-stubs`  
2. apply 真实删除 stub（非 trash），非 stub 不删  
3. 保护层仍拒绝其它 Containers 路径的普通删除  
4. coverage / README 反映已落地  
5. 发版 **1.6.0**

## 12. 实现后文档

- `docs/releases/v1.6.0.md`、findings、Formula  
- 扩 allowlist / Group Containers 另开 design  
