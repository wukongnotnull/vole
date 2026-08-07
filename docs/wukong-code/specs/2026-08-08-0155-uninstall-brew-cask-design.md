# uninstall brew cask 卸载联动设计（W2a①）

- 日期：2026-08-08
- 状态：已批准（盘点 Condensed；用户指示默认采纳）→ writing-plans
- 依据：路线图 [`2026-08-08-0119-mole-parity-roadmap-design.md`](2026-08-08-0119-mole-parity-roadmap-design.md) §2.3 W2a①；[`../../findings/2026-07-v2-m1-uninstall.md`](../../findings/2026-07-v2-m1-uninstall.md)；Mole `third_party/mole-1.48.1/lib/uninstall/brew.sh` + `batch.sh` brew 路由
- 包版本意图：**1.30.0**（MINOR；若与并行轨冲突则 rebase 顺延）
- **不 bump** `schema_version`

## 1. 结论

在既有 `vole uninstall` plan→apply 上交付 **Homebrew Cask 卸载联动**：

- **plan**：对 Applications 扫描到的应用，若判定为 brew cask，本体条目 `rule_id` 改为  
  `uninstall:brew-cask:zap:<token>` 或 `uninstall:brew-cask:nozap:<token>`（token = cask 名）
- **apply**：对该类条目执行 `brew uninstall --cask [--zap]`（参数分列、可注入），**不**绕过 Uninstall 保护 / TOCTOU / whitelist
- **sibling guard**：存在同 bundle 存活 sibling → **nozap**（对齐 Mole；zap stanza 会误伤 sibling 的 prefs/caches）
- leftovers：仍走现有 `uninstall:leftover:`；sibling 时继续抑制（既有行为）
- **非本刀**：login items、系统 LaunchDaemons / `/Library` sudo（W2a②③）、W1 快照、W2b/W2c

**采纳路径**：新模块 `vole-core::brew_cask` + plan/apply 窄接线；BrewDeps 可注入（测不下真 brew）。

## 2. 问题与风险

1. **误卸错误 cask**：多阶段检测必须 fail-closed；Caskroom 搜到多个 token → 不认定；stage 4 须 `brew info` 路径交叉验证。
2. **zap 误伤 sibling**：计划阶段写死 zap/nozap；apply 不再猜。
3. **brew 失败后硬删造成 brew 登记与磁盘不一致**：仅当 `is_brew_cask_installed` 明确为「未安装」时才回退 `mole_delete`；仍安装或状态未知 → skip + 人读提示手动 `brew uninstall --cask …`。
4. **交互/密码**：`HOMEBREW_NO_ENV_HINTS=1 HOMEBREW_NO_AUTO_UPDATE=1 NONINTERACTIVE=1`；不在本刀实现 Mole 式 sudo 预授权；超时默认 300s（大包可加码，测试可缩短）。
5. **保护绕过**：brew 路径前仍过 `should_protect_from_uninstall` / official uninstaller / `validate_path_for_deletion` / apply `verify_plan_entry_for_apply`。
6. **协议**：靠 `rule_id` 编码 token + zap 模式；**不**改 PlanEntry 字段、不 bump schema。

## 3. 方案对比（已选）

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. rule_id 编码 + BrewDeps（已选）** | detect → 特规 rule_id → apply 调 brew | 零协议 bump；可测 | rule_id 略长 |
| B. plan JSON 增字段 | 显式 `brew_cask` / `zap` | 清晰 | 需 schema / 兼容 |
| C. 仅 coverage 提示、不调 brew | 文档对齐 | 零风险 | 不兑现 W2a① |

## 4. 产品行为

```bash
vole uninstall --plan …              # 检出 brew 应用时，本体 rule_id 带 brew-cask
vole uninstall --plan "Some App" …
vole uninstall --apply <plan.json>   # brew 条目走 brew；其余仍 mole_delete_verified
```

- label 可含 `[Brew:<token>]` 便于人读（可选，非协议要求）
- 无 Homebrew / 检测失败：按普通 app 卸载（mole_delete 路径）；coverage_note 可计 `brew_miss=N`
- 显式认定失败（歧义 token 等）：不把该 app 标为 brew；等同普通路径

## 5. 检测（顺序写死，对齐 Mole）

输入：`app_path`；输出：`Option<CaskToken>`。

