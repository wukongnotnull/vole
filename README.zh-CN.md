<div align="center">

<img src="images/vole.webp" alt="Vole mascot" width="180" />

# Vole

**给 macOS 用的清理与监控命令行工具**  
先看再清 · 默认进废纸篓 · 一个命令装好就能用

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/github/v/tag/wukongnotnull/vole?label=version)](https://github.com/wukongnotnull/vole/releases)
[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-black.svg)](https://github.com/wukongnotnull/vole)
[![Download](https://img.shields.io/github/downloads/wukongnotnull/vole/total.svg)](https://github.com/wukongnotnull/vole/releases/latest)
[![Stars](https://img.shields.io/github/stars/wukongnotnull/vole?style=social)](https://github.com/wukongnotnull/vole/stargazers)

</div>

**多语言：** [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

> 缓存、日志、卸载残留、安装包、构建垃圾……Vole 帮你找出来，**先预览再清理**。默认进废纸篓，删错还能找回。

---

**快捷导航**
[界面预览](#界面预览) · [能做什么](#能做什么) · [安装](#安装) · [使用](#使用) · [安全说明](#安全说明) · [常见问题](#常见问题) · [桌面版](#更喜欢图形界面) · [关于我](#关于我) · [鸣谢](#鸣谢) · [许可证](#许可证)

---

## 界面预览

<p align="center">
  <img src="images/tui/home.png" alt="Vole 交互式首页" width="720" />
</p>

终端运行 `vole` 即可打开交互式首页：方向键移动，Enter 进入。

---

## 能做什么


| 功能      | 你能得到什么                   |
| ------- | ------------------------ |
| **清理**  | 扫出缓存、日志、残留，确认后再清理        |
| **卸载**  | 卸掉 App，并尽量清掉残留文件         |
| **优化**  | 做一组安全范围内的系统维护（如刷新缓存等）    |
| **净化**  | 清理陈旧项目构建物等占空间的大件         |
| **安装包** | 找出磁盘上落灰的 `.dmg` / `.pkg` |
| **分析**  | 看哪个文件夹、哪些大文件最占空间         |
| **历史**  | 回看做过的清理与删除记录             |
| **状态**  | 实时看 CPU、内存、磁盘健康情况        |


打开终端输入 `vole`，会进入交互式首页，用方向键选功能即可。内置约 **540** 条清理规则，**不必再单独安装**其他小工具。

适合：会开终端、想安全清理 Mac，又不喜欢「一键全删」的人。想要窗口界面请看下方「桌面版」。

---



## 安装

需要 **macOS 12 或更高**。

当前已发布版本：**[v2.16.0](https://github.com/wukongnotnull/vole/releases/tag/v2.16.0)**（Developer ID 签名并经 Apple 公证）。Apple Silicon 与 Intel 均有对应安装包。

### 方式一：下载安装包

1. 打开 [最新 Release](https://github.com/wukongnotnull/vole/releases/latest)
2. 下载对应芯片的压缩包：
  - Apple Silicon（M 系列）：`…-aarch64-apple-darwin.tar.gz`
  - Intel：`…-x86_64-apple-darwin.tar.gz`
3. 解压后，把 `bin/vole` 放到你 PATH 里的目录（例如 `~/.local/bin`），并保留同包里的 `share/vole/rules` 目录

示例（Apple Silicon / v2.16.0；请以 Release 页实际文件名为准）：

```bash
curl -LO https://github.com/wukongnotnull/vole/releases/download/v2.16.0/vole-2.16.0-aarch64-apple-darwin.tar.gz
tar xzf vole-2.16.0-aarch64-apple-darwin.tar.gz
mkdir -p ~/.local/bin ~/.local/share/vole
install -m 755 vole-2.16.0-aarch64-apple-darwin/bin/vole ~/.local/bin/vole
cp -R vole-2.16.0-aarch64-apple-darwin/share/vole/rules ~/.local/share/vole/
```

若终端提示找不到 `vole`，把下面这行写进 `~/.zshrc` 后执行 `source ~/.zshrc`：

```bash
export PATH="$HOME/.local/bin:$PATH"
```



### 方式二：Homebrew（推荐）

```bash
brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
brew install vole
```

装好后直接运行 `vole`。若 brew 安装失败或版本对不上，请改用上方「下载安装包」。

---



## 使用



### 常用命令

```bash
# 日常
vole                           # 交互式首页（最省事）
vole status                    # 看机器状态
vole analyze                   # 谁占了磁盘
vole clean                     # 扫描 → 确认 → 清理（默认废纸篓）
vole uninstall                 # 交互式卸载 App
vole optimize                  # 系统维护
vole history                   # 回看做过什么

# 只预览、先不删（适合不放心时）
vole clean --plan
vole uninstall --plan
vole optimize --plan
vole purge --plan
vole installer --plan

# 看过候选后再执行
vole clean --apply <plan.json>
vole uninstall --apply <plan.json>

# 其他常用
vole touchid status            # 查看 sudo Touch ID
vole update                    # 升级到新版本
vole remove --dry-run          # 卸载 Vole 自身（仅预览）
vole --help
vole --version
```

默认进废纸篓。只有你主动加上「永久删除」相关选项时，才会不可从废纸篓恢复。

---



### 全部命令

不带子命令时，在终端运行 `vole` 会打开交互式首页。


| 命令                 | 别名           | 说明                                                  |
| ------------------ | ------------ | --------------------------------------------------- |
| `vole`             | —            | 交互式首页（清理 / 卸载 / 优化 / 分析 / 状态）                       |
| `vole clean`       | —            | 清理缓存与残留                                             |
| `vole uninstall`   | —            | 卸载应用及残留                                             |
| `vole optimize`    | `optimise`   | 系统优化与维护                                             |
| `vole status`      | —            | 实时健康面板（CPU / 内存 / 磁盘）                               |
| `vole analyze`     | `analyse`    | 目录体积分析（默认从个人主目录开始）                                  |
| `vole history`     | —            | 操作历史与删除记录                                           |
| `vole purge`       | —            | 清理陈旧项目构建物                                           |
| `vole installer`   | —            | 查找并清理安装包                                            |
| `vole touchid`     | —            | 配置 sudo 的 Touch ID（`status` / `enable` / `disable`） |
| `vole update`      | —            | 自更新（只有你主动执行才会联网）                                    |
| `vole remove`      | —            | 卸载 Vole 自己                                          |
| `vole completions` | `completion` | 生成终端自动补全                                            |
| `vole help`        | —            | 帮助（也可用 `-h` / `--help`）                             |
| `vole --version`   | `-V`         | 打印版本                                                |




## 安全说明

```
你        ❯ vole clean → 查看候选 → 确认

Vole      ❯ ✓ 先列出候选，不直接动手
            ✓ 默认进废纸篓（可恢复）
            ✓ 执行前再检查保护路径
            ✓ 不确定就跳过——不会悄悄扩大删除范围
```


| 原则         | 含义                         |
| ---------- | -------------------------- |
| **先预览再执行** | 终端里会询问确认；也可用 `--plan` 只看不删 |
| **默认可恢复**  | 个人文件默认进废纸篓                 |
| **报告清楚**   | 会区分「进废纸篓」和「永久删除」           |
| **可追溯**    | 用 `vole history` 回看        |


日常清理在本地完成。只有执行 `vole update` 时才会联网下载更新。

---



## 常见问题

**Q：会不会不询问就删除？**  
A：在终端里直接运行 `clean` / `optimize` 时，会问你是否继续（默认否）。不放心可以先用 `--plan` 只看列表。

**Q：删错了怎么办？**  
A：默认进废纸篓，打开废纸篓还原即可。若你当时选了永久删除，则无法从废纸篓找回。

**Q：App 已经卸了，还是还装着？**  
A：卸干净后的残留 → `vole clean`。App 还在 → `vole uninstall`。

**Q：提示找不到命令** `vole`**？**  
A：确认安装路径已加入 PATH（见上方 `~/.local/bin` 示例），新开一个终端窗口再试。

**Q：更想用图形界面？**  
A：请用 [Vole for macOS](https://github.com/wukongnotnull/vole-macos)——同一套清理能力，窗口操作，支持 Apple Silicon 与 Intel。

**Q：和 Mole 是什么关系？**  
A：清理规则与安全思路受到 [Mole](https://github.com/tw93/Mole) 启发；Vole 是独立开源项目，不隶属于 Mole。

---



## 更喜欢图形界面？

[Vole for macOS](https://github.com/wukongnotnull/vole-macos) 是配套桌面版：侧栏提供清理、卸载、优化、净化、安装包、分析、历史、状态；可授予完全磁盘访问，并可选启用 Root 特权助手清理部分系统路径。

最新桌面版见 [vole-macos Releases](https://github.com/wukongnotnull/vole-macos/releases/latest)（当前为 Universal DMG）。

```text
同一套清理能力 · 同一套安全习惯 · 终端或窗口任选
```

---



## 关于我

**悟空非空也** — AI之道创始人，独立开发者，Up主。


| 平台         | 链接                                                                         |
| ---------- | -------------------------------------------------------------------------- |
| 🌐 官网      | [AI之道官网](https://waytoai.cn)                                               |
| 𝕏 Twitter | [悟空非空也](https://x.com/wukongnotnull)                                       |
| 📺 B站      | [悟空非空也](https://space.bilibili.com/456634391)                              |
| ▶️ YouTube | [悟空非空也](https://www.youtube.com/@wukongnotnull)                            |
| 📕 小红书     | [悟空非空也](https://www.xiaohongshu.com/user/profile/5ca89c2f000000001100952b) |
| 💬 公众号     | 微信搜「悟空非空也」                                                                 |


---



## 鸣谢

感谢这些产品与开源项目在 macOS 清理体验上的探索与积累，Vole 从中获益良多：

- [Mole](https://github.com/tw93/Mole) — 开源清理工具；规则与安全思路的重要启发
- [CleanMyMac](https://macpaw.com/cleanmymac) — 成熟的桌面清理产品体验参考
- [腾讯柠檬清理](https://lemon.qq.com/) — 中文用户熟悉的系统清理产品参考

Vole 是独立开源项目，与上述产品无隶属或商业关系。

---



## 许可证

Vole 遵循 [GPL-3.0](LICENSE)。  
如需基于本项目做自有产品，请更换名称以避免混淆，并注明来源于 Mole / Vole。

---

GPL-3.0 license © [悟空非空也](https://github.com/wukongnotnull)