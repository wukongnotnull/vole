# M4：§3.2 命令面核对闸门 — 清单草案

**规格**：[`2026-08-08-2030-v2-cli-complete-design.md`](../wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md) §3.2  
**Stub**：[`scripts/check-command-surface.sh`](../../scripts/check-command-surface.sh)  
**完整 CI 强制**：留给**收口**里程碑；M4 仅 stub + 本清单。

## 必覆盖集合（Mole 路由 − 豁免）

```text
clean uninstall optimize optimise analyze analyse status history
completion completions help version
purge installer touchid update remove
```

不要求顶层：`hints`、`whitelist`。

## 可执行步骤

1. 从 `third_party/mole-1.48.1/mole` 解析路由（含 early `history`、英式别名）。
2. 从 `vole --help`（macOS）或 `crates/vole-cli/src/main.rs` 解析 Vole 顶层命令 + 别名。
3. 断言必覆盖集合 ⊆ Vole 命令面（差集为空）。
4. 断言**无**顶层 `Hints` / `vole hints`。
5. 断言裸调用路径无 `check_for_updates` 等价联网：静态检查 `crates/vole-cli/src/interactive.rs` 无 GitHub/brew/版本探测调用。
6. 收口：将 stub 的 `--enforce` 接入 CI，gaps≠0 即红。

## M4 验收

```bash
./scripts/check-command-surface.sh          # report-only，exit 0，打印 MISSING
./scripts/check-command-surface.sh --enforce  # 当前应非 0（M5 前）
```
