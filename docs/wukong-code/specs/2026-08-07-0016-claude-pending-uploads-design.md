# Claude Pending Uploads 设计（Mole `pending-uploads` 同形）

- 日期：2026-08-07
- 状态：待实现（设计已批准）
- 依据：Mole `third_party/mole-1.48.1/lib/clean/dev.sh` → `safe_clean …/Claude/pending-uploads/*`；v1 刻意跳过（`Claude` data_protected / Application Support 非 explicit cache）；group-container Logs 形状豁免先例；coverage「claude pending-uploads」未移植
- 包版本意图：**`1.11.0`**（SemVer MINOR）

## 1. 结论

在 **`vole clean` plan→apply** 落地 Claude Desktop **pending-uploads** 叶清理：

- 根写死：`$HOME/Library/Application Support/Claude/pending-uploads`
- 仅 **单层叶**；保护层形状豁免使 Cleanup 可通过 `should_protect_path`
- apply 走普通 `mole_delete_verified`（废纸篓 / `--permanent`）
- **不改** `protection.toml` 的 `Claude` keyword；**不**扩 PrivilegeBackend；**无** apply 旁路

**采纳路径**：方案 A — `is_claude_pending_uploads_path` + TOML `all` 规则。

## 2. 问题与风险

1. **Claude 整树受保护**：`protection.toml` 含 `Claude`，Application Support 路径易被步骤 6/7 拦下。必须形状收窄到 `pending-uploads/<leaf>`，禁止放行 Local Storage / Cookies / 其它 Support 数据。
2. **与其它 Claude Electron cache 规则分工**：Cache / Code Cache / GPUCache 等已由 `is_explicit_clean_cache_path` 的 `/Cache/` 等段或独立规则覆盖；**仅** pending-uploads 缺口。
3. **误删上传中文件**：Mole 亦清目录下全部叶、无 mtime 门槛；本期对齐 Mole（不另加年龄阈）。用户可用规则 `disabled` 关掉。
4. **Rosetta 非本刀**。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 保护层形状豁免 + TOML（已选）** | `is_claude_pending_uploads_path` → explicit clean | 与 Cache 段 / Logs 豁免同形；无旁路 | 改全局 protect |
| B. TOML + apply skip_protection | 局部 | — | 扩大 apply 旁路面 |
| C. 仅 custom 后实测再豁免 | 渐进 | — | 两趟；已知必拦 |

## 4. 产品行为

```bash
vole clean --plan                 # 可含 Claude pending-uploads 叶
vole clean --apply <plan.json>    # 普通删除
```

- `rule_id`：`claude-pending-uploads`
- `category`：与邻近 Claude 规则一致（见实现：优先 `data/rules/user-devtools.toml` 或 `app-caches.toml` 中已有 Claude Electron 块）
- `paths`：`["~/Library/Application Support/Claude/pending-uploads/*"]`
- **不加** `not_running`（Mole 此条无进程 guard）
- 规则数：**516 → 517**；**不 bump** `schema_version`
- 环境变量：不增；禁用走 `disabled = true`

## 5. 实现

### 5.1 保护层

`crates/vole-core/src/protection/path.rs`：

```rust
fn is_claude_pending_uploads_path(path: &str) -> bool
```

**写死**：

1. 含 `/Library/Application Support/Claude/pending-uploads/`
2. 该前缀之后恰好**一层**叶名（非空、不含 `/`）
3. 拒绝：目录本身、更深路径、`…/Claude/other/…`

接入 `is_explicit_clean_cache_path` 早期返回 true（步骤 6/7 不再因 `Claude` keyword 拦截）。

**不改**：`protection.toml`；步骤 1–5 其它关键字；Uninstall 模式特殊逻辑（本豁免经 explicit 路径，与现有 Electron Cache 一致）。

### 5.2 规则

追加 TOML `all` 规则（handler 默认路径展开即可）。**禁止**写入 `zzz-orphaned.toml`。

### 5.3 Apply

无特殊分支。

## 6. 覆盖说明

- coverage：标明 **Claude pending-uploads 已落地**
- 仍未移植改为：Rosetta `/Library`、交互提权 / 桌面特权助手（去掉 claude pending-uploads）
- README：规则 **517**；Mole 对比句可去掉该缺口

## 7. 非目标

- Rosetta `/Library/Apple/.../rosetta_update_bundle`
- 清整个 `Application Support/Claude`
- 新 SkipReason / schema bump / sudo
- apply carve-out / `skip_protection`

## 8. 测试与安全

1. `…/Claude/pending-uploads/file.x` → Cleanup **不**保护
2. `…/Claude/pending-uploads/`（无叶）→ **仍**保护或未匹配豁免
3. `…/Claude/Local Storage/…`、`…/Claude/Cache/…` 行为不因本刀误伤（Cache 本可清则保持；Local Storage 仍拦）
4. plan fixture：叶入选并可 apply
5. `safety::property` 全绿
6. PR：**security-review**（放行面仅 pending-uploads 叶）

## 9. 验收

1. plan/apply 可清理 pending-uploads 叶
2. 其它 Claude Support 敏感路径仍受保护
3. coverage / README；规则 517；版本意图 **1.11.0**（仓内 bump；**默认不打 tag 发版**）

## 10. 实现后文档

- `docs/releases/v1.11.0.md`、findings  
- Rosetta `/Library` 另开 design
