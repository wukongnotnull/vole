# 产品 v2 CLI 全家桶 · 收口 findings

**日期**：2026-08-08  
**状态**：完成  
**规格**：[`../wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md`](../wukong-code/specs/2026-08-08-2030-v2-cli-complete-design.md)  
**计划**：[`../wukong-code/plans/2026-08-08-2354-v2-cli-complete-closeout.md`](../wukong-code/plans/2026-08-08-2354-v2-cli-complete-closeout.md)  
**包版本**：停在 **`2.5.0`**（收口仅文档 + CI 闸门，无新 CLI 能力，未 bump）

## 里程碑完成态

| 里程碑 | 内容 | 版本 | 状态 |
|---|---|---|---|
| M4 | CLI 做全 spike + 闸门 stub | docs | ✅ |
| M5 | `purge` + 别名 `optimise`/`analyse`/`completion` | 2.0.0 | ✅ |
| M6 | `clean` 内 hints（非顶层命令） | 2.1.0 | ✅ |
| M7 | `installer` | 2.2.0 | ✅ |
| M8 | `touchid` | 2.3.0 | ✅ |
| M9 | 自更新 `update` | 2.4.0 | ✅ |
| M10 | 自卸载 `remove` | 2.5.0 | ✅（PR #110 / `7ec8ea7`） |
| **收口** | §3.2 CI `--enforce` + README「全家桶」+ 本 findings | 2.5.0 | ✅ |

## §3.2 闸门

```bash
./scripts/check-command-surface.sh --enforce
# → OK: command surface covers required set（gaps=0）
# → OK: interactive.rs has no update/network probe markers
# → 无顶层 Hints；别名 analyse / optimise / completion 均检出
```

CI：`.github/workflows/ci.yml` 步骤 `Command surface (v2 CLI ⊇ Mole routes)` 调用 `--enforce`，gaps ≠ 0 即红。

## 成功标准核对（规格 §3.4 摘要）

- [x] 命令面 ⊇ Mole 1.48.1 路由（豁免 `hints` 形态 / `whitelist` 形态差异）
- [x] 别名 clap `visible_alias` 已落地
- [x] §3.2 机械闸门通过且 CI 硬门禁
- [x] README 明确「产品 v2 CLI 全家桶」并列出/指向子命令
- [x] 包线 `2.x`，首发 `2.0.0`，收口时最新为 `2.5.0`
- [x] 桌面 / SMAppService / Mole 广谱边缘：**非本续篇主路径**，诚实记录为余项

## 发版运营（与「CLI 做全」收口分离）

本 findings 宣告的是 **命令面 / 包版本线收口**，**不是** GitHub Release 资产齐全。

截至 2026-08-09 核验：`v2.5.0` tag / Release / tarball+`SHA256SUMS` / Formula 真实 sha **均未齐**；详见 [`../releases/v2.5.0.md`](../releases/v2.5.0.md)「发版运营状态」。对外装包通道在打 tag 并跑通 `Release` workflow 之前，仍以已发布的 `v1.28.0`（或 `brew install --HEAD`）为准。

## 结论

产品 v2 续篇「CLI 做全」可宣告完成。**GitHub 预编译发版运营另计**（见上节）。后续缺陷走 PATCH；新能力另开 design，不占用本收口。
