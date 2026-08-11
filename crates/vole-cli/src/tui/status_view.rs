//! `status` TUI 渲染（mole `cmd/status/view.go` 区块同构）。

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use vole_core::vole_proto::status::{
    BatteryStatus, CpuStatus, DiskStatus, MemoryStatus, NetworkStatus, ProcessAlert, ProcessInfo,
    ProxyStatus, StatusSnapshot, ThermalStatus,
};

use super::status_cat::render_mole_frame;
use super::theme::{color_bucket, Theme};
use super::widgets::{
    fit_status_header, format_bytes_bin, format_rate_mbs, line_pair, metric_bar_line, mini_bar,
    plain_progress_bar, shorten, status_footer, status_layout_mode, StatusLayoutMode,
};

const ICON_CPU: &str = "◉";
const ICON_MEMORY: &str = "◫";
const ICON_DISK: &str = "▥";
const ICON_NETWORK: &str = "⇅";
const ICON_BATTERY: &str = "◪";
const ICON_PROCS: &str = "❊";
const STATUS_NARROW: u16 = 80;

#[derive(Debug, Clone, Copy)]
pub struct StatusRenderOpts {
    pub cat_hidden: bool,
    pub anim_frame: u64,
    /// 0 = all cores.
    pub cpu_cores: i32,
}

impl Default for StatusRenderOpts {
    fn default() -> Self {
        Self {
            cat_hidden: false,
            anim_frame: 0,
            cpu_cores: 2,
        }
    }
}

pub fn render_status(
    frame: &mut Frame,
    snap: &StatusSnapshot,
    theme: &Theme,
    opts: StatusRenderOpts,
) {
    let area = frame.area();
    let width = area.width;
    let tip = snap
        .local_snapshots
        .as_ref()
        .map(|info| info.message.as_str());
    let alert = format_process_alert(snap.process_alerts.as_slice());
    let show_cat = !opts.cat_hidden && width >= 20;

    let mut constraints = vec![Constraint::Length(1)];
    if alert.is_some() {
        constraints.push(Constraint::Length(1));
    }
    if tip.is_some() {
        constraints.push(Constraint::Length(1));
    }
    if show_cat {
        constraints.push(Constraint::Length(4));
    }
    constraints.push(Constraint::Min(4));
    constraints.push(Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0usize;
    frame.render_widget(
        Paragraph::new(build_status_header(snap, width as usize, theme)),
        chunks[idx],
    );
    idx += 1;

    if let Some(ref text) = alert {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                text.clone(),
                theme.warn.add_modifier(Modifier::BOLD),
            ))),
            chunks[idx],
        );
        idx += 1;
    }

    if let Some(msg) = tip {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(msg.to_string(), theme.subtle))),
            chunks[idx],
        );
        idx += 1;
    }

    if show_cat {
        let mole = render_mole_frame(opts.anim_frame, width as usize);
        frame.render_widget(Paragraph::new(mole).style(theme.ok), chunks[idx]);
        idx += 1;
    }

    render_cards(frame, chunks[idx], snap, theme, width, opts.cpu_cores);
    idx += 1;

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(status_footer(), theme.subtle))),
        chunks[idx],
    );
}

/// Row heights from card line counts — content-sized so tall terminals don't
/// stretch every card row equally (leftover space sinks via a trailing `Min(0)`).
fn card_row_heights(card_lens: &[usize], two_column: bool) -> Vec<u16> {
    if two_column {
        card_lens
            .chunks(2)
            .map(|pair| pair.iter().copied().max().unwrap_or(1).max(1) as u16)
            .collect()
    } else {
        card_lens.iter().map(|&n| n.max(1) as u16).collect()
    }
}

fn render_cards(
    frame: &mut Frame,
    area: Rect,
    snap: &StatusSnapshot,
    theme: &Theme,
    width: u16,
    cpu_cores: i32,
) {
    let cards = build_card_blocks(snap, theme, width, cpu_cores);
    let two_column = matches!(status_layout_mode(width), StatusLayoutMode::TwoColumn);
    let heights = card_row_heights(
        &cards.iter().map(|c| c.len()).collect::<Vec<_>>(),
        two_column,
    );
    let mut constraints: Vec<Constraint> = heights.iter().map(|&h| Constraint::Length(h)).collect();
    // Absorb leftover vertical space below the cards, not between them.
    constraints.push(Constraint::Min(0));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    if two_column {
        for (row_i, pair) in cards.chunks(2).enumerate() {
            let Some(row) = rows.get(row_i) else { break };
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(*row);
            frame.render_widget(Paragraph::new(pair[0].clone()), cols[0]);
            if pair.len() > 1 {
                frame.render_widget(Paragraph::new(pair[1].clone()), cols[1]);
            }
        }
    } else {
        for (i, card) in cards.into_iter().enumerate() {
            if let Some(rect) = rows.get(i) {
                frame.render_widget(Paragraph::new(card), *rect);
            }
        }
    }
}

