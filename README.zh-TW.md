<div align="center">

<img src="images/vole.webp" alt="Vole mascot" width="180" />

# Vole

**給 macOS 用的清理與監控命令列工具**  
先看再清 · 預設進廢紙簍 · 一個命令裝好就能用

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/github/v/tag/wukongnotnull/vole?label=version)](https://github.com/wukongnotnull/vole/releases)
[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-black.svg)](https://github.com/wukongnotnull/vole)
[![Download](https://img.shields.io/github/downloads/wukongnotnull/vole/total.svg)](https://github.com/wukongnotnull/vole/releases/latest)
[![Stars](https://img.shields.io/github/stars/wukongnotnull/vole?style=social)](https://github.com/wukongnotnull/vole/stargazers)

</div>

**多語言：** [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

> 快取、日誌、解除安裝殘留、安裝套件、建置垃圾……Vole 幫你找出來，**先預覽再清理**。預設進廢紙簍，刪錯還能找回。

---

**快捷導覽**
[介面預覽](#介面預覽) · [能做什麼](#能做什麼) · [安裝](#安裝) · [使用](#使用) · [安全說明](#安全說明) · [常見問題](#常見問題) · [桌面版](#更喜歡圖形介面) · [關於我](#關於我) · [鳴謝](#鳴謝) · [授權條款](#授權條款)

---

## 介面預覽

<p align="center">
  <img src="images/tui/home.png" alt="Vole 互動式首頁" width="720" />
</p>

終端機執行 `vole` 即可開啟互動式首頁：方向鍵移動，Enter 進入。

---

## 能做什麼

| 功能 | 你能得到什麼 |
|------|-------------|
| **清理** | 掃出快取、日誌、殘留，確認後再清理 |
| **解除安裝** | 卸掉 App，並盡量清掉殘留檔案 |
| **最佳化** | 做一組安全範圍內的系統維護（如重新整理快取等） |
| **淨化** | 清理陳舊專案建置物等佔空間的大件 |
| **安裝套件** | 找出磁碟上落灰的 `.dmg` / `.pkg` |
| **分析** | 看哪個資料夾、哪些大檔最佔空間 |
| **歷史** | 回看做過的清理與刪除紀錄 |
| **狀態** | 即時看 CPU、記憶體、磁碟健康情況 |
| **Worktree** | 列出遺留 Git worktree，確認後移入廢紙簍 |

打開終端機輸入 `vole`，會進入互動式首頁，用方向鍵選功能即可。內建約 **540** 條清理規則，**不必再單獨安裝**其他小工具。

適合：會開終端機、想安全清理 Mac，又不喜歡「一鍵全刪」的人。想要視窗介面請看下方「桌面版」。

---

## 安裝

需要 **macOS 12 或更高**。

目前已發布版本：**[v2.19.0](https://github.com/wukongnotnull/vole/releases/tag/v2.19.0)**（Developer ID 簽名並經 Apple 公證）。Apple Silicon 與 Intel 均有對應安裝包。

### 方式一：發給 AI 安裝

把下面這段提示詞複製發給 Cursor、Claude Code、Codex、ChatGPT 等 AI 助手，它會幫你完成安裝。

```text
請在這台 Mac 上安裝 Vole（macOS 清理與監控命令列工具）。

官方倉庫：https://github.com/wukongnotnull/vole
需要 macOS 12+。只負責安裝，不要執行 clean / uninstall / optimize 等會改動系統的命令。

按順序嘗試，前一步成功就停：
1. 若已有 Homebrew：
   brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
   brew install vole
2. 否則從 GitHub Releases 下載最新公證包
   （Apple Silicon 用 aarch64-apple-darwin，Intel 用 x86_64-apple-darwin），
   把 bin/vole 裝到 ~/.local/bin，並把 share/vole/rules 複製到 ~/.local/share/vole/rules。
   如需要，把 ~/.local/bin 寫入 ~/.zshrc 的 PATH。
3. 不要從原始碼編譯，除非上面兩種方式都失敗。

裝完後執行 `vole --version`，告訴我安裝路徑和版本。
```

也可以直接說：`幫我按 https://github.com/wukongnotnull/vole 的官方說明安裝 Vole`

### 方式二：Homebrew

```bash
brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
brew install vole
```

裝好後直接執行 `vole`。若 brew 安裝失敗或版本對不上，請改用下方「下載安裝包」。

### 方式三：下載安裝包

1. 開啟 [最新 Release](https://github.com/wukongnotnull/vole/releases/latest)
2. 下載對應晶片的壓縮包：
   - Apple Silicon（M 系列）：`…-aarch64-apple-darwin.tar.gz`
   - Intel：`…-x86_64-apple-darwin.tar.gz`
3. 解壓後，把 `bin/vole` 放到你 PATH 裡的目錄（例如 `~/.local/bin`），並保留同包裡的 `share/vole/rules` 目錄

範例（Apple Silicon / v2.19.0；請以 Release 頁實際檔名為準）：

```bash
curl -LO https://github.com/wukongnotnull/vole/releases/download/v2.19.0/vole-2.19.0-aarch64-apple-darwin.tar.gz
tar xzf vole-2.19.0-aarch64-apple-darwin.tar.gz
mkdir -p ~/.local/bin ~/.local/share/vole
install -m 755 vole-2.19.0-aarch64-apple-darwin/bin/vole ~/.local/bin/vole
cp -R vole-2.19.0-aarch64-apple-darwin/share/vole/rules ~/.local/share/vole/
```

若終端機提示找不到 `vole`，把下面這行寫進 `~/.zshrc` 後執行 `source ~/.zshrc`：

```bash
export PATH="$HOME/.local/bin:$PATH"
```

---

## 使用

### 常用命令

```bash
# 日常
vole                           # 互動式首頁（最省事）
vole status                    # 看機器狀態
vole analyze                   # 誰佔了磁碟
vole clean                     # 掃描 → 確認 → 清理（預設廢紙簍）
vole uninstall                 # 互動式解除安裝 App
vole optimize                  # 系統維護
vole history                   # 回看做過什麼

# 只預覽、先不刪（適合不放心時）
vole clean --plan
vole uninstall --plan
vole optimize --plan
vole purge --plan
vole installer --plan
vole worktree --plan

# 看過候選後再執行
vole clean --apply <plan.json>
vole uninstall --apply <plan.json>

# 其他常用
vole touchid status            # 查看 sudo Touch ID
vole update                    # 升級到新版本
vole remove --plan             # 解除安裝 Vole 自身（僅預覽）
vole --help
vole --version
```

預設進廢紙簍。只有你主動加上「永久刪除」相關選項時，才會無法從廢紙簍復原。

---

### 全部命令

不帶子命令時，在終端機執行 `vole` 會開啟互動式首頁。

| 命令 | 別名 | 說明 |
|------|------|------|
| `vole` | — | 互動式首頁（清理 / 解除安裝 / 最佳化 / 分析 / 狀態 / Worktree） |
| `vole clean` | — | 清理快取與殘留 |
| `vole uninstall` | — | 解除安裝應用及殘留 |
| `vole optimize` | `optimise` | 系統最佳化與維護 |
| `vole status` | — | 即時健康面板（CPU / 記憶體 / 磁碟） |
| `vole analyze` | `analyse` | 目錄體積分析（預設從個人主目錄開始） |
| `vole history` | — | 操作歷史與刪除紀錄 |
| `vole purge` | — | 清理陳舊專案建置物 |
| `vole worktree` | — | 列出遺留 Git worktree，確認後移入廢紙簍 |
| `vole installer` | — | 尋找並清理安裝套件 |
| `vole touchid` | — | 設定 sudo 的 Touch ID（`status` / `enable` / `disable`） |
| `vole update` | — | 自我更新（只有你主動執行才會連網） |
| `vole remove` | — | 解除安裝 Vole 自己 |
| `vole completions` | `completion` | 產生終端機自動補全 |
| `vole help` | — | 說明（也可用 `-h` / `--help`） |
| `vole --version` | `-V` | 列印版本 |

## 安全說明

```
你        ❯ vole clean → 查看候選 → 確認

Vole      ❯ ✓ 先列出候選，不直接動手
            ✓ 預設進廢紙簍（可復原）
            ✓ 執行前再檢查保護路徑
            ✓ 不確定就略過——不會悄悄擴大刪除範圍
```

| 原則 | 含義 |
|------|------|
| **先預覽再執行** | 終端機裡會詢問確認；也可用 `--plan` 只看不刪 |
| **預設可復原** | 個人檔案預設進廢紙簍 |
| **報告清楚** | 會區分「進廢紙簍」和「永久刪除」 |
| **可追溯** | 用 `vole history` 回看 |

日常清理在本機完成。只有執行 `vole update` 時才會連網下載更新。

---

## 常見問題

**Q：會不會不詢問就刪除？**  
A：在終端機裡直接執行 `clean` / `optimize` 時，會問你是否繼續（預設否）。不放心可以先用 `--plan` 只看列表。

**Q：刪錯了怎麼辦？**  
A：預設進廢紙簍，打開廢紙簍還原即可。若你當時選了永久刪除，則無法從廢紙簍找回。

**Q：App 已經卸了，還是還裝著？**  
A：卸乾淨後的殘留 → `vole clean`。App 還在 → `vole uninstall`。

**Q：提示找不到命令 `vole`？**  
A：確認安裝路徑已加入 PATH（見上方 `~/.local/bin` 範例），新開一個終端機視窗再試。

**Q：更想用圖形介面？**  
A：請用 [Vole for macOS](https://github.com/wukongnotnull/vole-macos)——同一套清理能力，視窗操作，支援 Apple Silicon 與 Intel。

**Q：和 Mole 是什麼關係？**  
A：清理規則與安全思路受到 [Mole](https://github.com/tw93/Mole) 啟發；Vole 是獨立開源專案，不隸屬於 Mole。

---

## 更喜歡圖形介面？

[Vole for macOS](https://github.com/wukongnotnull/vole-macos) 是配套桌面版：側欄提供清理、解除安裝、最佳化、淨化、安裝套件、分析、歷史、狀態；可授予完整磁碟取用權限，並可選啟用 Root 特權助手清理部分系統路徑。

最新桌面版見 [vole-macos Releases](https://github.com/wukongnotnull/vole-macos/releases/latest)（目前為 **v0.2.0** Universal DMG）。

```text
同一套清理能力 · 同一套安全習慣 · 終端機或視窗任選
```

---

## 關於我

**悟空非空也** — AI之道創辦人，獨立開發者，Up 主。

| 平台 | 連結 |
|------|------|
| 🌐 官網 | [AI之道官網](https://waytoai.cn) |
| 𝕏 Twitter | [悟空非空也](https://x.com/wukongnotnull) |
| 📺 B站 | [悟空非空也](https://space.bilibili.com/456634391) |
| ▶️ YouTube | [悟空非空也](https://www.youtube.com/@wukongnotnull) |
| 📕 小紅書 | [悟空非空也](https://www.xiaohongshu.com/user/profile/5ca89c2f000000001100952b) |
| 💬 公眾號 | 微信搜「悟空非空也」 |

---

## 鳴謝

感謝這些產品與開源專案在 macOS 清理體驗上的探索與累積，Vole 從中獲益良多：

- [Mole](https://github.com/tw93/Mole) — 開源清理工具；規則與安全思路的重要啟發
- [CleanMyMac](https://macpaw.com/cleanmymac) — 成熟的桌面清理產品體驗參考
- [騰訊檸檬清理](https://lemon.qq.com/) — 中文使用者熟悉的系統清理產品參考

Vole 是獨立開源專案，與上述產品無隸屬或商業關係。

---

## 授權條款

Vole 遵循 [GPL-3.0](LICENSE)。  
清理規則與安全思路受到 [Mole](https://github.com/tw93/Mole) 啟發；Vole 是獨立開源專案，不隸屬於 Mole。  
如需基於本專案做自有產品，請更換名稱以避免混淆，並註明來源於 Mole / Vole。

---

<div align="center">

GPL-3.0 license © [悟空非空也](https://github.com/wukongnotnull)

</div>
