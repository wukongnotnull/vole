//! Optimize 前置只读诊断（对齐 Mole `bin/optimize.sh` `show_system_health` 与
//! `lib/optimize/diagnostics.sh` `run_optimize_diagnostics`）。
//!
//! - 系统概要：内存已用/总量、`$HOME` 所在卷已用/总量、运行天数。
//! - 性能诊断：两次 `ps -Aceo pcpu=,command=` 采样，按进程族（CloudShell / syspolicyd /
//!   WindowServer / Spotlight / CoreSimulator 磁盘镜像）累计 CPU；两次都超过阈值即判为持续高占用。
//! - syspolicyd 持续高占用时才查 `spctl --status` 与 `hdiutil info`，列出可卸载的用户挂载镜像
//!   （系统管理的镜像、受保护路径、白名单路径一律不列）。
//!
//! 本模块只读；卸载镜像只在 CLI 的 TTY 确认后经 [`detach_mounted_images`] 执行，
//! `VOLE_TEST_NO_AUTH` 下永不真跑 `hdiutil detach`。

use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::delete::test_no_auth;
use crate::protection::{should_protect_path, ProtectionCatalog, ProtectionMode};
use crate::whitelist;

pub const DEFAULT_CPU_THRESHOLD: f64 = 25.0;
pub const DEFAULT_SAMPLE_DELAY_SECS: f64 = 1.0;
const HDIUTIL_INFO_TIMEOUT_SECS: u64 = 8;
const HDIUTIL_DETACH_TIMEOUT_SECS: u64 = 15;
const SHORT_PROBE_TIMEOUT_SECS: u64 = 5;

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

fn env_first(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|n| std::env::var(n).ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env_non_negative_f64(names: &[&str], default: f64) -> f64 {
    env_first(names)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(default)
}

/// `VOLE_OPTIMIZE_DIAG_CPU_THRESHOLD`（兼容 `MOLE_`），默认 25（百分比）。
pub fn cpu_threshold() -> f64 {
    env_non_negative_f64(
        &[
            "VOLE_OPTIMIZE_DIAG_CPU_THRESHOLD",
            "MOLE_OPTIMIZE_DIAG_CPU_THRESHOLD",
        ],
        DEFAULT_CPU_THRESHOLD,
    )
}

/// `VOLE_OPTIMIZE_DIAG_SAMPLE_DELAY`（兼容 `MOLE_`），默认 1 秒。
pub fn sample_delay() -> Duration {
    Duration::from_secs_f64(env_non_negative_f64(
        &[
            "VOLE_OPTIMIZE_DIAG_SAMPLE_DELAY",
            "MOLE_OPTIMIZE_DIAG_SAMPLE_DELAY",
        ],
        DEFAULT_SAMPLE_DELAY_SECS,
    ))
}

// ---------------------------------------------------------------------------
// 系统概要
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct SystemSummary {
    pub memory_used_gb: f64,
    pub memory_total_gb: f64,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    pub uptime_days: f64,
}

/// `vm_stat` 输出 → 已用字节（active + wired + compressor）× 页大小。
/// 页大小优先取 `vm_stat` 表头声明（Apple Silicon 为 16384），否则用 `fallback_page_size`。
pub fn parse_vm_stat_used_bytes(vm_stat: &str, fallback_page_size: u64) -> u64 {
    let page_size = vm_stat
        .lines()
        .find_map(|l| {
            let rest = l.split("page size of ").nth(1)?;
            rest.split_whitespace().next()?.parse::<u64>().ok()
        })
        .filter(|&n| n > 0)
        .unwrap_or(if fallback_page_size > 0 {
            fallback_page_size
        } else {
            4096
        });
    let pages = |label: &str| -> u64 {
        vm_stat
            .lines()
            .find(|l| l.trim_start().starts_with(label))
            .and_then(|l| l.split_whitespace().last())
            .map(|v| v.trim_end_matches('.'))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let used_pages = pages("Pages active:")
        + pages("Pages wired down:")
        + pages("Pages occupied by compressor:");
    used_pages.saturating_mul(page_size)
}

/// `df -k <path>` 输出 → (used_kb, total_kb)，取最后一行。
pub fn parse_df_k(df: &str) -> Option<(u64, u64)> {
    let line = df.lines().rev().find(|l| !l.trim().is_empty())?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    // 设备名可能含空格：从末尾往前数不稳，按 Mole 取第 2、3 列。
    let total = cols.get(1)?.parse::<u64>().ok()?;
    let used = cols.get(2)?.parse::<u64>().ok()?;
    Some((used, total))
}

/// `sysctl -n kern.boottime` 输出（`{ sec = 1700000000, usec = 0 } ...`）→ 开机 epoch 秒。
pub fn parse_boottime_secs(out: &str) -> Option<u64> {
    let rest = out.split("sec = ").nth(1)?;
    rest.split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse::<u64>()
        .ok()
}

/// `System  5/16 GB RAM | 200/494 GB Disk | Uptime 3d`（四舍五入，对齐 Mole）。
pub fn format_system_summary(s: &SystemSummary) -> String {
    format!(
        "System  {:.0}/{:.0} GB RAM | {:.0}/{:.0} GB Disk | Uptime {:.0}d",
        s.memory_used_gb, s.memory_total_gb, s.disk_used_gb, s.disk_total_gb, s.uptime_days
    )
}

/// 采集系统概要；内存总量取不到时返回 `None`（调用方不打印概要行）。
pub fn collect_system_summary(home: &Path) -> Option<SystemSummary> {
    let total_bytes = command_stdout("sysctl", &["-n", "hw.memsize"])
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&n| n > 0)?;
    let page_size = command_stdout("sysctl", &["-n", "hw.pagesize"])
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(4096);
    let used_bytes = command_stdout("vm_stat", &[])
        .map(|s| parse_vm_stat_used_bytes(&s, page_size))
        .unwrap_or(0);
    let home_arg = home.to_string_lossy();
    let (disk_used_kb, disk_total_kb) = command_stdout("df", &["-k", home_arg.as_ref()])
        .and_then(|s| parse_df_k(&s))
        .unwrap_or((0, 0));
    let uptime_days = command_stdout("sysctl", &["-n", "kern.boottime"])
        .and_then(|s| parse_boottime_secs(&s))
        .and_then(|boot| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs();
            now.checked_sub(boot)
        })
        .map(|secs| secs as f64 / 86_400.0)
        .unwrap_or(0.0);
    Some(SystemSummary {
        memory_used_gb: used_bytes as f64 / GIB,
        memory_total_gb: total_bytes as f64 / GIB,
        disk_used_gb: disk_used_kb as f64 / (1024.0 * 1024.0),
        disk_total_gb: disk_total_kb as f64 / (1024.0 * 1024.0),
        uptime_days,
    })
}