pub fn build_status_header(snap: &StatusSnapshot, width: usize, theme: &Theme) -> Line<'static> {
    let head = format!(
        "Status  Health ● {} {}",
        snap.health_score, snap.health_score_msg
    );
    let compact = width > 0 && width <= STATUS_NARROW as usize;
    let mut identity = Vec::new();
    if !snap.hardware.model.is_empty() {
        identity.push(snap.hardware.model.clone());
    }
    if !snap.hardware.cpu_model.is_empty() {
        let mut cpu = snap.hardware.cpu_model.clone();
        if let Some(gpu) = snap.gpu.first() {
            if gpu.core_count > 0 {
                cpu.push_str(&format!(", {}GPU", gpu.core_count));
            }
        }
        identity.push(cpu);
    }
    let mut specs = Vec::new();
    if !snap.hardware.total_ram.is_empty() {
        specs.push(format!("RAM {}", snap.hardware.total_ram));
    } else if snap.memory.total > 0 {
        specs.push(format!("RAM {}", format_bytes_bin(snap.memory.total)));
    }
    if !snap.hardware.disk_size.is_empty() {
        specs.push(format!("Disk {}", snap.hardware.disk_size));
    } else if let Some(d) = snap.disks.first() {
        if d.total > 0 {
            specs.push(format!("Disk {}", format_bytes_bin(d.total)));
        }
    }
    let mut refresh = Vec::new();
    if !snap.hardware.refresh_rate.is_empty() {
        refresh.push(snap.hardware.refresh_rate.clone());
    }
    let mut optional = Vec::new();
    if !compact && !snap.hardware.os_version.is_empty() {
        optional.push(snap.hardware.os_version.clone());
    }
    if !compact && !snap.uptime.is_empty() {
        optional.push(format!("up {}", snap.uptime));
    }

    let join = |groups: &[Vec<String>]| -> Vec<String> {
        groups.iter().flat_map(|g| g.iter().cloned()).collect()
    };

    let mut candidates = vec![
        join(&[identity.clone(), specs.clone(), refresh.clone(), optional]),
        join(&[identity.clone(), specs.clone(), refresh.clone()]),
        join(&[identity.clone(), specs.clone()]),
    ];
    if identity.len() > 1 {
        candidates.push(join(&[identity[..1].to_vec(), specs.clone()]));
    }
    candidates.push(specs);

    let text = fit_status_header(&head, &candidates, width);
    let score_style = theme.style_for_health(snap.health_score);
    // "Status  Health ● N msg …" — color the score token when present.
    let score_token = format!("● {}", snap.health_score);
    if let Some((before, after)) = text.split_once(&score_token) {
        Line::from(vec![
            Span::styled(before.to_string(), theme.title),
            Span::styled(score_token, score_style),
            Span::styled(after.to_string(), theme.subtle),
        ])
    } else if let Some(rest) = text.strip_prefix("Status") {
        Line::from(vec![
            Span::styled("Status".to_string(), theme.title),
            Span::styled(rest.to_string(), theme.subtle),
        ])
    } else {
        Line::from(Span::styled(text, theme.title))
    }
}

pub fn format_process_alert(alerts: &[ProcessAlert]) -> Option<String> {
    let active: Vec<&ProcessAlert> = alerts
        .iter()
        .filter(|a| a.status.eq_ignore_ascii_case("active") || a.status.is_empty())
        .collect();
    let focus = active.first().copied().or_else(|| alerts.first())?;
    let mut text = format!(
        "ALERT {} at {:.1}% for {} (threshold {:.1}%)",
        focus.name, focus.cpu, focus.window, focus.threshold
    );
    if alerts.len() > 1 {
        text.push_str(&format!(" · +{} more", alerts.len() - 1));
    }
    Some(text)
}

