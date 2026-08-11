//! 基础 TUI 组件与可单测布局 helper。

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use super::design::{color_bucket, Theme};
#[allow(unused_imports)] // re-export for tests / external callers
pub use super::design::{status_footer, status_footer_line};

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
    let (filled, empty) = progress_bar_counts(percent, PROGRESS_BAR_WIDTH);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// mole `miniBar`：5 格 `▮▯`（窄进程条）。
pub fn mini_bar(percent: f64) -> String {
    let filled = ((percent / 20.0) as usize).clamp(0, 5);
    format!("{}{}", "▮".repeat(filled), "▯".repeat(5 - filled))
}

fn progress_bar_counts(percent: f64, width: usize) -> (usize, usize) {
    let pct = percent.clamp(0.0, 100.0);
    let filled = ((pct / 100.0) * width as f64) as usize;
    let filled = filled.min(width);
    (filled, width - filled)
}

/// Split fill/track colors so empty `░` stays visible on light backgrounds.
pub fn progress_bar_spans(theme: &Theme, percent: f64) -> Vec<Span<'static>> {
    let (filled, empty) = progress_bar_counts(percent, PROGRESS_BAR_WIDTH);
    let fill = theme.style_for_bucket(color_bucket(percent));
    let mut spans = vec![Span::raw(" ")];
    if filled > 0 {
        spans.push(Span::styled("█".repeat(filled), fill));
    }
    if empty > 0 {
        spans.push(Span::styled("░".repeat(empty), theme.bar_track));
    }
    spans
}

/// Narrow process bar with split fill/track colors.
pub fn mini_bar_spans(theme: &Theme, percent: f64) -> Vec<Span<'static>> {
    let filled = ((percent / 20.0) as usize).clamp(0, 5);
    let empty = 5 - filled;
    let fill = theme.style_for_bucket(color_bucket(percent));
    let mut spans = vec![Span::raw(" ")];
    if filled > 0 {
        spans.push(Span::styled("▮".repeat(filled), fill));
    }
    if empty > 0 {
        spans.push(Span::styled("▯".repeat(empty), theme.bar_track));
    }
    spans
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyzeFooterMode {
    Directory {
        can_go_back: bool,
        selected_count: usize,
        large_count: usize,
    },
    Top {
        selected_count: usize,
    },
    Filtering,
    DeleteConfirm,
}

#[allow(dead_code)] // kept for honesty/unit tests; UI uses analyze_footer_line
pub fn analyze_footer(mode: AnalyzeFooterMode) -> String {
    match mode {
        AnalyzeFooterMode::Filtering => "Filter: type… Enter apply | Esc clear".to_string(),
        AnalyzeFooterMode::DeleteConfirm => "Enter confirm | Esc cancel".to_string(),
        AnalyzeFooterMode::Top { selected_count } => {
            let del = if selected_count > 0 {
                format!("⌫ Del {selected_count}")
            } else {
                "⌫ Del".to_string()
            };
            format!("↑↓← | Space | / Filter | O Open | P Preview | F File | R Refresh | {del} | Esc Back | Q/Ctrl+C Quit")
        }
        AnalyzeFooterMode::Directory {
            can_go_back,
            selected_count,
            large_count,
        } => {
            let del = if selected_count > 0 {
                format!("⌫ Del {selected_count}")
            } else {
                "⌫ Del".to_string()
            };
            let top = if large_count > 0 {
                format!(" | T Top {large_count}")
            } else {
                String::new()
            };
            let arrows = if can_go_back {
                "↑↓←→"
            } else {
                "↑↓→"
            };
            let esc = if can_go_back {
                "Esc Back | Q/Ctrl+C Quit"
            } else {
                "Esc/Q Quit"
            };
            format!("{arrows} | Space | Enter | / Filter | O Open | P Preview | F File | R Refresh | {del}{top} | {esc}")
        }
    }
}

fn analyze_key(theme: &Theme, key: &str, label: &str) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        key.to_string(),
        theme.primary.add_modifier(Modifier::BOLD),
    )];
    if !label.is_empty() {
        spans.push(Span::styled(format!(" {label}"), theme.value));
    }
    spans
}

fn analyze_sep(theme: &Theme) -> Span<'static> {
    Span::styled(" | ", theme.subtle)
}

