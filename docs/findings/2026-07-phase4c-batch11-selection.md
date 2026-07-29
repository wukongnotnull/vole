# Phase 4c Batch 11 selection

Target: **40** rules (350 → **390**)  
**Actual: 40** (app-caches +20, user-devtools +20).

## Block A — macOS system caches (`user.sh`) (+20)

Recent items、DiagnosticReports、IdentityCaches、Siri Suggestions、Calendar、Address Book photos、Apple Configurator/Memoji/Music/CoreDevice/Neptune、Apple Media Services、helpd/GeoServices、duetexpertd/parsecd/python、Safari、amp.mediasevicesd。

## Block B — browser caches (`user.sh`) (+20)

Chrome/Brave/Arc User Data、Chromium、Edge、Firefox、Opera、Puppeteer、GoogleUpdater CRX。

## Excluded

- `~/Library/Caches/*` / `~/Library/Logs/*` 广域
- Rosetta `/Library/...`（系统路径）
- 重复 proposed_id 的 Arc/Chrome 变体（合并为 profile vs root 区分 id）

## Milestone

**390** 规则；`user.sh` 库存大幅消化；`ported` **382/513**（≈74%）。