fn build_card_blocks(
    snap: &StatusSnapshot,
    theme: &Theme,
    width: u16,
    cpu_cores: i32,
) -> Vec<Vec<Line<'static>>> {
    let card_w = if matches!(status_layout_mode(width), StatusLayoutMode::TwoColumn) {
        width / 2
    } else {
        width
    };
    vec![
        render_cpu_card(&snap.cpu, &snap.thermal, theme, cpu_cores),
        render_memory_card(&snap.memory, theme),
        render_disk_card(
            &snap.disks,
            snap.disk_io.read_rate,
            snap.disk_io.write_rate,
            theme,
        ),
        render_power_card(&snap.batteries, &snap.thermal, theme),
        render_process_card(&snap.top_processes, card_w, theme),
        render_network_card(&snap.network, &snap.proxy, theme),
    ]
}

fn card_header(icon: &str, title: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{} {}", icon, title), theme.title),
        Span::styled("  ╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌".to_string(), theme.rule),
    ])
}

fn render_cpu_card(
    cpu: &CpuStatus,
    thermal: &ThermalStatus,
    theme: &Theme,
    cpu_cores: i32,
) -> Vec<Line<'static>> {
    let mut lines = vec![card_header(ICON_CPU, "CPU", theme)];
    let mut usage = format!("{:.1}%", cpu.usage);
    if thermal.cpu_temp > 0.0 {
        usage.push_str(&format!(" @ {:.1}°C", thermal.cpu_temp));
    }
    lines.push(Line::from(vec![
        Span::styled("Total ".to_string(), theme.label),
        Span::styled(
            format!(" {}", plain_progress_bar(cpu.usage)),
            theme.style_for_bucket(color_bucket(cpu.usage)),
        ),
        Span::styled(format!("  {}", usage), theme.value),
    ]));
    if cpu.per_core_estimated {
        lines.push(Line::from(Span::styled(
            "Per-core data unavailable, using averaged load".to_string(),
            theme.subtle,
        )));
    } else if !cpu.per_core.is_empty() {
        let mut cores: Vec<(usize, f64)> = cpu
            .per_core
            .iter()
            .enumerate()
            .map(|(i, v)| (i, *v))
            .collect();
        cores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let limit = if cpu_cores <= 0 {
            usize::MAX
        } else {
            cpu_cores as usize
        };
        for (idx, val) in cores.into_iter().take(limit) {
            lines.push(Line::from(format!(
                "Core{:<2} {}  {:5.1}%",
                idx + 1,
                plain_progress_bar(val),
                val
            )));
        }
    }
    let load = if cpu.p_core_count > 0 && cpu.e_core_count > 0 {
        format!(
            "Load   {:.2} / {:.2} / {:.2}, {}P+{}E",
            cpu.load1, cpu.load5, cpu.load15, cpu.p_core_count, cpu.e_core_count
        )
    } else {
        format!(
            "Load   {:.2} / {:.2} / {:.2}, {} cores",
            cpu.load1, cpu.load5, cpu.load15, cpu.logical_cpu
        )
    };
    lines.push(Line::from(Span::styled(load, theme.value)));
    lines
}

fn render_memory_card(mem: &MemoryStatus, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![card_header(ICON_MEMORY, "Memory", theme)];
    lines.push(metric_bar_line(theme, "Used", mem.used_percent));
    let free_pct = if mem.total > 0 {
        (mem.available as f64 / mem.total as f64) * 100.0
    } else {
        0.0
    };
    lines.push(metric_bar_line(theme, "Free", free_pct));
    let has_swap = mem.swap_total > 0 || mem.swap_used > 0;
    if has_swap {
        let swap_pct = if mem.swap_total > 0 {
            (mem.swap_used as f64 / mem.swap_total as f64) * 100.0
        } else {
            0.0
        };
        lines.push(metric_bar_line(theme, "Swap", swap_pct));
        lines.push(line_pair(
            theme,
            "Total",
            &format!(
                "{} / {} · Avail {}",
                format_bytes_bin(mem.used),
                format_bytes_bin(mem.total),
                format_bytes_bin(mem.available)
            ),
        ));
    } else {
        lines.push(line_pair(
            theme,
            "Total",
            &format!(
                "{} / {}",
                format_bytes_bin(mem.used),
                format_bytes_bin(mem.total)
            ),
        ));
        if mem.cached > 0 {
            lines.push(line_pair(
                theme,
                "Cache",
                &format!(
                    "{} · Avail {}",
                    format_bytes_bin(mem.cached),
                    format_bytes_bin(mem.available)
                ),
            ));
        } else {
            lines.push(line_pair(theme, "Avail", &format_bytes_bin(mem.available)));
        }
    }
    if !mem.pressure.is_empty() {
        let style = match mem.pressure.as_str() {
            "warn" => theme.warn,
            "critical" => theme.danger,
            _ => theme.ok,
        };
        lines.push(Line::from(Span::styled(
            format!("Status {}", mem.pressure),
            style,
        )));
    }
    lines
}

