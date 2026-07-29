# 一致性测试的执行环境

一致性测试驱动两个会真实删除文件的程序，且 Mole 的规则路径不完全受 `$HOME`
约束（系统级路径、`/private/var`、废纸篓、`defaults` 域）。**不要在日常开发机上跑。**

容器不可用：macOS 的 TCC、`launchctl`、废纸篓语义在 Linux 容器里无法复现，
用容器等于测了个假东西。

## 方案一：一次性本地用户账户（推荐，最轻）

```bash
./scripts/new-test-user.sh voletest
su - voletest
```

跑完后删除账户与家目录。适合 plan 阶段用例与日常迭代。

## 方案二：macOS VM（apply 阶段用例必须用这个）

用 [Tart](https://tart.run) 起一台 macOS VM，跑前打快照、跑完回滚：

```bash
tart clone ghcr.io/cirruslabs/macos-sequoia-base:latest vole-test
tart run vole-test
# 在 VM 内 clone 仓库并跑 harness
# 跑完：tart delete vole-test && tart clone ... 重建
```

## 分层规定

| 用例类型 | 环境 | 进 CI |
|---|---|---|
| plan 阶段（只读，不删文件） | 一次性用户账户或 CI runner | 是 |
| B 类表驱动（规则引擎单测） | 任意，只碰 `VOLE_TEST_ROOT` | 是 |
| C 类 property / fuzz | 任意 | 是 |
| apply 阶段（真实删除） | **仅 VM，跑完回滚快照** | **否** |

CI 的 macOS runner 本身是一次性的，适合前三类。apply 阶段用例不进 CI——
这条规定也写在 `.github/workflows/ci.yml` 的注释里。

## 护栏

harness 强制要求 `VOLE_TEST_ROOT`，并在每次调用前后对若干根外哨兵目录
做 mtime 快照对比。任何越界改动会以退出码 2 中止整个运行，不是警告。
护栏实现在 `conformance/src/guard.rs`，其自身的有效性由
`detects_modification_outside_root` 用例保证。
