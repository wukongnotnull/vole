<div align="center">

<img src="images/vole.webp" alt="Vole mascot" width="180" />

# Vole

**macOS 向けクリーンアップ＆モニタ CLI**  
先に確認 · ゴミ箱が既定 · 入れてすぐ使える

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/github/v/tag/wukongnotnull/vole?label=version)](https://github.com/wukongnotnull/vole/releases)
[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-black.svg)](https://github.com/wukongnotnull/vole)
[![Download](https://img.shields.io/github/downloads/wukongnotnull/vole/total.svg)](https://github.com/wukongnotnull/vole/releases/latest)
[![Stars](https://img.shields.io/github/stars/wukongnotnull/vole?style=social)](https://github.com/wukongnotnull/vole/stargazers)

</div>

**言語：** [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

> キャッシュ、ログ、残骸、インストーラ、ビルドゴミ……Vole が見つけ、**プレビューしてから掃除**できます。既定はゴミ箱行きなので、間違えても戻せます。

---

**クイックナビ**
[画面プレビュー](#画面プレビュー) · [できること](#できること) · [インストール](#インストール) · [使い方](#使い方) · [安全について](#安全について) · [よくある質問](#よくある質問) · [デスクトップ版](#gui-が好きなら) · [About](#about) · [謝辞](#謝辞) · [ライセンス](#ライセンス)

---

## 画面プレビュー

<p align="center">
  <img src="images/tui/home.png" alt="Vole 対話ホーム" width="720" />
</p>

ターミナルで `vole` を実行すると対話ホームが開きます。矢印キーで移動、Enter で選択。

---

## できること

| 機能 | 得られること |
|------|-------------|
| **クリーン** | キャッシュ・ログ・残骸を検出し、確認してから削除 |
| **アンインストール** | App を削除し、残骸もできるだけ除去 |
| **最適化** | 安全な範囲のシステムメンテ（キャッシュ更新など） |
| **パージ** | 古いプロジェクトのビルド成果物など大きなゴミを掃除 |
| **インストーラ** | ディスクに残った `.dmg` / `.pkg` を発見 |
| **分析** | どのフォルダ・大きなファイルが容量を使っているかを確認 |
| **履歴** | 過去のクリーンと削除を振り返る |
| **ステータス** | CPU・メモリ・ディスクの健康状態をリアルタイム表示 |

ターミナルで `vole` と打つと対話ホームが開き、矢印キーで選べます。約 **540** 件のクリーン規則が内蔵され、**追加ツールは不要**です。

ターミナルは使えるが「一発全削除」は嫌、という人向け。ウィンドウアプリが欲しい場合は下のデスクトップ版へ。

---

## インストール

**macOS 12 以上**が必要です。

現在の公開版：**[v2.17.0](https://github.com/wukongnotnull/vole/releases/tag/v2.17.0)**（Developer ID 署名＋Apple 公証）。Apple Silicon / Intel 両対応。

### 方法 1：AI にプロンプトを送ってインストール

下のブロックを Cursor、Claude Code、Codex、ChatGPT などの AI アシスタントに貼り付けてください。代わりにインストールしてくれます。

```text
この Mac に Vole（macOS 向けクリーンアップ／モニタ CLI）をインストールしてください。

公式リポジトリ: https://github.com/wukongnotnull/vole
macOS 12 以上が必要です。インストールだけ行い、clean / uninstall / optimize などシステムを変更するコマンドは実行しないでください。

次の順で試し、成功したらそこで止めてください:
1. Homebrew がある場合:
   brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
   brew install vole
2. なければ GitHub Releases から最新の公証済みアーカイブをダウンロード
   （Apple Silicon: aarch64-apple-darwin、Intel: x86_64-apple-darwin）。
   bin/vole を ~/.local/bin に入れ、share/vole/rules を ~/.local/share/vole/rules にコピー。
   必要なら ~/.zshrc の PATH に ~/.local/bin を追加。
3. 上記がどちらも失敗しない限り、ソースからビルドしないでください。

完了後に `vole --version` を実行し、インストール先とバージョンを教えてください。
```

短く言うなら: `https://github.com/wukongnotnull/vole の公式手順で Vole をインストールして`

### 方法 2：Homebrew

```bash
brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
brew install vole
```

その後 `vole` を実行。brew が失敗したり版が合わない場合は、下のダウンロードを使ってください。

### 方法 3：ダウンロード

1. [最新 Release](https://github.com/wukongnotnull/vole/releases/latest) を開く
2. チップ用の圧縮ファイルを入手：
   - Apple Silicon（M シリーズ）：`…-aarch64-apple-darwin.tar.gz`
   - Intel：`…-x86_64-apple-darwin.tar.gz`
3. `bin/vole` を PATH 上の場所へ置き（例：`~/.local/bin`）、同梱の `share/vole/rules` も保持する

例（Apple Silicon / v2.17.0。ファイル名は Release ページに合わせてください）：

```bash
curl -LO https://github.com/wukongnotnull/vole/releases/download/v2.17.0/vole-2.17.0-aarch64-apple-darwin.tar.gz
tar xzf vole-2.17.0-aarch64-apple-darwin.tar.gz
mkdir -p ~/.local/bin ~/.local/share/vole
install -m 755 vole-2.17.0-aarch64-apple-darwin/bin/vole ~/.local/bin/vole
cp -R vole-2.17.0-aarch64-apple-darwin/share/vole/rules ~/.local/share/vole/
```

`vole: command not found` と出る場合は、`~/.zshrc` に次を追加して `source ~/.zshrc`：

```bash
export PATH="$HOME/.local/bin:$PATH"
```

---

## 使い方

### よく使うコマンド

```bash
# 日常
vole                           # 対話ホーム（いちばん簡単）
vole status                    # マシン状態
vole analyze                   # ディスクの使用元
vole clean                     # スキャン → 確認 → クリーン（既定はゴミ箱）
vole uninstall                 # 対話で App をアンインストール
vole optimize                  # システムメンテ
vole history                   # 過去の操作を確認

# プレビューのみ（まだ消さない）
vole clean --plan
vole uninstall --plan
vole optimize --plan
vole purge --plan
vole installer --plan

# 候補を見てから実行
vole clean --apply <plan.json>
vole uninstall --apply <plan.json>

# その他よく使う
vole touchid status            # sudo Touch ID 状態
vole update                    # 新しい版へ更新
vole remove --dry-run          # Vole 自身の削除プレビュー
vole --help
vole --version
```

既定はゴミ箱。完全削除は明示的に選んだときだけです。

---

### 全コマンド

サブコマンドなしで `vole` を実行すると対話ホームが開きます。

| コマンド | 別名 | 説明 |
|------|------|------|
| `vole` | — | 対話ホーム（Clean / Uninstall / Optimize / Analyze / Status） |
| `vole clean` | — | キャッシュと残骸をクリーン |
| `vole uninstall` | — | アプリと残骸をアンインストール |
| `vole optimize` | `optimise` | システム最適化・メンテ |
| `vole status` | — | ライブ健康パネル（CPU / メモリ / ディスク） |
| `vole analyze` | `analyse` | ディレクトリ容量分析（ホームから開始） |
| `vole history` | — | 操作履歴と削除ログ |
| `vole purge` | — | 古いプロジェクトのビルド成果物を掃除 |
| `vole installer` | — | インストーラを探して掃除 |
| `vole touchid` | — | sudo Touch ID 設定（`status` / `enable` / `disable`） |
| `vole update` | — | 自己更新（実行したときだけネット） |
| `vole remove` | — | Vole 自身をアンインストール |
| `vole completions` | `completion` | シェル補完を生成 |
| `vole help` | — | ヘルプ（`-h` / `--help` も可） |
| `vole --version` | `-V` | バージョン表示 |

## 安全について

```
あなた     ❯ vole clean → 候補を確認 → 承認

Vole      ❯ ✓ まず候補を表示し、すぐには削除しない
            ✓ 既定はゴミ箱（復元可能）
            ✓ 適用前に保護パスを再チェック
            ✓ 不確かならスキップ——削除範囲を静かに広げない
```

| 原則 | 意味 |
|------|------|
| **プレビューしてから実行** | ターミナルが確認を求める。`--plan` なら見るだけ |
| **既定で復元可能** | 個人ファイルはゴミ箱へ |
| **分かりやすい報告** | ゴミ箱と完全削除を区別 |
| **追跡可能** | `vole history` で確認 |

日常のクリーンはローカル完結。`vole update` を実行したときだけネット更新します。

---

## よくある質問

**Q：確認なしで消しませんか？**  
A：`clean` / `optimize` は確認を求めます（既定は No）。不安なら先に `--plan` で一覧だけ見てください。

**Q：間違って消した？**  
A：既定はゴミ箱です。ゴミ箱から復元できます。完全削除を選んだ場合は戻せません。

**Q：App はもう無い／まだある？**  
A：アンインストール後の残骸 → `vole clean`。App がまだある → `vole uninstall`。

**Q：`vole: command not found`？**  
A：インストール先が PATH に入っているか確認し（上の `~/.local/bin`）、ターミナルを開き直してください。

**Q：GUI が欲しい？**  
A：[Vole for macOS](https://github.com/wukongnotnull/vole-macos) をどうぞ——同じ掃除力のウィンドウアプリ（Apple Silicon / Intel）。

**Q：Mole との関係は？**  
A：規則と安全の考え方は [Mole](https://github.com/tw93/Mole) から着想しています。Vole は独立 OSS で Mole に所属しません。

---

## GUI が好きなら？

[Vole for macOS](https://github.com/wukongnotnull/vole-macos) は対応デスクトップ版：サイドバーで Clean / Uninstall / Optimize / Purge / Installer / Analyze / History / Status、フルディスクアクセス、一部システムパス用の任意 Root 特権ヘルパー。

最新デスクトップ版：[vole-macos Releases](https://github.com/wukongnotnull/vole-macos/releases/latest)（現在 **v0.2.0** Universal DMG）。

```text
同じ掃除力 · 同じ安全習慣 · ターミナルでもウィンドウでも
```

---

## About

**悟空非空也（Wukong）** — AI之道創業者、インディー開発者、クリエイター。

| プラットフォーム | リンク |
|------|------|
| 🌐 Web | [waytoai.cn](https://waytoai.cn) |
| 𝕏 Twitter | [悟空非空也](https://x.com/wukongnotnull) |
| 📺 Bilibili | [悟空非空也](https://space.bilibili.com/456634391) |
| ▶️ YouTube | [悟空非空也](https://www.youtube.com/@wukongnotnull) |
| 📕 小紅書 | [悟空非空也](https://www.xiaohongshu.com/user/profile/5ca89c2f000000001100952b) |
| 💬 WeChat | 「悟空非空也」で検索 |

---

## 謝辞

macOS クリーン体験を切り拓いてきた製品・OSS に感謝します。Vole は多くを学びました：

- [Mole](https://github.com/tw93/Mole) — OSS クリーナー。規則と安全の大きな着想源
- [CleanMyMac](https://macpaw.com/cleanmymac) — 洗練されたデスクトップ掃除 UX の参考
- [Tencent Lemon](https://lemon.qq.com/) — 中国語圏で親しまれるシステムクリーナーの参考

Vole は独立したオープンソースであり、上記との所属・商業関係はありません。

---

## ライセンス

Vole は [GPL-3.0](LICENSE) です。  
自社製品として派生させる場合は、混同を避けるため名称を変え、Mole / Vole を出典として明記してください。

---

<div align="center">

GPL-3.0 license © [悟空非空也](https://github.com/wukongnotnull)

</div>
