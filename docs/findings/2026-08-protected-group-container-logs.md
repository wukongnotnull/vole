# Protected Group Container Logs（1.9.0）

补齐 `group-container-caches`（1.7.0）已知覆盖缺口：保护层形状豁免使受保护容器 Logs 与 bundle 命名日志叶可清理。

## 闸口目标矩阵

路径均在 `~/Library/Group Containers/`，`ProtectionMode::Cleanup`：

| 形状 | 1.7.0 | 1.9.0 |
|---|---|---|
| `com.macpaw.CleanMyMac/Logs/x` | 拦（步骤 3） | 放行 |
| `com.macpaw.CleanMyMac/Library/Logs/x` | 拦（步骤 3） | 放行 |
| `com.macpaw.CleanMyMac/Caches/x` 等 | 拦 | 仍拦 |
| `group.com.docker…/Logs/com.docker.helper.log` | 拦（步骤 7） | 放行 |
| `group.com.apple.notes/Logs/x` | 拦（步骤 1） | 仍拦 |
| OrbStack runtime | 拦 | 仍拦 |
| `…/<id>/Other/Logs/x` | 拦 | 仍拦 |

## 实现落点

- `crates/vole-core/src/protection/path.rs`
  - `is_group_container_logs_path`：仅相对容器根 `Logs/<leaf>` 或 `Library/Logs/<leaf>`
  - 步骤 3：命中时 `container_cache = true`
  - `is_explicit_clean_cache_path`：命中时返回 true（顶层 `/Logs/` 纵深）
- 不动：handler / TOML / apply rule_id 旁路 / `protection.toml`

## 验收

- protection 单测 + plan 集成（macpaw Logs 入选；Caches 不入选；bundle 命名入选）
- coverage 去掉「受保护容器的组容器缓存」未移植措辞
- 发版 1.9.0；规则数 516