fn render_disk_card(
    disks: &[DiskStatus],
    read_rate: f64,
    write_rate: f64,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = vec![card_header(ICON_DISK, "Disk", theme)];
    if disks.is_empty() {
        lines.push(Line::from(Span::styled(
            "Collecting...".to_string(),
            theme.subtle,
        )));
    } else {
        let (internal, external): (Vec<_>, Vec<_>) = disks.iter().partition(|d| !d.external);
        let mut push_group = |prefix: &str, list: &[&DiskStatus]| {
            for (i, d) in list.iter().enumerate() {
                let label = if list.len() <= 1 {
                    prefix.to_string()
                } else {
                    format!("{}{}", prefix, i + 1)
                };
                let free = d.total.saturating_sub(d.used);
                lines.push(Line::from(format!(
                    "{:<6} {}  {} used, {} free",
                    label,
                    plain_progress_bar(d.used_percent),
                    format_bytes_bin(d.used),
                    format_bytes_bin(free)
                )));
            }
        };
        push_group("INTR", &internal);
        push_group("EXTR", &external);
        if disks.len() == 1 {
            let d = &disks[0];
            let mut parts = vec![format_bytes_bin(d.total)];
            if !d.fstype.is_empty() {
                parts.push(d.fstype.to_uppercase());
            }
            lines.push(line_pair(theme, "Total", &parts.join(" · ")));
        }
        lines.push(Line::from(format!(
            "SMART  {}",
            format_smart_summary(disks)
        )));
    }
    lines.push(Line::from(format!(
        "I/O    R {:.1} · W {:.1} MB/s",
        read_rate, write_rate
    )));
    lines
}