/// Styled analyze footer — same copy as `analyze_footer`, keys in primary bold.
pub fn analyze_footer_line(theme: &Theme, mode: AnalyzeFooterMode) -> Line<'static> {
    match mode {
        AnalyzeFooterMode::Filtering => {
            let mut spans = vec![Span::styled("Filter: type… ", theme.value)];
            spans.extend(analyze_key(theme, "Enter", "apply"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "Esc", "clear"));
            Line::from(spans)
        }
        AnalyzeFooterMode::DeleteConfirm => {
            let mut spans = analyze_key(theme, "Enter", "confirm");
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "Esc", "cancel"));
            Line::from(spans)
        }
        AnalyzeFooterMode::Top { selected_count } => {
            let del_label = if selected_count > 0 {
                format!("Del {selected_count}")
            } else {
                "Del".to_string()
            };
            let mut spans = Vec::new();
            spans.extend(analyze_key(theme, "↑↓←", ""));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "Space", ""));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "/", "Filter"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "O", "Open"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "P", "Preview"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "F", "File"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "R", "Refresh"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "⌫", &del_label));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "Esc", "Back"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "Q", ""));
            spans.push(Span::styled("/", theme.subtle));
            spans.extend(analyze_key(theme, "Ctrl+C", "Quit"));
            Line::from(spans)
        }
        AnalyzeFooterMode::Directory {
            can_go_back,
            selected_count,
            large_count,
        } => {
            let del_label = if selected_count > 0 {
                format!("Del {selected_count}")
            } else {
                "Del".to_string()
            };
            let arrows = if can_go_back {
                "↑↓←→"
            } else {
                "↑↓→"
            };
            let mut spans = Vec::new();
            spans.extend(analyze_key(theme, arrows, ""));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "Space", ""));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "Enter", ""));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "/", "Filter"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "O", "Open"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "P", "Preview"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "F", "File"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "R", "Refresh"));
            spans.push(analyze_sep(theme));
            spans.extend(analyze_key(theme, "⌫", &del_label));
            if large_count > 0 {
                spans.push(analyze_sep(theme));
                spans.extend(analyze_key(theme, "T", &format!("Top {large_count}")));
            }
            spans.push(analyze_sep(theme));
            if can_go_back {
                spans.extend(analyze_key(theme, "Esc", "Back"));
                spans.push(analyze_sep(theme));
                spans.extend(analyze_key(theme, "Q", ""));
                spans.push(Span::styled("/", theme.subtle));
                spans.extend(analyze_key(theme, "Ctrl+C", "Quit"));
            } else {
                spans.extend(analyze_key(theme, "Esc", ""));
                spans.push(Span::styled("/", theme.subtle));
                spans.extend(analyze_key(theme, "Q", "Quit"));
            }
            Line::from(spans)
        }
    }
}

pub fn line_pair(theme: &Theme, label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:<6}", label), theme.label),
        Span::styled(format!(" {}", value), theme.value),
    ])
}

pub fn metric_bar_line(theme: &Theme, label: &str, percent: f64) -> Line<'static> {
    let mut spans = vec![Span::styled(format!("{:<6}", label), theme.label)];
    spans.extend(progress_bar_spans(theme, percent));
    spans.push(Span::styled(format!("  {:5.1}%", percent), theme.value));
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    #[test]
    fn plain_progress_bar_bounds() {
        assert_eq!(plain_progress_bar(0.0).chars().count(), 16);
        assert!(plain_progress_bar(0.0).chars().all(|c| c == '░'));
        assert!(plain_progress_bar(100.0).chars().all(|c| c == '█'));
        assert_eq!(
            plain_progress_bar(50.0)
                .chars()
                .filter(|c| *c == '█')
                .count(),
            8
        );
    }

    #[test]
    fn progress_bar_spans_split_fill_and_track() {
        let theme = Theme::universal();
        let spans = progress_bar_spans(&theme, 50.0);
        let fill = theme.style_for_bucket(color_bucket(50.0));
        assert!(
            spans
                .iter()
                .any(|s| s.content.contains('█') && s.style == fill),
            "{spans:?}"
        );
        assert!(
            spans
                .iter()
                .any(|s| s.content.contains('░') && s.style == theme.bar_track),
            "{spans:?}"
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
            vec!["MacBook Pro".into(), "RAM 16 GB".into(), "Disk 1 TB".into()],
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
    fn status_footer_declares_cat_and_cores() {
        let f = status_footer();
        assert!(f.contains('K') || f.contains("Vole"), "{f}");
        assert!(f.contains('C') || f.contains("Cores"), "{f}");
        assert!(f.contains('B') || f.contains("Back"), "{f}");
        assert!(f.contains('Q'), "{f}");
    }

    #[test]
    fn status_footer_line_highlights_keys() {
        let theme = Theme::default();
        let line = status_footer_line(&theme);
        let joined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(joined, status_footer());
        let key_style = theme.primary.add_modifier(Modifier::BOLD);
        assert!(
            line.spans
                .iter()
                .any(|s| s.content == "K" && s.style == key_style),
            "{line:?}"
        );
        assert!(
            line.spans
                .iter()
                .any(|s| s.content == "B" && s.style == key_style),
            "{line:?}"
        );
    }

    #[test]
    fn analyze_footer_declares_wired_keys_only() {
        let f = analyze_footer(AnalyzeFooterMode::Directory {
            can_go_back: true,
            selected_count: 0,
            large_count: 3,
        });
        assert!(f.contains("Space"));
        assert!(f.contains("⌫") || f.contains("Del"));
        assert!(f.contains("O Open"));
        assert!(f.contains("P Preview"));
        assert!(f.contains("/ Filter"));
        assert!(f.contains("T Top"));
        assert!(f.contains("F File"));
        assert!(f.contains("R Refresh"));
        assert!(f.contains("↑↓←→"));
        assert!(!f.contains("S Live"));
        let root = analyze_footer(AnalyzeFooterMode::Directory {
            can_go_back: false,
            selected_count: 0,
            large_count: 0,
        });
        assert!(root.contains("↑↓→"));
        assert!(!root.contains("←"));
        assert!(root.contains("Esc/Q Quit"));
        let top = analyze_footer(AnalyzeFooterMode::Top { selected_count: 0 });
        assert!(top.contains("↑↓←"));
    }

    #[test]
    fn color_bucket_thresholds() {
        use crate::tui::design::ColorBucket;
        assert_eq!(color_bucket(0.0), ColorBucket::Ok);
        assert_eq!(color_bucket(60.0), ColorBucket::Warn);
        assert_eq!(color_bucket(85.0), ColorBucket::Danger);
    }
}
