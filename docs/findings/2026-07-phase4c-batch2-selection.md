# Phase 4c Batch 2 selection

Target: 40 rules  
**Actual: 40** (app-caches 35 + user-devtools 5). Total enabled rules in tree ≈ 46 including prior AI/Codex/example.

## Block A — `data/rules/app-caches.toml` (`all`)

| proposed_id | label | path | strategy |
|---|---|---|---|
| xcode-documentation-index | Xcode documentation index | `~/Library/Developer/Xcode/DocumentationIndex/*` | all |
| vs-code-extension-cache | VS Code extension cache | `~/Library/Application Support/Code/CachedExtensions/*` | all |
| ios-device-logs | iOS device logs | `~/Library/Developer/Xcode/iOS Device Logs/*` | all |
| vs-code-cache | VS Code cache | `~/Library/Application Support/Code/Cache/*` | all |
| vs-code-data-cache | VS Code data cache | `~/Library/Application Support/Code/CachedData/*` | all |
| zed-cache | Zed cache | `~/Library/Caches/Zed/*` | all |
| xcode-build-products | Xcode build products | `~/Library/Developer/Xcode/Products/*` | all |
| zed-logs | Zed logs | `~/Library/Logs/Zed/*` | all |
| xcode-cache | Xcode cache | `~/Library/Caches/com.apple.dt.Xcode/*` | all |
| simulator-temp-files | Simulator temp files | `~/Library/Developer/CoreSimulator/Devices/*/data/tmp/*` | all |
| vs-code-webview-cache | VS Code webview cache | `~/Library/Application Support/Code/WebStorage/*/CacheStorage/*` | all |
| coresimulator-logs | CoreSimulator logs | `~/Library/Logs/CoreSimulator/*` | all |
| xcode-documentation-cache | Xcode documentation cache | `~/Library/Developer/Xcode/DocumentationCache/*` | all |
| vs-code-logs | VS Code logs | `~/Library/Application Support/Code/logs/*` | all |
| watchos-device-logs | watchOS device logs | `~/Library/Developer/Xcode/watchOS Device Logs/*` | all |
| zed-npm-cache | Zed npm cache | `~/Library/Application Support/Zed/node/node-v*/cache/*` | all |
| simulator-cache | Simulator cache | `~/Library/Developer/CoreSimulator/Caches/*` | all |
| sublime-text-cache | Sublime Text cache | `~/Library/Caches/com.sublimetext.*/*` | all |
| codebuddy-extension-cache | CodeBuddy Extension cache | `~/Library/Application Support/CodeBuddyExtension/Cache/*` | all |
| codebuddy-extension-logs | CodeBuddy Extension logs | `~/Library/Application Support/CodeBuddyExtension/logs/*` | all |
| codebuddy-cn-cache | CodeBuddy CN cache | `~/Library/Application Support/CodeBuddy CN/Cache/*` | all |
| codebuddy-cn-cached-data | CodeBuddy CN cached data | `~/Library/Application Support/CodeBuddy CN/CachedData/*` | all |
| codebuddy-cn-extension-cache | CodeBuddy CN extension cache | `~/Library/Application Support/CodeBuddy CN/CachedExtensionVSIXs/*` | all |
| codebuddy-cn-code-cache | CodeBuddy CN code cache | `~/Library/Application Support/CodeBuddy CN/Code Cache/*` | all |
| codebuddy-cn-gpu-cache | CodeBuddy CN GPU cache | `~/Library/Application Support/CodeBuddy CN/GPUCache/*` | all |
| codebuddy-cn-dawn-cache | CodeBuddy CN Dawn cache | `~/Library/Application Support/CodeBuddy CN/DawnGraphiteCache/*` | all |
| codebuddy-cn-webgpu-cache | CodeBuddy CN WebGPU cache | `~/Library/Application Support/CodeBuddy CN/DawnWebGPUCache/*` | all |
| codebuddy-cn-logs | CodeBuddy CN logs | `~/Library/Application Support/CodeBuddy CN/logs/*` | all |
| discord-cache | Discord cache | `~/Library/Application Support/discord/Cache/*` | all |
| legcord-cache | Legcord cache | `~/Library/Application Support/legcord/Cache/*` | all |
| slack-cache | Slack cache | `~/Library/Application Support/Slack/Cache/*` | all |
| zoom-cache | Zoom cache | `~/Library/Caches/us.zoom.xos/*` | all |
| wechat-cache | WeChat cache | `~/Library/Caches/com.tencent.xinWeChat/*` | all |
| telegram-cache | Telegram cache | `~/Library/Caches/ru.keepcoder.Telegram/*` | all |
| microsoft-teams-cache | Microsoft Teams cache | `~/Library/Caches/com.microsoft.teams2/*` | all |

## Block B — `data/rules/user-devtools.toml`

| proposed_id | label | path | strategy | notes |
|---|---|---|---|---|
| npm-cacache | npm cache directory | `~/.npm/_cacache/*` | all |  |
| npm-npx-cache | npm npx cache | `~/.npm/_npx/*` | all |  |
| npm-prebuilds | npm prebuilds | `~/.npm/_prebuilds/*` | all |  |
| tnpm-logs | tnpm logs | `~/.tnpm/_logs/*` | all |  |
| npm-logs-keep-newest | npm logs | `~/.npm/_logs/*` | keep_newest_by_mtime keep=5 | keep newest 5 by mtime (mole deletes all logs; intentional vole policy for Batch2) |

## Excluded this batch

- sudo / system privilege rules
- `$var` path custom loops
- brew cleanup (`brew` subprocess)
- new `custom` handlers (quota: 0 this batch)


## Custom ratio note

Batch 2 added **0** custom handlers. Pre-existing custom rules: 3. Total rules: 46 → custom ratio ≈ 6.5% (slightly above 5% hard cap). Next batch should prefer `all`/`keep_*` only until ratio ≤ 5%.
