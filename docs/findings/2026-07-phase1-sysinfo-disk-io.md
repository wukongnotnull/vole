# Phase 1 sysinfo 磁盘 I/O 实测

日期：2026-07-29

探针：`cargo run -p vole-sys --example disk_io_probe`

## 结论：**可用**（通过 sysinfo 内置 IOKit 路径）

`sysinfo` 0.39 在 macOS 上通过 IOKit `IOBlockStorageDriver` 统计提供每块磁盘的累计与增量读写字节（`Disk::usage()` → `DiskUsage`）。

## 本机样本（2026-07-29）

```
=== initial ===
Macintosh HD @ / | delta read=7447424843776 write=2371653632000 | total read=7447424843776 write=2371653632000
...
=== after 1s refresh ===
Macintosh HD @ / | delta read=5922816 write=233472 | total read=7447430766592 write=2371653865472
Macintosh HD @ /System/Volumes/Data | delta read=5865472 write=225280 | total read=7447430766592 write=2371653865472
```

- **累计字节**（`total_read_bytes` / `total_written_bytes`）：首次 `Disks::new_with_refreshed_list()` 即有非零值。
- **增量字节**（`read_bytes` / `written_bytes`）：间隔 1s 后 `disks.refresh(false)`，内置盘 delta 非零；外置卷/安装镜像 delta 为 0（符合预期）。

实现位于 `sysinfo` `unix/apple/macos/disk.rs` 的 `get_disk_io()`，无需 vole 单独写 IOKit 绑定即可做 **速率估算**（delta / Δt）。

## Phase 2 `status` 建议

- 磁盘占用、挂载点、文件系统：继续用 `Disks` 列表字段。
- 读/写速率：用两次 refresh 之间的 `Disk::usage()` delta 除以间隔；与 gopsutil 语义对齐需在 Phase 2 与 mole `status` 对照一次数值。
- 若需更细粒度或进程级 I/O：仍可用 `Process::disk_usage()`（`proc_pid_rusage`），与磁盘级指标分开。

**不需要** 为 Phase 2 单独加 IOKit crate；除非对照测试发现与 mole 数值偏差不可接受。
