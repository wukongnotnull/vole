# Spike C：平台行为核实

**日期**：2026-07-29  
**环境**：本机 macOS（arm64），日常开发账户

## 1. SQLite WAL（Chrome History）

路径：`~/Library/Application Support/Google/Chrome/Default/History`

| 观测 | 结果 |
|---|---|
| `-wal` 文件 | **不存在**；仅有 `History` + `History-journal` |
| `immutable=1` | 成功，`count(*)=1764` |
| `mode=ro`（Chrome 运行中） | **失败**：`database is locked (5)` |

**结论**：本机 Chrome 未使用 WAL 模式（或 WAL 已 checkpoint）。`immutable=1` 在 DB 未锁时可读；运行中 Chrome 的普通只读打开会被锁拒绝。设计文档 5.2 的三路径策略仍成立，但需按应用实际 journal 模式分支处理。

## 2. 废纸篓口径

50 MB 文件经 Finder `delete` 移入废纸篓：

| 观测 | 结果 |
|---|---|
| `df` 可用空间（before） | 280713080 KB |
| `df` 可用空间（after） | 280712984 KB（差 96 KB，非 50 MB） |
| 列出 `~/.Trash` | **Operation not permitted**（TCC） |

**结论**：移入废纸篓不立即释放磁盘空间，与设计 5.7 一致。`trashed_bytes` 应计废纸篓内文件体积，不能看 `df` 差值。列出废纸篓需 Full Disk Access 或用户授权。

## 3. TCC 与签名

| 观测 | 结果 |
|---|---|
| 未签名 / ad-hoc 读 `~/Library/Containers` | 退出码均为 **0**（本账户无 TCC 弹窗） |
| 重编译后 cdhash | ad-hoc 重签后 `codesign -dv` 未稳定输出 CDHash 行（需 Phase 1 用 Developer ID 实测） |

**结论**：读 Containers 在本环境未触发 TCC，不能代表未签名二进制在 Full Disk Access 场景下的行为。开发期反复弹窗风险仍待 Phase 1 用 Developer ID 证书验证。

## 4. Homebrew 签名政策

来源：[Homebrew brew issue #20755](https://github.com/homebrew/brew/issues/20755)、[Workbrew 5.0 说明](https://workbrew.com/blog/homebrew-5-0-0)

| 类型 | 签名要求 |
|---|---|
| **Cask**（预编译二进制） | 2026-09-01 起官方 Tap 要求 codesign + notarize；`--no-quarantine` 将移除 |
| **Formula**（源码/本地 bottle） | 不受 Cask 审计约束；用户本地构建无公证要求 |

**结论**：若 Vole 通过 **Formula** 分发（用户本地 `cargo install` 或 brew 从源码构建），无强制公证。若通过 **Cask** 分发预编译 `vole` 二进制，需 Apple Developer ID + 公证。设计 5.5 待核实项可关闭为：**Cask 路径需 $99/yr + 公证流水线；Formula 路径无此要求。**
