# Brew one-liner install notes

日期：2026-07-30  
分支：`feat/brew-one-liner`  
设计：`docs/wukong-code/specs/2026-07-30-2255-brew-one-liner-design.md`

## 验证

### 单元测试

```bash
cargo test -p vole-core default_rules_dir_finds_share_layout -- --nocapture
cargo test -p vole-core --lib
```

结果：新测试 PASS；`vole-core` lib **165 passed**.

### Release 布局（无 `VOLE_RULES_DIR`）

用 GitHub Release `v1.2.0` aarch64 tarball 解压后：

```bash
unset VOLE_RULES_DIR
./bin/vole --help
./bin/vole clean --plan   # exit 0，产出含多条 rule 候选的 plan JSON
```

结论：`bin/vole` + `share/vole/rules` 相对发现可用，无需 env。

### Formula 布局 bug

Homebrew 解压单顶层目录后，`install` 的 cwd/`buildpath` 已在 `vole-1.2.0-<arch>/` 内，原 `Dir["vole-#{version}-*"]` 恒空 → `unexpected tarball layout`。  
已改为优先检测 `buildpath/bin/vole`。

### Homebrew 本机安装

```bash
git -C /opt/homebrew/Library/Taps/wukongnotnull/homebrew-vole pull
HOMEBREW_NO_AUTO_UPDATE=1 brew reinstall --formula wukongnotnull/vole/vole
unset VOLE_RULES_DIR
vole clean --plan   # exit 0；本机观测 entries=964
ls "$(brew --prefix vole)/share/vole/rules"
```

Caveats 已不再要求 `export VOLE_RULES_DIR`。

### 文档同步

- `README.md` / `docs/findings/2026-07-phase5-signing.md` / `install.sh`：去掉强制 env；Homebrew 主路径两行（tap + install）。

## Core formula

候选：`docs/homebrew/vole-homebrew-core.rb`（源码构建，`std_cargo_args` + `pkgshare.install "data/rules"`）。

### 本机构建

临时 tap `wukongnotnull/vole-core-test`：

```text
==> cargo install --path=crates/vole-cli
🍺  /opt/homebrew/Cellar/vole/1.2.0: 14 files, 5.8MB, built in 2 minutes 17 seconds
```

无 `VOLE_RULES_DIR`：`vole clean --plan` → entries=989，exit 0。

### Audit

```bash
brew audit --strict --online wukongnotnull/vole-core-test/vole
```

初版问题（已修进候选）：

- Use `pkgshare` instead of `share/"vole"`（两处）

`--new` audit：与 `--strict` 相同问题已消除后 **exit 0、无输出问题**。

## PR

Homebrew Core new formula PR：https://github.com/Homebrew/homebrew-core/pull/296168（曾因模板不完整被 bot 关闭，已按官方模板补全并自动 reopen）

分支：`wukongnotnull/homebrew-core` → `vole-1.2.0-new-formula`

## 验收

| # | 标准 | 证据 |
|---|---|---|
| 1 | 未设 env 可加载 rules | bottle 布局 entries=964；源码构建 entries=989 |
| 2 | caveats 无强制 export | `Formula/vole.rb` caveats 仅 Gatekeeper |
| 3 | README / findings 同步 | Task 2 + Core PR 链接 |
| 4 | core 候选 + audit | `docs/homebrew/vole-homebrew-core.rb`；`brew audit --new` 通过 |
| 5 | PR 或阻塞说明 | https://github.com/Homebrew/homebrew-core/pull/296168 |

本仓部分已交付；真正一行 `brew install vole` 取决于 Core PR 合并。

## Homebrew Core CI（2026-07-30）

[PR #296168](https://github.com/Homebrew/homebrew-core/pull/296168) CI 失败根因（`brew audit --online --new`）：

```text
Self-submitted GitHub repository not notable enough
(<90 forks, <90 watchers and <225 stars)
```

本机/CI 构建本身成功（x86 bottle 已打出）；阻塞是 **Homebrew 新 formula 知名度门禁**，不是编译错误。

当时仓库指标：stars=0 / forks=0 / watchers=0（仓库创建于 2026-07-29）。

**对策：**
1. 短期：继续自建 tap 两行安装（已合并进 vole `main`）。
2. 达标后再推 Core：需达到 forks≥90 **或** watchers≥90 **或** stars≥225（满足其一即可，以 audit 文案为准）。
3. 不要为此改 formula「绕过」audit；无公开 exception 路径。

