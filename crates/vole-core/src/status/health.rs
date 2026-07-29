//! 系统健康分，移植自 mole `metrics_health.go`。

use crate::vole_proto::status::{
    smart_status, BatteryStatus, CpuStatus, DiskIoStatus, DiskStatus, MemoryStatus, ThermalStatus,
};

const HEALTH_CPU_WEIGHT: f64 = 30.0;
const HEALTH_MEM_WEIGHT: f64 = 25.0;
const HEALTH_DISK_WEIGHT: f64 = 20.0;
const HEALTH_THERMAL_WEIGHT: f64 = 15.0;
const HEALTH_IO_WEIGHT: f64 = 10.0;

const CPU_NORMAL_THRESHOLD: f64 = 50.0;
const CPU_HIGH_THRESHOLD: f64 = 85.0;

const MEM_NORMAL_THRESHOLD: f64 = 70.0;
const MEM_HIGH_THRESHOLD: f64 = 88.0;
const MEM_PRESSURE_WARN_PENALTY: f64 = 5.0;
const MEM_PRESSURE_CRIT_PENALTY: f64 = 15.0;

const DISK_WARN_THRESHOLD: f64 = 80.0;
const DISK_CRIT_THRESHOLD: f64 = 93.0;

const THERMAL_NORMAL_THRESHOLD: f64 = 65.0;
const THERMAL_HIGH_THRESHOLD: f64 = 85.0;

const IO_NORMAL_THRESHOLD: f64 = 50.0;
const IO_HIGH_THRESHOLD: f64 = 150.0;

const BATTERY_CYCLE_WARN: i32 = 800;
const BATTERY_CYCLE_DANGER: i32 = 900;
const BATTERY_CAP_WARN: i32 = 80;
const BATTERY_CAP_DANGER: i32 = 60;

const UPTIME_WARN_SECS: u64 = 7 * 86400;
const UPTIME_DANGER_SECS: u64 = 14 * 86400;

const SCORE_EXCELLENT_THRESHOLD: i32 = 85;
const SCORE_GOOD_THRESHOLD: i32 = 65;
const SCORE_FAIR_THRESHOLD: i32 = 45;

