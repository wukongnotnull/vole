//! 基础 TUI 组件与可单测布局 helper。

use ratatui::text::{Line, Span};

use super::theme::{color_bucket, Theme};

pub const PROGRESS_BAR_WIDTH: usize = 16;
pub const STATUS_NARROW_MAX: u16 = 80;
pub const ANALYZE_BAR_WIDTH: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLayoutMode {
    Single,
    TwoColumn,
}

pub fn status_layout_mode(width: u16) -> StatusLayoutMode {
    if width > STATUS_NARROW_MAX {
        StatusLayoutMode::TwoColumn
    } else {
        StatusLayoutMode::Single
    }
}

/// mole `plainProgressBar`：16 格 `█░`。
pub fn plain_progress_bar(percent: f64) -> String {
    let pct = percent.clamp(0.0, 100.0);
    let filled = ((pct / 100.0) * PROGRESS_BAR_WIDTH as f64) as usize;
    let filled = filled.min(PROGRESS_BAR_WIDTH);
    format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(PROGRESS_BAR_WIDTH - filled)
    )
}

/// mole `miniBar`：5 格 `▮▯`（窄进程条）。
pub fn mini_bar(percent: f64) -> String {
    let filled = ((percent / 20.0) as usize).clamp(0, 5);
    format!("{}{}", "▮".repeat(filled), "▯".repeat(5 - filled))
}

/// mole analyze `coloredProgressBar` 的无色骨架（24 宽）。
pub fn analyze_progress_bar(value: i64, max_value: i64) -> String {
    if value <= 0 || max_value <= 0 {
        return " ".repeat(ANALYZE_BAR_WIDTH);
    }
    let filled = ((value as i128 * ANALYZE_BAR_WIDTH as i128) / max_value as i128)
        .clamp(0, ANALYZE_BAR_WIDTH as i128) as usize;
    if filled == 0 {
        return format!("▏{}", " ".repeat(ANALYZE_BAR_WIDTH - 1));
    }
    format!(
        "{}{}",
        "█".repeat(filled),
        " ".repeat(ANALYZE_BAR_WIDTH - filled)
    )
}

pub fn format_percent_label(percent: f64, known: bool) -> String {
    if !known {
        return "  --  ".to_string();
    }
    let label = if percent > 0.0 && percent < 0.1 {
        "< 0.1%".to_string()
    } else {
        format!("{:.1}%", percent)
    };
    format!("{:>6}", label)
}

pub fn format_bytes_bin(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    let v = bytes as f64;
    if v >= TIB {
        format!("{:.1} TiB", v / TIB)
    } else if v >= GIB {
        format!("{:.1} GiB", v / GIB)
    } else if v >= MIB {
        format!("{:.1} MiB", v / MIB)
    } else if v >= KIB {
        format!("{:.0} KiB", v / KIB)
    } else {
        format!("{} B", bytes)
    }
}

/// mole analyze `units.BytesSI` 口径（近似）。
pub fn format_bytes_si(bytes: u64) -> String {
    const KB: f64 = 1000.0;
    const MB: f64 = KB * 1000.0;
    const GB: f64 = MB * 1000.0;
    const TB: f64 = GB * 1000.0;
    let v = bytes as f64;
    if v >= TB {
        format!("{:.1} TB", v / TB)
    } else if v >= GB {
        format!("{:.1} GB", v / GB)
    } else if v >= MB {
        format!("{:.1} MB", v / MB)
    } else if v >= KB {
        format!("{:.0} KB", v / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_rate_mbs(mb: f64) -> String {
    if mb < 0.01 {
        "0 MB/s".to_string()
    } else if mb < 1.0 {
        format!("{:.2} MB/s", mb)
    } else if mb < 10.0 {
        format!("{:.1} MB/s", mb)
    } else {
        format!("{:.0} MB/s", mb)
    }
}

pub fn shorten(s: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_len {
        return s.to_string();
    }
    if max_len == 1 {
        return "…".to_string();
    }
    chars.into_iter().take(max_len - 1).collect::<String>() + "…"
}

pub fn pad_name(name: &str, width: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() >= width {
        return chars.into_iter().take(width).collect();
    }
    format!("{}{}", name, " ".repeat(width - chars.len()))
}

/// mole `calculateViewport`：reserved 后 clamp 1..=30。
pub fn calculate_viewport(term_height: u16, reserved: u16) -> usize {
    if term_height == 0 {
        return 12;
    }
    let available = term_height.saturating_sub(reserved) as usize;
    available.clamp(1, 30)
}

/// mole `calculateNameWidth`：termWidth − 61，clamp 24..=60。
pub fn calculate_name_width(term_width: u16) -> usize {
    const FIXED: i32 = 61;
    let available = term_width as i32 - FIXED;
    if available < 24 {
        24
    } else if available > 60 {
        60
    } else {
        available as usize
    }
}

/// 窄终端 header：按候选组尝试拼接，直到 `head + parts` 宽度合适。
pub fn fit_status_header(head: &str, candidates: &[Vec<String>], width: usize) -> String {
    if width == 0 {
        return head.to_string();
    }
    if display_width(head) > width {
        return truncate_to_width(head, width);
    }
    for parts in candidates {
        if parts.is_empty() {
            continue;
        }
        let joined = format!("{}  {}", head, parts.join(" · "));
        if display_width(&joined) <= width {
            return joined;
        }
    }
    head.to_string()
}

pub fn display_width(s: &str) -> usize {
    s.chars().count()
}

fn truncate_to_width(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
}

pub fn status_footer() -> String {
    "Q/Esc/Ctrl+C Quit".to_string()
}

pub fn analyze_footer(can_go_back: bool) -> String {
    if can_go_back {
        "↑↓ | Enter | Esc Back | Q/Ctrl+C Quit".to_string()
    } else {
        "↑↓ | Enter | Esc/Q Quit".to_string()
    }
}

pub fn line_pair(theme: &Theme, label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:<6}", label), theme.label),
        Span::styled(format!(" {}", value), theme.value),
    ])
}