// ---------------------------------------------------------------------------
// 性能诊断
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFamily {
    CloudShell,
    Syspolicyd,
    WindowServer,
    Spotlight,
    CoreSimDiskImages,
}

/// 与 Mole `families` 列表同序；主瓶颈并列时取先出现者。
pub const CPU_FAMILIES: [CpuFamily; 5] = [
    CpuFamily::CloudShell,
    CpuFamily::Syspolicyd,
    CpuFamily::WindowServer,
    CpuFamily::Spotlight,
    CpuFamily::CoreSimDiskImages,
];

impl CpuFamily {
    pub fn label(self) -> &'static str {
        match self {
            CpuFamily::CloudShell => "CloudShell / AliEntSafe",
            CpuFamily::Syspolicyd => "syspolicyd",
            CpuFamily::WindowServer => "WindowServer",
            CpuFamily::Spotlight => "Spotlight indexing",
            CpuFamily::CoreSimDiskImages => "CoreSimulator disk images",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            CpuFamily::CloudShell => "External enterprise agent pressure detected. Vole will not terminate enterprise security processes; restart or policy checks must happen outside Vole.",
            CpuFamily::Syspolicyd => "Gatekeeper and code-signature assessment activity is elevated.",
            CpuFamily::WindowServer => "Desktop composition is busy. When another family is higher, treat this as a likely symptom rather than the root cause.",
            CpuFamily::Spotlight => "Metadata indexing or import work is consuming CPU.",
            CpuFamily::CoreSimDiskImages => "Simulator runtime disk-image services are active.",
        }
    }

    fn index(self) -> usize {
        CPU_FAMILIES.iter().position(|f| *f == self).unwrap_or(0)
    }
}

