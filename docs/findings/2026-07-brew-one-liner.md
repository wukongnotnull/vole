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