pub fn calculate_health_score(
    cpu: &CpuStatus,
    mem: &MemoryStatus,
    disks: &[DiskStatus],
    disk_io: &DiskIoStatus,
    thermal: &ThermalStatus,
    batteries: &[BatteryStatus],
    uptime_secs: u64,
) -> (i32, String) {
    let mut score = 100.0;
    let mut issues: Vec<String> = Vec::new();

    let mut cpu_penalty = 0.0;
    if cpu.usage > CPU_NORMAL_THRESHOLD {
        if cpu.usage > CPU_HIGH_THRESHOLD {
            cpu_penalty = HEALTH_CPU_WEIGHT * (cpu.usage - CPU_NORMAL_THRESHOLD)
                / (100.0 - CPU_NORMAL_THRESHOLD);
        } else {
            cpu_penalty = (HEALTH_CPU_WEIGHT / 2.0) * (cpu.usage - CPU_NORMAL_THRESHOLD)
                / (CPU_HIGH_THRESHOLD - CPU_NORMAL_THRESHOLD);
        }
    }
    score -= cpu_penalty;
    if cpu.usage > CPU_HIGH_THRESHOLD {
        issues.push("High CPU".into());
    }

    let mut mem_penalty = 0.0;
    if mem.used_percent > MEM_NORMAL_THRESHOLD {
        if mem.used_percent > MEM_HIGH_THRESHOLD {
            mem_penalty = HEALTH_MEM_WEIGHT * (mem.used_percent - MEM_NORMAL_THRESHOLD)
                / (100.0 - MEM_NORMAL_THRESHOLD);
        } else {
            mem_penalty = (HEALTH_MEM_WEIGHT / 2.0) * (mem.used_percent - MEM_NORMAL_THRESHOLD)
                / (MEM_HIGH_THRESHOLD - MEM_NORMAL_THRESHOLD);
        }
    }
    score -= mem_penalty;
    if mem.used_percent > MEM_HIGH_THRESHOLD {
        issues.push("High Memory".into());
    }

    match mem.pressure.as_str() {
        "warn" => {
            score -= MEM_PRESSURE_WARN_PENALTY;
            issues.push("Memory Pressure".into());
        }
        "critical" => {
            score -= MEM_PRESSURE_CRIT_PENALTY;
            issues.push("Critical Memory".into());
        }
        _ => {}
    }

    let mut disk_penalty = 0.0;
    if !disks.is_empty() {
        let disk_usage = disks[0].used_percent;
        if disk_usage > DISK_WARN_THRESHOLD {
            if disk_usage > DISK_CRIT_THRESHOLD {
                disk_penalty = HEALTH_DISK_WEIGHT * (disk_usage - DISK_WARN_THRESHOLD)
                    / (100.0 - DISK_WARN_THRESHOLD);
            } else {
                disk_penalty = (HEALTH_DISK_WEIGHT / 2.0) * (disk_usage - DISK_WARN_THRESHOLD)
                    / (DISK_CRIT_THRESHOLD - DISK_WARN_THRESHOLD);
            }
        }
        score -= disk_penalty;
        if disk_usage > DISK_CRIT_THRESHOLD {
            issues.push("Disk Almost Full".into());
        }
    }
    for disk in disks {
        if disk.smart_status == smart_status::FAILING {
            if score > 44.0 {
                score = 44.0;
            }
            issues.push("Disk SMART Failing".into());
            break;
        }
    }

    let mut thermal_penalty = 0.0;
    if thermal.cpu_temp > 0.0 {
        if thermal.cpu_temp > THERMAL_NORMAL_THRESHOLD {
            if thermal.cpu_temp > THERMAL_HIGH_THRESHOLD {
                thermal_penalty = HEALTH_THERMAL_WEIGHT;
                issues.push("Overheating".into());
            } else {
                thermal_penalty = HEALTH_THERMAL_WEIGHT
                    * (thermal.cpu_temp - THERMAL_NORMAL_THRESHOLD)
                    / (THERMAL_HIGH_THRESHOLD - THERMAL_NORMAL_THRESHOLD);
            }
        }
        score -= thermal_penalty;
    }

    let mut io_penalty = 0.0;
    let total_io = disk_io.read_rate + disk_io.write_rate;
    if total_io > IO_NORMAL_THRESHOLD {
        if total_io > IO_HIGH_THRESHOLD {
            io_penalty = HEALTH_IO_WEIGHT;
            issues.push("Heavy Disk IO".into());
        } else {
            io_penalty = HEALTH_IO_WEIGHT * (total_io - IO_NORMAL_THRESHOLD)
                / (IO_HIGH_THRESHOLD - IO_NORMAL_THRESHOLD);
        }
    }
    score -= io_penalty;

    if !batteries.is_empty() {
        let b = &batteries[0];
        let sev = battery_health_severity(b.cycle_count, b.capacity);
        match sev {
            "danger" => {
                score -= 5.0;
                issues.push("Battery Service Soon".into());
            }
            "warn" => score -= 2.0,
            _ => {}
        }
    }

    if uptime_secs > UPTIME_DANGER_SECS {
        score -= 3.0;
        issues.push("Restart Recommended".into());
    } else if uptime_secs > UPTIME_WARN_SECS {
        score -= 1.0;
    }

    score = score.clamp(0.0, 100.0);

    let mut msg = match score as i32 {
        s if s >= SCORE_EXCELLENT_THRESHOLD => "Excellent",
        s if s >= SCORE_GOOD_THRESHOLD => "Good",
        s if s >= SCORE_FAIR_THRESHOLD => "Fair",
        _ => "Needs Attention",
    }
    .to_string();

    if !issues.is_empty() {
        msg = format!("{}: {}", msg, issues.join(", "));
    }

    (score as i32, msg)
}

