# JianyingPro generated cache

**日期**：2026-07-30  
**状态**：已落地  
**对照**：mole `clean_jianying_pro_generated_caches`；仿 FCP generated（`2026-07-cmdline-fcp-generated.md`）

## 规则

| id | 说明 |
|---|---|
| `jianyingpro-generated-cache` | `~/Movies/JianyingPro/User Data/Cache` → custom handler；仅白名单可再生成子目录；`VideoFusion-macOS` `-x` + 主可执行路径 `-f`（避开常驻 TrayHelper） |

### 选中（对齐 mole）

`recognize` / `frameThumbnail` / `audioWave` / `AlgorithmCache` / `ILASDKDB` / `RemuxCache` / `prerender` / `segmentPrerenderCache` / `MotionBlurCache` / `ttsTemp` / `tmp`

### 刻意排除

- `Projects`（草稿）及 Cache 以外 sibling
- `effect` / `music` / `image` / `importcache3` / `AigcMaterailCache` / `agencycache` 等草稿引用或导入副本

规则总数：**504**（三分支合入 main 后）