pub fn metric_bar_line(theme: &Theme, label: &str, percent: f64) -> Line<'static> {
    let bar = plain_progress_bar(percent);
    let bucket = color_bucket(percent);
    Line::from(vec![
        Span::styled(format!("{:<6}", label), theme.label),
        Span::styled(format!(" {}", bar), theme.style_for_bucket(bucket)),
        Span::styled(format!("  {:5.1}%", percent), theme.value),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_progress_bar_bounds() {
        assert_eq!(plain_progress_bar(0.0).chars().count(), 16);
        assert!(plain_progress_bar(0.0).chars().all(|c| c == '░'));
        assert!(plain_progress_bar(100.0).chars().all(|c| c == '█'));
        assert_eq!(
            plain_progress_bar(50.0).chars().filter(|c| *c == '█').count(),
            8
        );
    }

    #[test]
    fn layout_mode_threshold() {
        assert_eq!(status_layout_mode(80), StatusLayoutMode::Single);
        assert_eq!(status_layout_mode(81), StatusLayoutMode::TwoColumn);
    }

    #[test]
    fn fit_header_drops_low_priority() {
        let head = "Status  Health ● 80";
        let candidates = vec![
            vec![
                "MacBook Pro".into(),
                "RAM 16 GB".into(),
                "Disk 1 TB".into(),
                "macOS 15".into(),
                "up 10d".into(),
            ],
            vec![
                "MacBook Pro".into(),
                "RAM 16 GB".into(),
                "Disk 1 TB".into(),
            ],
            vec!["RAM 16 GB".into(), "Disk 1 TB".into()],
        ];
        let wide = fit_status_header(head, &candidates, 120);
        assert!(wide.contains("macOS 15"));
        let mid = fit_status_header(head, &candidates, 55);
        assert!(mid.contains("RAM 16 GB"));
        assert!(!mid.contains("macOS 15"));
        let narrow = fit_status_header(head, &candidates, 30);
        assert_eq!(narrow, head);
    }

    #[test]
    fn viewport_and_name_width_clamp() {
        assert_eq!(calculate_viewport(10, 6), 4);
        assert_eq!(calculate_viewport(3, 6), 1);
        assert_eq!(calculate_viewport(100, 6), 30);
        assert_eq!(calculate_name_width(50), 24);
        assert_eq!(calculate_name_width(200), 60);
        assert_eq!(calculate_name_width(100), 39);
    }

    #[test]
    fn analyze_footer_modes() {
        assert!(analyze_footer(false).contains("Esc/Q Quit"));
        assert!(analyze_footer(true).contains("Esc Back"));
        assert!(!analyze_footer(true).contains("Space Select"));
    }

    #[test]
    fn color_bucket_thresholds() {
        assert_eq!(color_bucket(0.0), super::super::theme::ColorBucket::Ok);
        assert_eq!(color_bucket(60.0), super::super::theme::ColorBucket::Warn);
        assert_eq!(color_bucket(85.0), super::super::theme::ColorBucket::Danger);
    }
}