fn format_smart_summary(disks: &[DiskStatus]) -> String {
    if disks.len() == 1 {
        return smart_label(&disks[0].smart_status, false);
    }
    disks
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let prefix = if d.external { "EXTR" } else { "INTR" };
            format!("{}{} {}", prefix, i + 1, smart_label(&d.smart_status, true))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn smart_label(status: &str, compact: bool) -> String {
    match status {
        "Verified" | "verified" | "OK" | "ok" => {
            if compact {
                "OK".into()
            } else {
                "Verified".into()
            }
        }
        "Failing" | "failing" | "FAIL" => {
            if compact {
                "FAIL".into()
            } else {
                "Failing".into()
            }
        }
        "Unsupported" | "unsupported" => {
            if compact {
                "N/A".into()
            } else {
                "Unsupported".into()
            }
        }
        "" => {
            if compact {
                "?".into()
            } else {
                "Unknown".into()
            }
        }
        other => other.to_string(),
    }
}

fn render_power_card(
    batts: &[BatteryStatus],
    thermal: &ThermalStatus,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = vec![card_header(ICON_BATTERY, "Power", theme)];
    if batts.is_empty() {
        lines.push(Line::from(Span::styled(
            "No battery".to_string(),
            theme.subtle,
        )));
        return lines;
    }
    let b = &batts[0];
    lines.push(metric_bar_line(theme, "Level", b.percent));
    if b.capacity > 0 {
        lines.push(metric_bar_line(theme, "Health", b.capacity as f64));
    }
    let mut summary = b.status.clone();
    if !b.time_left.is_empty() && b.time_left != "0:00" {
        summary.push_str(" · ");
        summary.push_str(&b.time_left);
    }
    if b.cycle_count > 0 {
        summary.push_str(&format!(" · {} cycles", b.cycle_count));
    }
    if thermal.battery_temp > 0.0 {
        summary.push_str(&format!(" · {:.1}°C", thermal.battery_temp));
    }
    lines.push(Line::from(Span::styled(summary, theme.value)));
    lines
}

fn render_process_card(
    procs: &[ProcessInfo],
    card_width: u16,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = vec![card_header(ICON_PROCS, "Processes", theme)];
    if procs.is_empty() {
        lines.push(Line::from(Span::styled(
            "Collecting...".to_string(),
            theme.subtle,
        )));
        return lines;
    }
    let wide = card_width >= 46;
    for (i, p) in procs.iter().take(3).enumerate() {
        let bar = if wide {
            plain_progress_bar(p.cpu)
        } else {
            mini_bar(p.cpu)
        };
        let mem = if let Some(bytes) = p.memory_bytes {
            format_bytes_bin(bytes)
        } else if p.memory >= 10.0 {
            format!("M{:.0}%", p.memory)
        } else {
            String::new()
        };
        let mut row = format!("#{:<5} {} {:5.1}% {:>7}", i + 1, bar, p.cpu, mem);
        let remain = (card_width as usize).saturating_sub(row.chars().count() + 1);
        if remain > 0 {
            row.push(' ');
            row.push_str(&shorten(&p.name, remain));
        }
        lines.push(Line::from(row));
    }
    lines
}

fn render_network_card(
    nets: &[NetworkStatus],
    proxy: &ProxyStatus,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = vec![card_header(ICON_NETWORK, "Network", theme)];
    if nets.is_empty() {
        lines.push(Line::from(Span::styled(
            "Collecting...".to_string(),
            theme.subtle,
        )));
        return lines;
    }
    let (rx, tx) = nets.iter().fold((0.0, 0.0), |(rx, tx), n| {
        (rx + n.rx_rate_mbs, tx + n.tx_rate_mbs)
    });
    lines.push(Line::from(format!(
        "Down   {}  {}",
        plain_progress_bar((rx * 10.0).min(100.0)),
        format_rate_mbs(rx)
    )));
    lines.push(Line::from(format!(
        "Up     {}  {}",
        plain_progress_bar((tx * 10.0).min(100.0)),
        format_rate_mbs(tx)
    )));
    let mut info = Vec::new();
    if proxy.enabled {
        info.push(format!("Proxy {}", proxy.kind));
    }
    if let Some(n) = nets.iter().find(|n| n.name == "en0" && !n.ip.is_empty()) {
        info.push(n.ip.clone());
    } else if let Some(n) = nets.iter().find(|n| !n.ip.is_empty()) {
        info.push(n.ip.clone());
    }
    if !info.is_empty() {
        lines.push(Line::from(Span::styled(info.join(" · "), theme.subtle)));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use vole_core::vole_proto::status::HardwareInfo;

    fn sample_snap() -> StatusSnapshot {
        StatusSnapshot {
            health_score: 82,
            health_score_msg: "Good".into(),
            uptime: "1d".into(),
            hardware: HardwareInfo {
                model: "MacBook Pro".into(),
                cpu_model: "M3".into(),
                total_ram: "16 GB".into(),
                disk_size: "1 TB".into(),
                os_version: "macOS 15.0".into(),
                refresh_rate: "120Hz".into(),
            },
            ..Default::default()
        }
    }

    #[test]
    fn header_keeps_health_and_drops_os_when_narrow() {
        let theme = Theme::default();
        let snap = sample_snap();
        let wide = build_status_header(&snap, 120, &theme);
        let wide_s = wide
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(wide_s.starts_with("Status"));
        assert!(wide_s.contains("Health"));

        let narrow = build_status_header(&snap, 36, &theme);
        let narrow_s = narrow
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(narrow_s.contains("Health"));
        assert!(!narrow_s.contains("macOS 15.0"));
    }

    #[test]
    fn alert_formats_focus_process() {
        let text = format_process_alert(&[ProcessAlert {
            pid: 1,
            name: "chrome".into(),
            command: None,
            cpu: 95.5,
            threshold: 80.0,
            window: "30s".into(),
            triggered_at: String::new(),
            status: "active".into(),
        }])
        .unwrap();
        assert!(text.contains("ALERT chrome"));
        assert!(text.contains("95.5%"));
    }

    #[test]
    fn cpu_card_has_total_and_load_labels() {
        let theme = Theme::default();
        let cpu = CpuStatus {
            usage: 12.5,
            load1: 1.0,
            load5: 0.8,
            load15: 0.5,
            logical_cpu: 8,
            ..Default::default()
        };
        let lines = render_cpu_card(&cpu, &ThermalStatus::default(), &theme, 2);
        let joined: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("CPU"));
        assert!(joined.contains("Total"));
        assert!(joined.contains("Load"));
    }

    #[test]
    fn card_row_heights_follow_content_not_equal_stretch() {
        assert_eq!(
            card_row_heights(&[5, 8, 10, 2, 4, 3], true),
            vec![8, 10, 4]
        );
        assert_eq!(card_row_heights(&[5, 8, 3], false), vec![5, 8, 3]);
        assert_eq!(card_row_heights(&[2], true), vec![2]);
    }
}