/// 进程命令行 → 进程族（大小写不敏感；规则同 Mole `classify`）。
pub fn classify_command(cmd: &str) -> Option<CpuFamily> {
    let lower = cmd.to_ascii_lowercase();
    if lower.contains("cloudshell") || lower.contains("alientsafe") || lower.contains("aliedr") {
        return Some(CpuFamily::CloudShell);
    }
    if executable_is(&lower, "syspolicyd") {
        return Some(CpuFamily::Syspolicyd);
    }
    if executable_is(&lower, "windowserver") {
        return Some(CpuFamily::WindowServer);
    }
    if executable_is(&lower, "mds")
        || lower.contains("mdworker")
        || lower.contains("mds_stores")
        || lower.contains("mdbulkimport")
    {
        return Some(CpuFamily::Spotlight);
    }
    if lower.contains("diskimagesiod") || lower.contains("simdiskimaged") {
        return Some(CpuFamily::CoreSimDiskImages);
    }
    None
}

/// `(^|/)name([[:space:]]|$)`：词元以 name 结尾且前面是行首或 `/`，后面是空白或行尾。
fn executable_is(lower: &str, name: &str) -> bool {
    let bytes = lower.as_bytes();
    let mut from = 0;
    while let Some(off) = lower[from..].find(name) {
        let start = from + off;
        let end = start + name.len();
        let before_ok = start == 0 || bytes[start - 1] == b'/';
        let after_ok = end == bytes.len() || bytes[end].is_ascii_whitespace();
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// `ps -Aceo pcpu=,command=` 输出 → 各进程族 CPU 合计（下标同 [`CPU_FAMILIES`]）。
pub fn family_totals(ps: &str) -> [f64; 5] {
    let mut sums = [0.0f64; 5];
    for line in ps.lines() {
        let line = line.trim_start();
        let Some((cpu, cmd)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(cpu) = cpu.trim().parse::<f64>() else {
            continue;
        };
        if let Some(family) = classify_command(cmd.trim()) {
            sums[family.index()] += cpu;
        }
    }
    // 对齐 Mole `printf "%.1f"`。
    sums.map(|v| (v * 10.0).round() / 10.0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SustainedFamily {
    pub family: CpuFamily,
    pub avg_cpu: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountedImage {
    pub image_path: String,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PerformanceDiagnosis {
    /// 两次采样都 ≥ 阈值的进程族（按 [`CPU_FAMILIES`] 顺序）。
    pub sustained: Vec<SustainedFamily>,
    /// 平均 CPU 最高的持续进程族。
    pub primary: Option<SustainedFamily>,
    /// 仅 syspolicyd 持续高占用时探测。
    pub gatekeeper_status: Option<String>,
    /// 只挂载了系统管理的 CoreSimulator 镜像（仅提示，不作为卸载目标）。
    pub coresim_only: bool,
    /// 可提议卸载的用户挂载镜像。
    pub detach_candidates: Vec<MountedImage>,
}

/// 只读探针；测试注入 Fake。
pub trait DiagnosticsDeps {
    /// 第 `index`（1 或 2）次 `ps` 采样。
    fn ps_sample(&self, index: u8) -> String;
    /// 两次采样之间的等待；两次采样都被覆盖时不调用。
    fn sleep_between_samples(&self);
    /// 两次采样是否都来自覆盖值（无需等待）。
    fn samples_overridden(&self) -> bool {
        false
    }
    fn spctl_status(&self) -> String;
    fn hdiutil_info(&self) -> String;
}

pub struct LiveDiagnosticsDeps;

impl DiagnosticsDeps for LiveDiagnosticsDeps {
    fn ps_sample(&self, index: u8) -> String {
        let names: &[&str] = match index {
            1 => &["VOLE_OPTIMIZE_PS_SAMPLE_1", "MOLE_OPTIMIZE_PS_SAMPLE_1"],
            _ => &["VOLE_OPTIMIZE_PS_SAMPLE_2", "MOLE_OPTIMIZE_PS_SAMPLE_2"],
        };
        if let Some(v) = env_first(names) {
            return v;
        }
        command_stdout("ps", &["-Aceo", "pcpu=,command="]).unwrap_or_default()
    }

    fn sleep_between_samples(&self) {
        thread::sleep(sample_delay());
    }

    fn samples_overridden(&self) -> bool {
        env_first(&["VOLE_OPTIMIZE_PS_SAMPLE_1", "MOLE_OPTIMIZE_PS_SAMPLE_1"]).is_some()
            && env_first(&["VOLE_OPTIMIZE_PS_SAMPLE_2", "MOLE_OPTIMIZE_PS_SAMPLE_2"]).is_some()
    }

    fn spctl_status(&self) -> String {
        if let Some(v) = env_first(&["VOLE_OPTIMIZE_SPCTL_STATUS", "MOLE_OPTIMIZE_SPCTL_STATUS"]) {
            return v;
        }
        run_with_timeout("spctl", &["--status"], SHORT_PROBE_TIMEOUT_SECS)
            .map(|(_, out)| out.trim().to_string())
            .unwrap_or_default()
    }

    fn hdiutil_info(&self) -> String {
        if let Some(v) = env_first(&["VOLE_OPTIMIZE_HDIUTIL_INFO", "MOLE_OPTIMIZE_HDIUTIL_INFO"]) {
            return v;
        }
        run_with_timeout("hdiutil", &["info"], HDIUTIL_INFO_TIMEOUT_SECS)
            .map(|(_, out)| out)
            .unwrap_or_default()
    }
}

/// `hdiutil info` 输出 → (image-path, mount) 对。只认 `/dev/disk*` 行上的挂载点。
pub fn parse_image_mount_pairs(info: &str) -> Vec<MountedImage> {
    let mut out = Vec::new();
    let mut image = String::new();
    let mut mounts: Vec<String> = Vec::new();
    let flush = |image: &str, mounts: &mut Vec<String>, out: &mut Vec<MountedImage>| {
        if !image.is_empty() {
            for m in mounts.iter() {
                out.push(MountedImage {
                    image_path: image.to_string(),
                    mount_path: m.clone(),
                });
            }
        }
        mounts.clear();
    };
    for line in info.lines() {
        if !line.is_empty() && line.chars().all(|c| c == '=') {
            flush(&image, &mut mounts, &mut out);
            image.clear();
            continue;
        }
        if let Some(value) = line
            .strip_prefix("image-path")
            .and_then(|rest| rest.trim_start().strip_prefix(':'))
        {
            image = value.trim().to_string();
            continue;
        }
        if let Some(mount) = extract_mount(line) {
            mounts.push(mount);
        }
    }
    flush(&image, &mut mounts, &mut out);
    out
}

/// Mole `sub(/^.*[[:space:]]\//, "/")`：取最后一个「空白 + /」起的部分。
fn extract_mount(line: &str) -> Option<String> {
    if !line.starts_with("/dev/disk") {
        return None;
    }
    let bytes = line.as_bytes();
    let mut idx = None;
    for i in 1..bytes.len() {
        if bytes[i] == b'/' && bytes[i - 1].is_ascii_whitespace() {
            idx = Some(i);
        }
    }
    let mount = line[idx?..].trim_end();
    if mount.starts_with('/') {
        Some(mount.to_string())
    } else {
        None
    }
}

pub fn is_system_managed_mount(image_path: &str, mount_path: &str) -> bool {
    const IMAGE_PREFIXES: &[&str] = &[
        "/System/",
        "/Library/Apple/",
        "/private/var/run/com.apple.security.cryptexd/",
    ];
    const MOUNT_PREFIXES: &[&str] = &[
        "/Library/Developer/CoreSimulator/Volumes/",
        "/private/var/run/com.apple.security.cryptexd/",
    ];
    IMAGE_PREFIXES.iter().any(|p| image_path.starts_with(p))
        || MOUNT_PREFIXES.iter().any(|p| mount_path.starts_with(p))
}

fn is_detach_candidate(
    img: &MountedImage,
    catalog: &ProtectionCatalog,
    whitelist_patterns: &[String],
) -> bool {
    if is_system_managed_mount(&img.image_path, &img.mount_path) {
        return false;
    }
    if !img.mount_path.starts_with("/Volumes/") {
        return false;
    }
    const IMAGE_EXTS: &[&str] = &[
        ".dmg",
        ".iso",
        ".img",
        ".cdr",
        ".sparseimage",
        ".sparsebundle",
    ];
    if !IMAGE_EXTS.iter().any(|ext| img.image_path.ends_with(ext)) {
        return false;
    }
    let guarded = |p: &str| {
        should_protect_path(p, catalog, ProtectionMode::Cleanup)
            || whitelist::is_match(Path::new(p), whitelist_patterns)
    };
    if guarded(&img.mount_path) {
        return false;
    }
    if !img.image_path.is_empty() && guarded(&img.image_path) {
        return false;
    }
    true
}

/// 跑完整诊断（两次采样 → 持续族 → syspolicyd 时查镜像）。
pub fn run_performance_diagnosis(
    deps: &dyn DiagnosticsDeps,
    catalog: &ProtectionCatalog,
    whitelist_patterns: &[String],
    threshold: f64,
) -> PerformanceDiagnosis {
    let sample1 = deps.ps_sample(1);
    if !deps.samples_overridden() {
        deps.sleep_between_samples();
    }
    let sample2 = deps.ps_sample(2);
    let totals1 = family_totals(&sample1);
    let totals2 = family_totals(&sample2);

    let mut diag = PerformanceDiagnosis::default();
    for family in CPU_FAMILIES {
        let (c1, c2) = (totals1[family.index()], totals2[family.index()]);
        if c1 >= threshold && c2 >= threshold {
            let avg = (((c1 + c2) / 2.0) * 10.0).round() / 10.0;
            let entry = SustainedFamily {
                family,
                avg_cpu: avg,
            };
            if diag.primary.as_ref().is_none_or(|p| avg > p.avg_cpu) {
                diag.primary = Some(entry.clone());
            }
            diag.sustained.push(entry);
        }
    }

    // 健康系统上挂着 DMG 不算诊断结论：只有 syspolicyd 持续高占用才查镜像。
    if diag
        .sustained
        .iter()
        .any(|s| s.family == CpuFamily::Syspolicyd)
    {
        let status = deps.spctl_status();
        if !status.trim().is_empty() {
            diag.gatekeeper_status = Some(status.trim().to_string());
        }
        let pairs = parse_image_mount_pairs(&deps.hdiutil_info());
        diag.detach_candidates = pairs
            .iter()
            .filter(|p| is_detach_candidate(p, catalog, whitelist_patterns))
            .cloned()
            .collect();
        let managed = pairs
            .iter()
            .filter(|p| is_system_managed_mount(&p.image_path, &p.mount_path))
            .count();
        let coresim = pairs
            .iter()
            .filter(|p| {
                p.mount_path
                    .starts_with("/Library/Developer/CoreSimulator/Volumes/")
            })
            .count();
        diag.coresim_only = managed > 0 && managed == coresim && diag.detach_candidates.is_empty();
    }
    diag
}

/// 人读诊断文案（不含卸载询问；CLI 自行决定是否提议卸载）。
pub fn format_diagnosis_lines(diag: &PerformanceDiagnosis) -> Vec<String> {
    let mut lines = vec!["PERFORMANCE DIAGNOSIS".to_string()];
    match &diag.primary {
        None => lines.push("  ✓ No sustained high-CPU bottleneck detected".into()),
        Some(primary) => {
            lines.push(format!(
                "  ! Likely bottleneck: {} (~{:.1}% CPU sustained)",
                primary.family.label(),
                primary.avg_cpu
            ));
            lines.push(format!("    {}", primary.family.note()));
            let others: Vec<_> = diag
                .sustained
                .iter()
                .filter(|s| s.family != primary.family)
                .collect();
            if !others.is_empty() {
                lines.push("  Additional sustained pressure:".into());
                for s in others {
                    lines.push(format!("    {} ~{:.1}%", s.family.label(), s.avg_cpu));
                }
            }
        }
    }
    if let Some(status) = &diag.gatekeeper_status {
        lines.push(format!("  Gatekeeper status: {status}"));
    }
    if diag.coresim_only {
        lines.push(
            "  Only system-managed CoreSimulator images are mounted, informational only, not a detach target"
                .into(),
        );
    }
    if !diag.detach_candidates.is_empty() {
        lines.push(if diag.detach_candidates.len() == 1 {
            "  Mounted image adds assessment overhead:".into()
        } else {
            "  Mounted images add assessment overhead:".into()
        });
        for img in &diag.detach_candidates {
            let name = img
                .image_path
                .rsplit('/')
                .next()
                .unwrap_or(img.image_path.as_str());
            lines.push(format!("    {name} → {}", img.mount_path));
        }
    }
    lines
}

/// 卸载镜像的执行器；测试注入 Fake。
pub trait ImageDetacher {
    fn detach(&self, mount_path: &str) -> bool;
}

pub struct LiveImageDetacher;

impl ImageDetacher for LiveImageDetacher {
    fn detach(&self, mount_path: &str) -> bool {
        if test_no_auth() {
            return false;
        }
        run_with_timeout(
            "hdiutil",
            &["detach", mount_path],
            HDIUTIL_DETACH_TIMEOUT_SECS,
        )
        .is_some_and(|(ok, _)| ok)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetachOutcome {
    pub detached: Vec<String>,
    pub failed: Vec<String>,
}

/// 逐个 `hdiutil detach`；调用方须已取得用户确认。
pub fn detach_mounted_images(
    detacher: &dyn ImageDetacher,
    candidates: &[MountedImage],
) -> DetachOutcome {
    let mut out = DetachOutcome::default();
    for img in candidates {
        if detacher.detach(&img.mount_path) {
            out.detached.push(img.mount_path.clone());
        } else {
            out.failed.push(img.mount_path.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 进程工具
// ---------------------------------------------------------------------------

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    run_with_timeout(program, args, SHORT_PROBE_TIMEOUT_SECS)
        .filter(|(ok, _)| *ok)
        .map(|(_, out)| out)
}

/// 运行命令并在超时后 kill；返回 (退出成功, stdout)。
fn run_with_timeout(program: &str, args: &[&str], timeout_secs: u64) -> Option<(bool, String)> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child.wait_with_output().ok()?;
                return Some((
                    output.status.success(),
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                ));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct FakeDeps {
        s1: &'static str,
        s2: &'static str,
        spctl: &'static str,
        hdiutil: &'static str,
        slept: Cell<bool>,
        hdiutil_called: Cell<bool>,
    }

    impl FakeDeps {
        fn new(s1: &'static str, s2: &'static str) -> Self {
            Self {
                s1,
                s2,
                spctl: "assessments enabled",
                hdiutil: "",
                slept: Cell::new(false),
                hdiutil_called: Cell::new(false),
            }
        }
    }

    impl DiagnosticsDeps for FakeDeps {
        fn ps_sample(&self, index: u8) -> String {
            if index == 1 { self.s1 } else { self.s2 }.to_string()
        }
        fn sleep_between_samples(&self) {
            self.slept.set(true);
        }
        fn spctl_status(&self) -> String {
            self.spctl.to_string()
        }
        fn hdiutil_info(&self) -> String {
            self.hdiutil_called.set(true);
            self.hdiutil.to_string()
        }
    }

    const HDIUTIL_INFO: &str = "framework       : 671\n\
driver          : 10.0\n\
================================================\n\
image-path      : /Users/me/Downloads/Tool.dmg\n\
image-alias     : /Users/me/Downloads/Tool.dmg\n\
shadow-path     : <none>\n\
/dev/disk4          \tGUID_partition_scheme          \t\n\
/dev/disk4s1        \tApple_HFS                      \t/Volumes/Tool Installer\n\
================================================\n\
image-path      : /Library/Developer/CoreSimulator/Images/iOS.dmg\n\
/dev/disk5s1        \tApple_APFS                     \t/Library/Developer/CoreSimulator/Volumes/iOS_22A\n\
================================================\n\
image-path      : /Users/me/notes.txt\n\
/dev/disk6s1        \tApple_HFS                      \t/Volumes/Notes\n";

    #[test]
    fn vm_stat_uses_declared_page_size() {
        let vm = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
Pages free:                               10000.\n\
Pages active:                            100000.\n\
Pages wired down:                         50000.\n\
Pages occupied by compressor:             10000.\n";
        assert_eq!(parse_vm_stat_used_bytes(vm, 4096), 160_000 * 16_384);
    }

    #[test]
    fn vm_stat_falls_back_to_given_page_size() {
        let vm = "Pages active: 10.\nPages wired down: 5.\n";
        assert_eq!(parse_vm_stat_used_bytes(vm, 4096), 15 * 4096);
        assert_eq!(parse_vm_stat_used_bytes("", 4096), 0);
    }

    #[test]
    fn df_and_boottime_parse() {
        let df = "Filesystem   1024-blocks      Used Available Capacity iused ifree %iused  Mounted on\n\
/dev/disk3s5   494384795 209715200 200000000    52%  1 2 0%   /System/Volumes/Data\n";
        assert_eq!(parse_df_k(df), Some((209_715_200, 494_384_795)));
        assert_eq!(parse_df_k(""), None);
        assert_eq!(
            parse_boottime_secs("{ sec = 1760000000, usec = 123 } Thu Oct  9 2025"),
            Some(1_760_000_000)
        );
        assert_eq!(parse_boottime_secs("garbage"), None);
    }

    #[test]
    fn summary_line_rounds_like_mole() {
        let s = SystemSummary {
            memory_used_gb: 5.7,
            memory_total_gb: 16.0,
            disk_used_gb: 200.4,
            disk_total_gb: 494.6,
            uptime_days: 3.2,
        };
        assert_eq!(
            format_system_summary(&s),
            "System  6/16 GB RAM | 200/495 GB Disk | Uptime 3d"
        );
    }

    #[test]
    fn classify_matches_mole_families() {
        assert_eq!(
            classify_command("/usr/libexec/syspolicyd"),
            Some(CpuFamily::Syspolicyd)
        );
        assert_eq!(classify_command("syspolicyd"), Some(CpuFamily::Syspolicyd));
        assert_eq!(classify_command("syspolicydx"), None);
        assert_eq!(
            classify_command("WindowServer -daemon"),
            Some(CpuFamily::WindowServer)
        );
        assert_eq!(classify_command("mds"), Some(CpuFamily::Spotlight));
        assert_eq!(classify_command("mds_stores"), Some(CpuFamily::Spotlight));
        assert_eq!(
            classify_command("mdworker_shared"),
            Some(CpuFamily::Spotlight)
        );
        assert_eq!(classify_command("cmds"), None);
        assert_eq!(classify_command("AliEntSafe"), Some(CpuFamily::CloudShell));
        assert_eq!(
            classify_command("diskimagesiod"),
            Some(CpuFamily::CoreSimDiskImages)
        );
        assert_eq!(classify_command("Safari"), None);
    }

    #[test]
    fn family_totals_sum_per_family() {
        let ps =
            " 20.0 mdworker_shared\n 15.5 mds_stores\n 3.0 Safari\n 30.0 syspolicyd\nbad line\n";
        let t = family_totals(ps);
        assert_eq!(t[CpuFamily::Spotlight.index()], 35.5);
        assert_eq!(t[CpuFamily::Syspolicyd.index()], 30.0);
        assert_eq!(t[CpuFamily::WindowServer.index()], 0.0);
    }

    #[test]
    fn healthy_system_reports_no_bottleneck_and_skips_image_probe() {
        let deps = FakeDeps::new(" 2.0 WindowServer\n", " 3.0 WindowServer\n");
        let diag = run_performance_diagnosis(&deps, &ProtectionCatalog::embedded(), &[], 25.0);
        assert!(diag.primary.is_none());
        assert!(deps.slept.get());
        assert!(!deps.hdiutil_called.get());
        let lines = format_diagnosis_lines(&diag);
        assert!(lines
            .iter()
            .any(|l| l.contains("No sustained high-CPU bottleneck detected")));
    }

    #[test]
    fn single_spike_is_not_sustained() {
        let deps = FakeDeps::new(" 90.0 mds_stores\n", " 1.0 mds_stores\n");
        let diag = run_performance_diagnosis(&deps, &ProtectionCatalog::embedded(), &[], 25.0);
        assert!(diag.sustained.is_empty());
    }

    #[test]
    fn primary_is_highest_sustained_family() {
        let deps = FakeDeps::new(
            " 40.0 WindowServer\n 60.0 mds_stores\n",
            " 30.0 WindowServer\n 80.0 mds_stores\n",
        );
        let diag = run_performance_diagnosis(&deps, &ProtectionCatalog::embedded(), &[], 25.0);
        let primary = diag.primary.clone().unwrap();
        assert_eq!(primary.family, CpuFamily::Spotlight);
        assert_eq!(primary.avg_cpu, 70.0);
        assert_eq!(diag.sustained.len(), 2);
        assert!(!deps.hdiutil_called.get());
        let lines = format_diagnosis_lines(&diag);
        assert!(lines
            .iter()
            .any(|l| l.contains("Likely bottleneck: Spotlight indexing (~70.0% CPU sustained)")));
        assert!(lines.iter().any(|l| l.contains("WindowServer ~35.0%")));
    }

    #[test]
    fn syspolicyd_pressure_lists_user_dmg_only() {
        let mut deps = FakeDeps::new(" 50.0 syspolicyd\n", " 40.0 syspolicyd\n");
        deps.hdiutil = HDIUTIL_INFO;
        let diag = run_performance_diagnosis(&deps, &ProtectionCatalog::embedded(), &[], 25.0);
        assert_eq!(diag.primary.as_ref().unwrap().family, CpuFamily::Syspolicyd);
        assert_eq!(
            diag.gatekeeper_status.as_deref(),
            Some("assessments enabled")
        );
        assert_eq!(
            diag.detach_candidates,
            vec![MountedImage {
                image_path: "/Users/me/Downloads/Tool.dmg".into(),
                mount_path: "/Volumes/Tool Installer".into(),
            }]
        );
        assert!(!diag.coresim_only);
        let lines = format_diagnosis_lines(&diag);
        assert!(lines
            .iter()
            .any(|l| l.contains("Tool.dmg → /Volumes/Tool Installer")));
    }

    #[test]
    fn whitelisted_mount_is_not_offered() {
        let mut deps = FakeDeps::new(" 50.0 syspolicyd\n", " 40.0 syspolicyd\n");
        deps.hdiutil = HDIUTIL_INFO;
        let wl = vec!["/Volumes/Tool*".to_string()];
        let diag = run_performance_diagnosis(&deps, &ProtectionCatalog::embedded(), &wl, 25.0);
        assert!(diag.detach_candidates.is_empty());
    }

    #[test]
    fn coresim_only_is_informational() {
        let mut deps = FakeDeps::new(" 50.0 syspolicyd\n", " 40.0 syspolicyd\n");
        deps.hdiutil = "================\n\
image-path      : /Library/Developer/CoreSimulator/Images/iOS.dmg\n\
/dev/disk5s1        \tApple_APFS                     \t/Library/Developer/CoreSimulator/Volumes/iOS_22A\n";
        let diag = run_performance_diagnosis(&deps, &ProtectionCatalog::embedded(), &[], 25.0);
        assert!(diag.coresim_only);
        assert!(diag.detach_candidates.is_empty());
    }

    #[test]
    fn image_pairs_ignore_non_dev_lines() {
        let pairs = parse_image_mount_pairs(HDIUTIL_INFO);
        assert_eq!(pairs.len(), 3);
        assert!(pairs.iter().all(|p| p.mount_path.starts_with('/')));
        assert!(!pairs
            .iter()
            .any(|p| p.mount_path.contains("Downloads/Tool.dmg")));
    }

    struct FakeDetacher(&'static str);
    impl ImageDetacher for FakeDetacher {
        fn detach(&self, mount_path: &str) -> bool {
            mount_path != self.0
        }
    }

    #[test]
    fn detach_reports_success_and_failure() {
        let imgs = vec![
            MountedImage {
                image_path: "/a.dmg".into(),
                mount_path: "/Volumes/A".into(),
            },
            MountedImage {
                image_path: "/b.dmg".into(),
                mount_path: "/Volumes/B".into(),
            },
        ];
        let out = detach_mounted_images(&FakeDetacher("/Volumes/B"), &imgs);
        assert_eq!(out.detached, vec!["/Volumes/A".to_string()]);
        assert_eq!(out.failed, vec!["/Volumes/B".to_string()]);
    }

    #[test]
    fn live_detacher_never_runs_under_test_no_auth() {
        let _guard = crate::test_env::lock();
        std::env::set_var("VOLE_TEST_NO_AUTH", "1");
        assert!(!LiveImageDetacher.detach("/Volumes/Nope"));
        std::env::remove_var("VOLE_TEST_NO_AUTH");
    }
}
