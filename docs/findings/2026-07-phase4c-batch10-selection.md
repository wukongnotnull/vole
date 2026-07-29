# Phase 4c Batch 10 selection

Target: **40** rules (310 → **350**)  
**Actual: 40** (app-caches +20, user-devtools +20).

## Block A — notes / remote / proxy (+20)

WeType、mihomo-party、Stash、Notion/Logseq/Bear/Evernote/Yinxiang、Alfred、Unarchiver、TeamViewer/AnyDesk/ToDesk/Sunlogin。

## Block B — dev pkg caches + macOS system container caches (+20)

CocoaPods/Flutter/Dart Pub/OPAM/Homebrew downloads；Messages preview、Photo analysis、QuickLook、App Store/Stocks 等 Apple container cache/tmp。

## Excluded

- Claude pending-uploads（bundle 保护，非 explicit cache）
- `user.sh` 广域 `~/Library/Caches/*` / `~/Library/Logs/*`

## Milestone

**350** 规则；`ported` **343/513**（≈67%）。
