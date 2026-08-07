# GPU Metal caches 清理设计（system.sh 续刀）

- 日期：2026-08-07；已批准
- 依据：Mole `is_rebuildable_gpu_cache_dir` + `gpu_cache_dir_is_stale` + `safe_sudo_remove`；find maxdepth 8、`*/C/*`、名 `com.apple.metal|metalfe|gpuarchiver`；跳过 EDR
- 版本：**1.25.0**；规则 **530 → 531**

## 结论

- 一规则 `gpu-metal-caches`
- 形状：folders 下恰好 `*/*/C/*/NAME`（NAME ∈ metal/metalfe/gpuarchiver）
- stale：目录非 symlink，且目录树内无「mtime 落在最近 `MOLE_GPU_CACHE_AGE_DAYS`（1）天」的普通文件
- Privilege + apply 绑谓词 + EDR 跳过 + `sudo -n` 整目录删
- remap：`VOLE_TEST_SYSTEM_LIBRARY=$ROOT/Library` → `$ROOT/private/var/folders`

## 非目标

Install macOS*.app、`/Library/Updates`、交互提权；不打 tag
