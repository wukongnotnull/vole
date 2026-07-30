# Mail / 云盘具名缓存 + XCTestDevices

**日期**：2026-07-30  
**状态**：已落地  
**规则**：504 → **509**（+5）

## 新增

| id | 路径 | 说明 |
|---|---|---|
| `baidu-netdisk-cache` | `~/Library/Caches/com.baidu.netdisk` | 具名；亦被 `user-app-cache` 广域覆盖 |
| `alibaba-cloud-cache` | `~/Library/Caches/com.alibaba.teambitiondisk` | 同上 |
| `box-cache` | `~/Library/Caches/com.box.desktop` | 同上 |
| `apple-mail-cache` | `~/Library/Caches/com.apple.mail/*` | 同上 |
| `xcode-xctest-devices` | `~/Library/Developer/XCTestDevices/*` | 多进程 `not_running` + cmdline（对齐 mole） |

## XCTestDevices guards

精确名：`Xcode` / `Simulator` / `CoreSimulatorService` / `simdiskimaged` / `xcodebuild` / `xctest` / `XCTRunner`  
cmdline：`com.apple.CoreSimulator` / `com.apple.dt.XCTest` / `XCTest`