fn battery_health_severity(cycles: i32, capacity: i32) -> &'static str {
    if cycles > BATTERY_CYCLE_DANGER || (capacity > 0 && capacity < BATTERY_CAP_DANGER) {
        "danger"
    } else if cycles > BATTERY_CYCLE_WARN || (capacity > 0 && capacity < BATTERY_CAP_WARN) {
        "warn"
    } else {
        "ok"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[derive(serde::Deserialize)]
    struct FixtureCpu {
        usage: f64,
    }

    #[derive(serde::Deserialize)]
    struct FixtureMemory {
        used_percent: f64,
        pressure: String,
    }

    #[derive(serde::Deserialize)]
    struct FixtureDisk {
        used_percent: f64,
        smart_status: String,
    }

    #[derive(serde::Deserialize)]
    struct FixtureDiskIo {
        read_rate: f64,
        write_rate: f64,
    }

    #[derive(serde::Deserialize)]
    struct FixtureThermal {
        cpu_temp: f64,
    }

    #[derive(serde::Deserialize)]
    struct FixtureCase {
        name: String,
        cpu: FixtureCpu,
        memory: FixtureMemory,
        disks: Vec<FixtureDisk>,
        disk_io: FixtureDiskIo,
        thermal: FixtureThermal,
        batteries: Vec<BatteryStatus>,
        uptime_seconds: u64,
        want_score: Option<i32>,
        want_score_max: Option<i32>,
        want_msg_contains: String,
    }

    #[test]
    fn fixture_table_matches_go_tests() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/fixtures/status-health-score.json"
        );
        let text = fs::read_to_string(path).expect("fixture");
        let cases: Vec<FixtureCase> = serde_json::from_str(&text).expect("parse fixture");
        for case in cases {
            let cpu = CpuStatus {
                usage: case.cpu.usage,
                ..Default::default()
            };
            let memory = MemoryStatus {
                used_percent: case.memory.used_percent,
                pressure: case.memory.pressure,
                ..Default::default()
            };
            let disks: Vec<DiskStatus> = case
                .disks
                .iter()
                .map(|d| DiskStatus {
                    used_percent: d.used_percent,
                    smart_status: d.smart_status.clone(),
                    ..Default::default()
                })
                .collect();
            let disk_io = DiskIoStatus {
                read_rate: case.disk_io.read_rate,
                write_rate: case.disk_io.write_rate,
            };
            let thermal = ThermalStatus {
                cpu_temp: case.thermal.cpu_temp,
                ..Default::default()
            };
            let (score, msg) = calculate_health_score(
                &cpu,
                &memory,
                &disks,
                &disk_io,
                &thermal,
                &case.batteries,
                case.uptime_seconds,
            );
            if let Some(want) = case.want_score {
                assert_eq!(score, want, "case {}", case.name);
            }
            if let Some(max) = case.want_score_max {
                assert!(score < max, "case {} score {} >= {}", case.name, score, max);
            }
            assert!(
                msg.contains(&case.want_msg_contains),
                "case {} msg {:?} missing {:?}",
                case.name,
                msg,
                case.want_msg_contains
            );
        }
    }

    #[test]
    fn monotonic_in_cpu() {
        let mut prev = 101;
        let mut usage = 40.0;
        while usage <= 100.0 {
            let (score, _) = calculate_health_score(
                &CpuStatus {
                    usage,
                    ..Default::default()
                },
                &MemoryStatus {
                    used_percent: 20.0,
                    pressure: "normal".into(),
                    ..Default::default()
                },
                &[DiskStatus {
                    used_percent: 30.0,
                    ..Default::default()
                }],
                &DiskIoStatus {
                    read_rate: 5.0,
                    write_rate: 5.0,
                },
                &ThermalStatus {
                    cpu_temp: 40.0,
                    ..Default::default()
                },
                &[],
                0,
            );
            assert!(
                score <= prev,
                "score rose from {} to {} at cpu {}",
                prev,
                score,
                usage
            );
            prev = score;
            usage += 0.5;
        }
    }
}
