//! `mo status --json` 对齐的类型定义。

use serde::{Deserialize, Serialize};

/// 与 mole `MetricsSnapshot` 字段名一致。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StatusSnapshot {
    pub collected_at: String,
    pub host: String,
    pub platform: String,
    pub uptime: String,
    pub uptime_seconds: u64,
    pub procs: u64,
    pub hardware: HardwareInfo,
    pub health_score: i32,
    pub health_score_msg: String,
    pub cpu: CpuStatus,
    pub gpu: Vec<GpuStatus>,
    pub memory: MemoryStatus,
    pub disks: Vec<DiskStatus>,
    pub trash_size: u64,
    pub trash_approx: bool,
    pub disk_io: DiskIoStatus,
    pub network: Vec<NetworkStatus>,
    pub network_history: NetworkHistory,
    pub proxy: ProxyStatus,
    pub batteries: Vec<BatteryStatus>,
    pub thermal: ThermalStatus,
    pub sensors: Vec<SensorReading>,
    pub bluetooth: Vec<BluetoothDevice>,
    pub top_processes: Vec<ProcessInfo>,
    pub process_watch: ProcessWatchConfig,
    pub process_alerts: Vec<ProcessAlert>,
    /// Time Machine 本地快照报告（仅 list；无则 JSON 省略）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_snapshots: Option<LocalSnapshotsInfo>,
}

/// `tmutil listlocalsnapshots` 报告面（Mole `clean_local_snapshots` 对齐）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocalSnapshotsInfo {
    /// Present 时为数量；Skipped* 时省略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HardwareInfo {
    pub model: String,
    pub cpu_model: String,
    pub total_ram: String,
    pub disk_size: String,
    pub os_version: String,
    pub refresh_rate: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DiskIoStatus {
    pub read_rate: f64,
    pub write_rate: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProcessInfo {
    pub pid: i32,
    pub ppid: i32,
    pub name: String,
    pub command: String,
    pub cpu: f64,
    pub memory: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CpuStatus {
    pub usage: f64,
    pub per_core: Vec<f64>,
    pub per_core_estimated: bool,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub core_count: i32,
    pub logical_cpu: i32,
    pub p_core_count: i32,
    pub e_core_count: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GpuStatus {
    pub name: String,
    pub usage: f64,
    pub memory_used: f64,
    pub memory_total: f64,
    pub core_count: i32,
    pub note: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MemoryStatus {
    pub used: u64,
    pub total: u64,
    pub available: u64,
    pub used_percent: f64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub cached: u64,
    pub pressure: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DiskStatus {
    pub mount: String,
    pub device: String,
    pub used: u64,
    pub total: u64,
    pub used_percent: f64,
    pub fstype: String,
    pub external: bool,
    pub smart_status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NetworkStatus {
    pub name: String,
    pub rx_rate_mbs: f64,
    pub tx_rate_mbs: f64,
    pub ip: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NetworkHistory {
    pub rx_history: Vec<f64>,
    pub tx_history: Vec<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProxyStatus {
    pub enabled: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub host: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BatteryStatus {
    pub percent: f64,
    pub status: String,
    pub time_left: String,
    pub health: String,
    pub cycle_count: i32,
    pub capacity: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ThermalStatus {
    pub cpu_temp: f64,
    pub gpu_temp: f64,
    pub battery_temp: f64,
    pub fan_speed: i32,
    pub fan_count: i32,
    pub system_power: f64,
    pub adapter_power: f64,
    pub battery_power: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SensorReading {
    pub label: String,
    pub value: f64,
    pub unit: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BluetoothDevice {
    pub name: String,
    pub connected: bool,
    pub battery: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProcessWatchConfig {
    pub enabled: bool,
    pub cpu_threshold: f64,
    pub window: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProcessAlert {
    pub pid: i32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub cpu: f64,
    pub threshold: f64,
    pub window: String,
    pub triggered_at: String,
    pub status: String,
}

/// SMART 状态字符串（对齐 mole `metrics_disk.go`）。
pub mod smart_status {
    pub const VERIFIED: &str = "verified";
    pub const FAILING: &str = "failing";
    pub const UNSUPPORTED: &str = "unsupported";
    pub const UNKNOWN: &str = "unknown";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_snapshot_roundtrip_snake_case() {
        let snap = StatusSnapshot {
            host: "mac".into(),
            health_score: 85,
            ..Default::default()
        };
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(v["health_score"], 85);
        assert!(v.get("host").is_some());
    }

    #[test]
    fn local_snapshots_omitted_when_none() {
        let snap = StatusSnapshot::default();
        let v = serde_json::to_value(&snap).unwrap();
        assert!(v.get("local_snapshots").is_none());
    }

    #[test]
    fn local_snapshots_present_serializes() {
        let snap = StatusSnapshot {
            local_snapshots: Some(LocalSnapshotsInfo {
                count: Some(2),
                message: "Time Machine local snapshots · 2 (review: tmutil listlocalsnapshots /)"
                    .into(),
            }),
            ..Default::default()
        };
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(v["local_snapshots"]["count"], 2);
        assert!(v["local_snapshots"]["message"]
            .as_str()
            .unwrap()
            .contains("review"));
    }
}
