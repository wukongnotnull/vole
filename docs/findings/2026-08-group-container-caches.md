# Findings：Group Container Caches（v1.7.0）

## 背景

Mole `clean_group_container_caches` 清的是 Group Containers 内 Logs /（条件）Caches·tmp 的**叶节点**，不是整容器。初稿 design 假设「步骤 3 `data_protected` 挡住了绝大多数 Logs，必须扩展保护层才能交付」。

## 探针结论（设计 §6）

对 `should_protect_path(..., Cleanup)` 实测（路径均在 `~/Library/Group Containers/`）：

| 形状 | 保护？ | 拦截点 |
|---|---|---|
| `group.com.docker.docker/Library/Caches/foo` | 否 | —（`group.` 前缀使步骤 3 不命中） |
| `group.com.docker.docker/Logs/plain.log` | 否 | — |
| `group.com.docker.docker/Logs/com.docker.helper.log` | 是 | **步骤 7 叶子文件名 bundle guard** |
| `com.macpaw.CleanMyMac/Logs/x` | 是 | 步骤 3 `data_protected` |
| `TEAMID.com.tencent.*/...` | 否（仅剥 `group.` 时） | TeamID 绕过 → 本期在 handler 收严 |

## 决策

1. **本期零保护层改动**：收益主体（`group.*` 容器 Logs/Caches/tmp）今天即可走普通删除；放行面不扩大。
2. **已知覆盖缺口可接受**：受保护 id 的 Logs、bundle 命名日志文件 → plan skip；完整对齐留 1.8.0 + security-review。
3. **TeamID 收严**：四种归一化形态任一 `should_protect_data` 命中 → 只提 Logs。
4. **规模上限替代 Mole partial size**：Vole `PlanEntry.size` 全量 `path_size`，超限整树不提候选。

## 残余风险

- 非 protected 组容器内若误放业务文件到 Logs/Caches/tmp，会被清（与 Mole 同）。
- Safari 探测依赖同 id Containers 目录命名启发式，未知扩展可能漏跳过或过跳过。
- 上限触发后用户需改用 Mole / 缩小范围；无自动分批。
