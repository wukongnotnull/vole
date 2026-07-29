# Phase 4c Batch 4 selection

Target: **40** rules  
**Actual: 40** (app-caches +18, user-devtools +22). Total enabled rules in tree ≈ **126**.

## Block A — `data/rules/app-caches.toml` (`all`, +18)

| proposed_id | label | path | strategy |
|---|---|---|---|
| teams-legacy-application-cache | Microsoft Teams legacy application cache | `~/Library/Application Support/Microsoft/Teams/Application Cache/*` | all |
| teams-legacy-code-cache | Microsoft Teams legacy code cache | `~/Library/Application Support/Microsoft/Teams/Code Cache/*` | all |
| teams-legacy-gpu-cache | Microsoft Teams legacy GPU cache | `~/Library/Application Support/Microsoft/Teams/GPUCache/*` | all |
| dingtalk-holmes-logs | DingTalk holmes logs | `~/Library/Application Support/iDingTalk/holmeslogs/*` | all |
| sketch-app-cache | Sketch app cache | `~/Library/Application Support/com.bohemiancoding.sketch3/cache/*` | all |
| davinci-resolve-cache | DaVinci Resolve cache | `~/Library/Caches/com.blackmagic-design.DaVinciResolve/*` | all |
| davinci-resolve-cacheclip | DaVinci Resolve CacheClip | `~/Movies/CacheClip/*` | all |
| premiere-pro-cache | Premiere Pro cache | `~/Library/Caches/com.adobe.PremierePro.*/*` | all |
| blender-cache | Blender cache | `~/Library/Caches/org.blenderfoundation.blender/*` | all |
| cinema-4d-cache | Cinema 4D cache | `~/Library/Caches/com.maxon.cinema4d/*` | all |
| autodesk-cache | Autodesk cache | `~/Library/Caches/com.autodesk.*/*` | all |
| sketchup-cache | SketchUp cache | `~/Library/Caches/com.sketchup.*/*` | all |
| spotify-cache | Spotify cache | `~/Library/Caches/com.spotify.client/*` | all |
| apple-music-cache | Apple Music cache | `~/Library/Caches/com.apple.Music` | all |
| apple-podcasts-cache | Apple Podcasts cache | `~/Library/Caches/com.apple.podcasts` | all |
| netnewswire-cache | NetNewsWire cache | `~/Library/Containers/com.ranchero.NetNewsWire-Evergreen/Data/Library/Caches/*` | all |
| mindnode-cache | MindNode cache | `~/Library/Containers/com.ideasoncanvas.mindnode/Data/Library/Caches/*` | all |
| flomo-cache | Flomo cache | `~/Library/Caches/com.flomoapp.mac/*` | all |

## Block B — `data/rules/user-devtools.toml` (`all`, +22)

| proposed_id | label | path | strategy |
|---|---|---|---|
| gem-package-cache | gem package cache | `~/.gem/ruby/*/cache/*.gem` | all |
| cpan-sources | CPAN source cache | `~/.cpan/sources/*` | all |
| aws-cli-cache | AWS CLI cache | `~/.aws/cli/cache/*` | all |
| gcloud-logs | Google Cloud logs | `~/.config/gcloud/logs/*` | all |
| azure-logs | Azure CLI logs | `~/.azure/logs/*` | all |
| typescript-cache | TypeScript cache | `~/.cache/typescript/*` | all |
| electron-cache | Electron cache | `~/.cache/electron/*` | all |
| node-gyp-cache | node-gyp cache | `~/.cache/node-gyp/*` | all |
| node-gyp-build-cache | node-gyp build cache | `~/.node-gyp/*` | all |
| turbo-cache | Turbo cache | `~/.turbo/cache/*` | all |
| vite-cache | Vite cache | `~/.vite/cache/*` | all |
| vite-global-cache | Vite global cache | `~/.cache/vite/*` | all |
| webpack-cache | Webpack cache | `~/.cache/webpack/*` | all |
| parcel-cache | Parcel cache | `~/.parcel-cache/*` | all |
| eslint-cache | ESLint cache | `~/.cache/eslint/*` | all |
| prettier-cache | Prettier cache | `~/.cache/prettier/*` | all |
| android-studio-cache | Android Studio cache | `~/Library/Caches/Google/AndroidStudio*/*` | all |
| swift-pm-cache | Swift package manager cache | `~/.cache/swift-package-manager/*` | all |
| swift-pm-library-cache | Swift package manager library cache | `~/Library/Caches/org.swift.swiftpm/*` | all |
| expo-go-cache | Expo Go cache | `~/.expo/expo-go/*` | all |
| expo-android-apk-cache | Expo Android APK cache | `~/.expo/android-apk-cache/*` | all |
| expo-ios-simulator-cache | Expo iOS simulator app cache | `~/.expo/ios-simulator-app-cache/*` | all |

## Excluded this batch

- Final Cut Pro generated cache (not_running guard)
- user.sh broad sweeps
- guard / custom / sudo

## Milestone

Crosses design **Top 100** target (126 total enabled rules).
