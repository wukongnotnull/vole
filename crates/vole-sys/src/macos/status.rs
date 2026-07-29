//! macOS 状态指标采集。

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sysinfo::{
    CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind, RefreshKind, System,
};

use vole_proto::status::{
    smart_status, CpuStatus, DiskIoStatus, DiskStatus, HardwareInfo, MemoryStatus, NetworkHistory,
    NetworkStatus, ProcessInfo, ProcessWatchConfig, ProxyStatus, StatusSnapshot, ThermalStatus,
};

use crate::macos::syscommand::MacSysCommand;
use crate::timeouts::SHORT_QUERY;
use crate::traits::SysCommand;

const CPU_SAMPLE: Duration = Duration::from_millis(100);

pub struct MacStatusCollector {
    sys: System,
    disks: Disks,
    networks: Networks,
    prev_disk_totals: Option<(u64, u64, Instant)>,
    prev_net: HashMap<String, (u64, u64)>,
    last_net_at: Option<Instant>,
    hardware_cache: Option<HardwareInfo>,
    last_hw_at: Option<Instant>,
}

impl Default for MacStatusCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MacStatusCollector {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            prev_disk_totals: None,
            prev_net: HashMap::new(),
            last_net_at: None,
            hardware_cache: None,
            last_hw_at: None,
        }
    }

    pub fn collect_snapshot(&mut self, full_hardware: bool) -> StatusSnapshot {
        self.refresh_fast();
        let cpu = self.collect_cpu();
        let memory = self.collect_memory();
        let disks = self.collect_disks();
        let disk_io = self.collect_disk_io();
        let network = self.collect_network();
        let top_processes = self.collect_top_processes(10);
        let hardware = self.collect_hardware(full_hardware, memory.total, &disks);
        let host = System::host_name().unwrap_or_default();
        let platform = System::long_os_version().unwrap_or_else(|| "macOS".into());
        let uptime_secs = System::uptime();
        let uptime = format_uptime(uptime_secs);
        let collected_at = format_rfc3339(SystemTime::now());

        StatusSnapshot {
            collected_at,
            host,
            platform,
            uptime,
            uptime_seconds: uptime_secs,
            procs: self.sys.processes().len() as u64,
            hardware,
            health_score: 0,
            health_score_msg: String::new(),
            cpu,
            gpu: Vec::new(),
            memory,
            disks,
            trash_size: 0,
            trash_approx: false,
            disk_io,
            network,
            network_history: NetworkHistory::default(),
            proxy: ProxyStatus::default(),
            batteries: Vec::new(),
            thermal: ThermalStatus::default(),
            sensors: Vec::new(),
            bluetooth: Vec::new(),
            top_processes,
            process_watch: ProcessWatchConfig::default(),
            process_alerts: Vec::new(),
        }
    }

    fn refresh_fast(&mut self) {
        self.sys.refresh_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
                .with_processes(ProcessRefreshKind::everything()),
        );
        self.disks.refresh(false);
        self.networks.refresh(false);
    }

    fn collect_cpu(&mut self) -> CpuStatus {
        self.sys.refresh_cpu_usage();
        std::thread::sleep(CPU_SAMPLE);
        self.sys.refresh_cpu_usage();

        let logical = self.sys.cpus().len();
        let per_core: Vec<f64> = self
            .sys
            .cpus()
            .iter()
            .map(|c| c.cpu_usage() as f64)
            .collect();
        let usage = if per_core.is_empty() {
            0.0
        } else {
            per_core.iter().sum::<f64>() / per_core.len() as f64
        };

        let mut load = [0.0, 0.0, 0.0];
        let n = unsafe { libc::getloadavg(load.as_mut_ptr(), 3) };
        let (load1, load5, load15) = if n == 3 {
            (load[0], load[1], load[2])
        } else {
            (0.0, 0.0, 0.0)
        };

        CpuStatus {
            usage,
            per_core,
            per_core_estimated: false,
            load1,
            load5,
            load15,
            core_count: logical as i32,
            logical_cpu: logical as i32,
            p_core_count: 0,
            e_core_count: 0,
        }
    }

    fn collect_memory(&self) -> MemoryStatus {
        let total = self.sys.total_memory();
        let used = self.sys.used_memory();
        let available = self.sys.available_memory();
        let used_percent = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        MemoryStatus {
            used,
            total,
            available,
            used_percent,
            swap_used: self.sys.used_swap(),
            swap_total: self.sys.total_swap(),
            cached: 0,
            pressure: get_memory_pressure(),
        }
    }

    fn collect_disks(&mut self) -> Vec<DiskStatus> {
        let mut out = Vec::new();
        for disk in self.disks.list() {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            let used_percent = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            out.push(DiskStatus {
                mount: disk.mount_point().display().to_string(),
                device: disk.name().to_string_lossy().into_owned(),
                used,
                total,
                used_percent,
                fstype: disk.file_system().to_string_lossy().into_owned(),
                external: disk.mount_point().starts_with("/Volumes"),
                smart_status: smart_status::UNKNOWN.into(),
            });
        }
        out.sort_by(|a, b| a.mount.cmp(&b.mount));
        out
    }

    fn collect_disk_io(&mut self) -> DiskIoStatus {
        let now = Instant::now();
        let mut total_read = 0u64;
        let mut total_write = 0u64;
        for disk in self.disks.list() {
            let u = disk.usage();
            total_read += u.total_read_bytes;
            total_write += u.total_written_bytes;
        }

        let (read_rate, write_rate) = match self.prev_disk_totals {
            Some((prev_r, prev_w, at)) => {
                let dt = now.duration_since(at).as_secs_f64();
                if dt > 0.0 {
                    (
                        (total_read.saturating_sub(prev_r) as f64 / dt) / 1_048_576.0,
                        (total_write.saturating_sub(prev_w) as f64 / dt) / 1_048_576.0,
                    )
                } else {
                    (0.0, 0.0)
                }
            }
            None => (0.0, 0.0),
        };
        self.prev_disk_totals = Some((total_read, total_write, now));

        DiskIoStatus {
            read_rate,
            write_rate,
        }
    }

    fn collect_network(&mut self) -> Vec<NetworkStatus> {
        let now = Instant::now();
        let dt = self
            .last_net_at
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(0.0);
        self.last_net_at = Some(now);

        let mut out = Vec::new();
        for (name, data) in self.networks.iter() {
            let received = data.received();
            let transmitted = data.transmitted();
            let (rx_rate, tx_rate) = if dt > 0.0 {
                match self.prev_net.get(name) {
                    Some((pr, pt)) => (
                        (received.saturating_sub(*pr) as f64 / dt) / 1_048_576.0,
                        (transmitted.saturating_sub(*pt) as f64 / dt) / 1_048_576.0,
                    ),
                    None => (0.0, 0.0),
                }
            } else {
                (0.0, 0.0)
            };
            self.prev_net.insert(name.clone(), (received, transmitted));
            out.push(NetworkStatus {
                name: name.clone(),
                rx_rate_mbs: rx_rate,
                tx_rate_mbs: tx_rate,
                ip: String::new(),
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn collect_top_processes(&mut self, n: usize) -> Vec<ProcessInfo> {
        let mut procs: Vec<ProcessInfo> = self
            .sys
            .processes()
            .values()
            .map(|p| {
                let mem_bytes = p.memory();
                let total = self.sys.total_memory().max(1);
                ProcessInfo {
                    pid: p.pid().as_u32() as i32,
                    ppid: p.parent().map(|x| x.as_u32() as i32).unwrap_or(0),
                    name: p.name().to_string_lossy().into_owned(),
                    command: p
                        .cmd()
                        .iter()
                        .map(|s| s.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" "),
                    cpu: p.cpu_usage() as f64,
                    memory: (mem_bytes as f64 / total as f64) * 100.0,
                    memory_bytes: Some(mem_bytes),
                }
            })
            .collect();
        procs.sort_by(|a, b| {
            b.cpu
                .partial_cmp(&a.cpu)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        procs.truncate(n);
        procs
    }

    fn collect_hardware(
        &mut self,
        refresh: bool,
        total_ram: u64,
        disks: &[DiskStatus],
    ) -> HardwareInfo {
        if !refresh {
            if let Some(hw) = &self.hardware_cache {
                return hw.clone();
            }
        }
        if let (Some(hw), Some(at)) = (&self.hardware_cache, self.last_hw_at) {
            if at.elapsed() < Duration::from_secs(60) && !refresh {
                return hw.clone();
            }
        }

        let cmd = MacSysCommand;
        let mut model = String::new();
        let mut cpu_model = String::new();
        if let Ok(out) = cmd.run(
            &["system_profiler", "SPHardwareDataType"],
            Duration::from_secs(3),
        ) {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let lower = line.to_lowercase();
                if lower.contains("model name:") {
                    if let Some((_, v)) = line.split_once(':') {
                        model = v.trim().to_string();
                    }
                }
                if lower.contains("chip:") {
                    if let Some((_, v)) = line.split_once(':') {
                        cpu_model = v.trim().to_string();
                    }
                }
            }
        }
        if model.is_empty() {
            if let Ok(out) = cmd.run(&["sysctl", "-n", "hw.model"], SHORT_QUERY) {
                model = String::from_utf8_lossy(&out.stdout).trim().into();
            }
        }
        if cpu_model.is_empty() {
            if let Ok(out) = cmd.run(&["sysctl", "-n", "machdep.cpu.brand_string"], SHORT_QUERY) {
                cpu_model = String::from_utf8_lossy(&out.stdout).trim().into();
            }
        }
        let os_version = if let Ok(out) = cmd.run(&["sw_vers", "-productVersion"], SHORT_QUERY) {
            format!("macOS {}", String::from_utf8_lossy(&out.stdout).trim())
        } else {
            System::long_os_version().unwrap_or_default()
        };

        let disk_size = disks
            .iter()
            .find(|d| d.mount == "/" || d.mount == "/System/Volumes/Data")
            .map(|d| human_bytes_base10(d.total))
            .unwrap_or_else(|| "Unknown".into());

        let hw = HardwareInfo {
            model,
            cpu_model,
            total_ram: human_bytes_base10(total_ram),
            disk_size,
            os_version,
            refresh_rate: String::new(),
        };
        self.hardware_cache = Some(hw.clone());
        self.last_hw_at = Some(Instant::now());
        hw
    }
}

fn human_bytes_base10(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        let scaled = (bytes * 100 + 500_000_000) / 1_000_000_000;
        format!("{}.{:02}GB", scaled / 100, scaled % 100)
    } else if bytes >= 1_000_000 {
        let scaled = (bytes * 10 + 500_000) / 1_000_000;
        format!("{}.{:01}MB", scaled / 10, scaled % 10)
    } else if bytes >= 1000 {
        format!("{}KB", (bytes + 500) / 1000)
    } else if bytes > 0 {
        format!("{}B", bytes)
    } else {
        "0B".into()
    }
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn get_memory_pressure() -> String {
    let cmd = MacSysCommand;
    let Ok(out) = cmd.run(&["memory_pressure"], Duration::from_millis(500)) else {
        return String::new();
    };
    parse_memory_pressure(&String::from_utf8_lossy(&out.stdout))
}

/// 解析 `memory_pressure` 输出（对齐 mole `getMemoryPressure`）。
pub(crate) fn parse_memory_pressure(output: &str) -> String {
    let lower = output.to_lowercase();
    if lower.contains("critical") {
        return "critical".into();
    }
    if lower.contains("warn") {
        return "warn".into();
    }
    if lower.contains("normal") {
        return "normal".into();
    }
    String::new()
}

fn format_rfc3339(time: SystemTime) -> String {
    let dur = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let nanos = dur.subsec_nanos();

    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let year_days = if leap { 366 } else { 365 };
        if remaining_days < year_days {
            break;
        }
        remaining_days -= year_days;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0;
    while m < 12 && remaining_days >= month_days[m] as i64 {
        remaining_days -= month_days[m] as i64;
        m += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        y,
        m + 1,
        day,
        hour,
        minute,
        second,
        nanos
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn parse_memory_pressure_levels() {
        assert_eq!(
            parse_memory_pressure("system-wide memory pressure: normal"),
            "normal"
        );
        assert_eq!(parse_memory_pressure("WARN level"), "warn");
        assert_eq!(parse_memory_pressure("CRITICAL"), "critical");
        assert_eq!(parse_memory_pressure("unknown"), "");
    }

    #[test]
    fn format_rfc3339_is_iso_datetime() {
        let t = UNIX_EPOCH + Duration::from_secs(1_704_067_200); // 2024-01-01 00:00:00 UTC
        let s = format_rfc3339(t);
        assert!(s.starts_with("2024-01-01T00:00:00."));
        assert!(s.ends_with('Z'));
    }
}
