# Phase 1 TCC 实测（deferred + ad-hoc）

日期：2026-07-29

## 完整矩阵（设计 4.1）— deferred

| 签名身份 | 启动方式 |
|---|---|
| 未签名（`cargo build`） | 终端 |
| ad-hoc 签名 | 终端、Raycast |
| Developer ID 签名 | 终端、Raycast、从 app bundle 内 spawn |

**Deferred 原因**：尚未购买 Apple Developer ID（$99/yr）。Developer ID 列无法在本机实测；Raycast / app spawn 列在 ad-hoc 身份下也不具代表性。

**触发补测条件**：购买 Developer ID 后第一个 Sprint 跑完整矩阵，并把结论写回 `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` 4.1。

## Phase 0.5 已测项

- ad-hoc 签名下读 `~/Library/Containers`，退出码 0，未观测 TCC 弹窗。
- 见 `docs/findings/2026-07-spike-platform.md`。

不足以代表 Full Disk Access 或重编译 cdhash 变化场景。

## Phase 1 ad-hoc 子集（本机 2026-07-29）

由 `scripts/tcc-adhoc-matrix.sh` 执行：

```
=== build vole-cli ===
=== probe: read Containers ===
containers: 0
=== probe: read Caches ===
caches: 0
=== probe: recompile cdhash ===
no-cdhash-line
```

补充：`codesign -dv target/debug/vole` 显示 `Signature=adhoc`，`CodeDirectory` 嵌入；`grep CDHash` 无单独行（脚本回退输出 `no-cdhash-line`）。

### 解读

1. **Containers / Caches 可读（exit 0）**：与 Phase 0.5 一致；本机终端会话下未触发新弹窗。不能推断 FDA 已授予或无需 FDA。
2. **重编译后 cdhash**：未从 `codesign -dv` 提取到独立 CDHash 行；重编译是否触发重新授权 **仍待 Developer ID / 完整矩阵** 验证。
3. **Raycast / app spawn**：未测。

## Phase 2 前行动项

- 购买 Developer ID 后补跑完整矩阵。
- `clean` 实现前仍需 `check_tcc_permissions()` 等价预热逻辑；授权身份与分发形态（Formula vs Cask vs app 内嵌）一并规划。
