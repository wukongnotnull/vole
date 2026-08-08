# optimize `spotlight_index_optimize` 设计（闸控轨 G3）

- 日期：2026-08-08 18:36
- 状态：已批准（用户明确「批准执行轨 G3」；本会话 design 落盘后直接实现）
- 依据：[`2026-08-08-1727-mole-parity-roadmap-design.md`](2026-08-08-1727-mole-parity-roadmap-design.md) §3.3 P3；计划 [`../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md`](../plans/2026-08-08-1739-mole-parity-closeout-gated-rails.md) Task G3；Mole `opt_spotlight_index_optimize`（`tasks.sh` ≈754）；G2 design；W2b③ 特权模式
- 包版本意图：**1.44.0**（MINOR；相对 `1.43.0`）
- **不 bump** `schema_version`

## 1. 结论

将 catalog **`spotlight_index_optimize.in_m3: false` → `true`**，主路径 20 → **21**：

| 阶段 | 行为 |
|---|---|
| **plan** | 始终 1 条 action sentinel（不探测、不 sudo；对齐 W2b③） |
| **apply** | 智能门控：禁用 / 非 AC / 探针未慢 → **noop Ok**；两探针皆慢 → `PrivilegeBackend::rebuild_spotlight_index`（`sudo -n mdutil -E /`） |
| **失败** | 需重建但无凭证 → `NeedsPrivilege`；命令失败 → `Failed`；`VOLE_TEST_NO_AUTH` 下永不真 sudo |

## 2. 与 `system_maintenance` Spotlight 去重

| | `system_maintenance` | `spotlight_index_optimize`（本轨） |
|---|---|---|
| Spotlight | apply 末尾 **只读** `mdutil -s /`（状态检查，无副作用） | 条件满足时 **`mdutil -E /` 重建索引** |
| DNS | `flush_dns_cache` | 不碰 |
| 职责 | 维护套件里的轻量检查 | 慢搜索时的破坏性重建 |

**禁止**在本轨改 `system_maintenance` 去跑 `-E`；两任务可同 plan 共存，职责不重叠。

## 3. 何时 `mdutil -E`（副作用）

仅当 **全部**成立：

1. `mdutil -s /` 显示 Indexing **enabled**（且非「Indexing and searching disabled」）
2. **AC 供电**（电池上不重建；对齐 Mole）
3. 两次 `mdfind "kMDItemFSName == 'Applications'"` 探针耗时均 **>** 阈值（默认 3s；`VOLE_OPTIMIZE_SPOTLIGHT_SLOW_SEC` / 测试注入可覆盖）
4. 特权可用（`sudo -n`）

副作用：索引重建可耗时 1–2 小时；后台继续。失败或跳过须响亮（`NeedsPrivilege` / Failed），不静默成功。

## 4. 采纳路径

| 点 | 决策 |
|---|---|
| catalog | `in_m3: true`；主路径 **21**；余 `shared_file_list_repair` / `disk_verify` 仍 false |
| 特权 | 新方法 `PrivilegeBackend::rebuild_spotlight_index` → `sudo -n mdutil -E /`；`NoPrivilege`/`RecordingPrivilege`/`SudoNoninteractive` 全补 |
| 模块 | `optimize/tasks/spotlight_index.rs`：状态/AC/慢探针纯函数 + env 注入 |
| plan | `~/.vole-optimize-action/spotlight_index_optimize`；label `Spotlight Optimization` |
| 测试注入 | `VOLE_TEST_SPOTLIGHT_STATUS=enabled\|disabled\|other`；`VOLE_TEST_AC_POWER=0\|1`；`VOLE_TEST_SPOTLIGHT_SLOW=0\|1` |
| `VOLE_TEST_NO_AUTH` | Live backend 返回 Unavailable；需重建路径 → NeedsPrivilege |
| coverage / 版本 | 补一句；**1.44.0** + release / README / Formula |

## 5. Mole 对照

| Mole | Vole |
|---|---|
| `mdutil -s` / AC / 双探针 | 同语义；env 可强制 |
| `sudo mdutil -E /` | `sudo -n` via PrivilegeBackend |
| dry-run 假装 started | plan sentinel；apply 才重建 |
| 无凭证 skip 警告 | NeedsPrivilege |

## 6. 测试策略

- Catalog：含 id，`main.len()==21`，仍排除 shared_file_list / disk_verify
- apply：disabled/battery/fast → Ok 且 RecordingPrivilege 调用计数 0；slow + allowing → 调用 1；slow + denying → NeedsPrivilege
- 禁止：单测真 `sudo mdutil -E`

## 7. 非目标

- G4/G5/D1；改 `system_maintenance` 语义；交互式 sudo 新体系
