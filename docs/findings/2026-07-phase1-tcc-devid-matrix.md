# Phase 1 TCC 完整矩阵（Developer ID）

**日期**：2026-07-30  
**机器**：开发机 macOS（darwin 25）  
**Developer ID**：`Developer ID Application: Kong Wu (WCYC8XY4V2)` / Team `WCYC8XY4V2`  
**脚本**：`bash scripts/tcc-devid-matrix.sh`

## 方法

在 `$HOME/Library/{Caches,Containers,Application Support,Logs}` 与 `~/.cache` 下各建小型探针目录，用对应签名的 `vole analyze --json <probe>` 测可读性（避免整树扫描）。  
父进程均为**终端**；app-bundle 单元格是「从 `.app/Contents/MacOS/vole` 直接 exec」，**不是** `open -a` 图形启动。

## 结果摘要

| 签名身份 | 启动 | Caches | Containers | App Support | Logs | ~/.cache |
|---|---|---|---|---|---|---|
| 未签名（去签名） | 终端 | **137 SIGKILL** | 137 | 137 | 137 | 137 |
| ad-hoc | 终端 | 0 | 0 | 0 | 0 | 0 |
| Developer ID | 终端 | 0 | 0 | 0 | 0 | 0 |
| Developer ID | app bundle 内二进制路径 | 0 | 0 | 0 | 0 | 0 |
| 任一 | Raycast | **未自动化**（见下） | | | | |

### 未签名 → SIGKILL

本机对**已剥离签名**的 Mach-O 直接 exec 被杀死（exit 137），未进入路径读取逻辑。与「未签名触发 TCC 弹窗」不同：在当前 Gatekeeper 策略下，未签名二进制往往**跑不起来**。开发与分发应始终至少 ad-hoc 或 Developer ID。

### Developer ID CDHash

| 步骤 | CDHash（sha256 截断行） |
|---|---|
| 首次签名 | `d2560b9e…0841` |
| 改源码 rebuild + 再签名 | `b621462c…c3fb`（**变了**） |
| 相同字节再 `codesign --timestamp` | `9d435e09…cb9c`（**又变了**） |

结论：

1. **每次正式 rebuild + Developer ID 重签会换 CDHash** → TCC 可能视为新程序；发布应用固定 Release 产物，开发迭代不要依赖「用户给 debug 二进制的 FDA」。
2. **带 `--timestamp` 的重签即使字节相同也会改 CDHash**（时间戳进签名）。对比稳定性时应用同一时间戳策略，或接受「仅同一次签名产物稳定」。
3. **Release / Homebrew 固定二进制**的 CDHash 在用户机器上稳定，直到升级。

### 终端继承

ad-hoc 与 Developer ID 在终端下对探针目录均为 exit 0，**本次未观察到新的交互式 TCC 弹窗**（可能继承终端已有权限，或探针目录本身不触发敏感策略）。**不能**据此声称「不需要 Full Disk Access」；真实 `~/Library/Containers/<real.app>` 仍可能受限。

### Raycast / 真正的 app GUI spawn

未在本脚本自动化。手动步骤见脚本末尾。若需补测：用 Raycast Script Command 指向 Developer ID 的 `vole analyze --json`，记录弹窗上的应用名。

## 对产品的含义

1. **分发**：继续 Developer ID（+ 公证）；不要期望用户跑未签名 debug 产物。  
2. **开发**：ad-hoc 足够本地迭代路径探测；不要假设 debug CDHash 跨编译稳定。  
3. **桌面 app**：内嵌 vole 应与 app **同一 Developer ID / bundle 签名**，让 TCC 绑在 app 身份上；CLI 从终端跑与 app spawn 是不同身份面。  
4. **clean 预热**：仍值得在 clean 路径保留「轻量 touch/探测敏感根」的 UX（对齐 mole），但授权成功与否取决于启动身份，不是单靠探针目录 exit 0。

## 设计文档

结论已回写 `docs/wukong-code/specs/2026-07-29-rust-rewrite-design.md` §4.1。  
旧 deferred 记录保留：`2026-07-phase1-tcc-deferred.md`。
