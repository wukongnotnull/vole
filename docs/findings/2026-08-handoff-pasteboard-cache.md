# Findings：Handoff Pasteboard Cache（v1.8.0）

## 背景

Mole `clean_handoff_pasteboard_cache` 清理 `shared-pasteboard` 叶节点（`-mmin +60`），避免重剪贴板同步堆积到数百 GB（#1178）。初稿假定 `group.com.apple.*` 必被步骤 3 `data_protected` 拦住，需形状豁免 + `skip_protection`。

## 探针结论

对 `should_protect_path(..., Cleanup)` 实测：

| 形状 | prot | 说明 |
|---|---|---|
| `…/useractivityd/shared-pasteboard/item1` | **false** | raw bid=`group.com.apple.…` → `should_protect_data` false（不剥 `group.`） |
| `…/useractivityd/other` | **false** | 故 handler **只**扫 `shared-pasteboard` |

剥 `group.` 后再查则为 true——未来若保护层归一化剥前缀，本路径可能重新被拦，需另开 design。

## 决策

1. **零保护层 / 零豁免 / 零 skip_protection**
2. apply 仍做**政策重验**（单层根 + mtime>60），防止篡改 plan 挂到同容器其它路径或飞行中条目
3. 版本 **1.8.0**