1. `brew` 不可用 → `None`（整次会话可缓存）
2. **Stage 1**：`realpath`/`canonicalize` 后路径落在  
   `/opt/homebrew/Caskroom/<token>/…` 或 `/usr/local/Caskroom/<token>/…`；token 校验 `^[a-z0-9][a-z0-9-]*$`
3. **Stage 2**：在两 Caskroom 下 `find` maxdepth 3 同名 `.app`；唯一 token 且 `brew list --cask` 含之，且 `brew info --cask` 文本含 `app_path` 或 `/Applications/<bundle>` 或 bundle 名 → 采用；多 token → `None`
4. **Stage 3**：`app_path` 为 symlink，readlink 目标可抽 token → 采用
5. **Stage 4**：`brew list --cask` 精确匹配（大小写不敏感）`basename(app).strip_suffix(.app)`；`brew info` 交叉验证同上；失败 → `None`

生产：`LiveBrewDeps` 调真 `brew`；测试：Fake + 目录夹具（Caskroom 形路径可不调 brew 完成 stage 1/3）。

## 6. Plan 接线

对每个通过保护筛选的 app：

1. `detect_cask(app_path)` → optional token  
2. sibling → `zap_mode = nozap`，否则 `zap`  
3. 本体 `rule_id = uninstall:brew-cask:{zap|nozap}:{token}`（有 token 时）；否则仍 `uninstall:{bundle_id}`  
4. leftovers 规则不变；sibling 抑制不变  
5. `coverage_note` 更新：去掉「brew cask zap」长尾；保留 login items / system LaunchDaemons / `/Library` sudo；可含 `brew_cask=N`

## 7. Apply 接线

对每条 entry（保护 / TTL / schema / TOCTOU 与现网一致之后）：

| rule_id 前缀 | 行为 |
|---|---|
| `uninstall:brew-cask:zap:` / `uninstall:brew-cask:nozap:` | 解析 token；`brew uninstall --cask` [+`--zap`]；见下 |
| 其他 `uninstall:` | 现有 `mole_delete_verified` |

brew 分支：

1. 解析 token；非法 → skip  
2. `BrewDeps::uninstall_cask(token, zap_mode, Optional app_path)`  
3. 成功且（cask 未安装 **且** app 路径已消失或本不要求）→ succeeded  
4. 失败：若 cask **明确未安装** → 允许对 **app 路径** 回退 `mole_delete_verified`；若仍安装或未知 → skip（Whitelisted/PathVanished 类记法或 failed 计数按现网风格；建议 skip + 可读建议）  
5. **禁止**对任意路径因「brew 装饰」而跳过 `UninstallPathProtection`

超时：默认 300s；路径 size 启发可对齐 Mole（>5GB→600s，>15GB→900s）；测试 Fake 瞬时返回。

## 8. 覆盖说明 / 文档

- `uninstall_plan` coverage：Long-tail **不再**列 brew cask zap  
- 全局 `coverage.rs` / README：诚实写「uninstall brew cask 联动已落地」；长尾改为 login items、系统 LaunchDaemons、`/Library` sudo  
- findings `2026-07-v2-m1-uninstall.md`：① 标完成  
- Formula / workspace version：**1.30.0**（窄改）

## 9. 测试与安全

1. Stage 1：fixture app realpath → Caskroom → 检出 token  
2. 多 token find → `None`  
3. sibling → plan rule_id 含 `nozap`  
4. 无 sibling → `zap`  
5. Fake uninstall：zap 调一次带 `--zap`；nozap 不带  
6. brew 失败且 cask 仍在 → 不 mole_delete  
7. brew 失败且 cask 已不在 → 回退 delete 一次  
8. Safari / 保护 app → 仍不进 plan（含 brew 装饰也不能进）  
9. PR：**security-review** 必过；合并 `gh pr merge --merge`

## 10. 验收

1. 真机（可选）：已装 cask（如一小应用）`uninstall --plan` 见 brew-cask rule_id；`--apply` 后 `brew list --cask` 无该 token  
2. 单测全绿；保护单测不回归  
3. 版本 **1.30.0**；②③ / W1 / W2b / W2c 未改动  

## 11. 下一步

`writing-plans` → `docs/wukong-code/plans/YYYY-MM-DD-HHmm-uninstall-brew-cask.md` → branch `feat/uninstall-brew-cask` 实现。
