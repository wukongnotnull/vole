//! `analyze` TUI 渲染（mole `cmd/analyze/view.go` 区块同构）。

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use vole_core::vole_proto::{AnalyzeEntry, AnalyzeOutput};

use super::theme::Theme;
use super::widgets::{
    analyze_footer, analyze_progress_bar, calculate_name_width, calculate_viewport,
    format_bytes_si, format_percent_label, pad_name, shorten,
};

pub fn render_analyze(
    frame: &mut Frame,
    out: &AnalyzeOutput,
    selected: usize,
    scanning: bool,
    theme: &Theme,
    local_snapshots_tip: Option<&str>,
    can_go_back: bool,
) {
    let area = frame.area();
    let tip_h = if local_snapshots_tip.is_some() {
        1u16
    } else {
        0
    };
    let large_h = if out.large_files.is_empty() { 0u16 } else { 5 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(tip_h),
            Constraint::Min(4),
            Constraint::Length(large_h),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(build_analyze_header(out, scanning, theme)),
        chunks[0],
    );

    let mut body_idx = 1usize;
    if let Some(tip) = local_snapshots_tip {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(tip.to_string(), theme.subtle))),
            chunks[body_idx],
        );
        body_idx += 1;
    }

    let list_area = chunks[body_idx];
    body_idx += 1;

    let name_width = calculate_name_width(area.width);
    let viewport = calculate_viewport(area.height, 6);
    let max_size = out
        .entries
        .iter()
        .map(|e| e.size.max(0))
        .max()
        .unwrap_or(1)
        .max(1);

    let offset = selected.saturating_sub(viewport.saturating_sub(1) / 2);
    let end = (offset + viewport).min(out.entries.len());
    let start = offset.min(end);

    let items: Vec<ListItem> = if out.entries.is_empty() {
        let empty = if scanning {
            "  Scanning…"
        } else if out.overview {
            "  Select a location to explore"
        } else {
            "  Empty directory"
        };
        vec![ListItem::new(Line::from(Span::styled(
            empty.to_string(),
            theme.subtle,
        )))]
    } else {
        out.entries[start..end]
            .iter()
            .enumerate()
            .map(|(rel, e)| {
                let idx = start + rel;
                let row = format_analyze_row(
                    e,
                    idx,
                    idx == selected,
                    max_size,
                    out.total_size,
                    name_width,
                    out.overview,
                );
                let style = if idx == selected {
                    theme.selected
                } else {
                    theme.normal
                };
                ListItem::new(Line::from(row)).style(style)
            })
            .collect()
    };
    frame.render_widget(List::new(items), list_area);

    if large_h > 0 {
        let large_lines = build_large_files_block(out, theme);
        frame.render_widget(Paragraph::new(large_lines), chunks[body_idx]);
        body_idx += 1;
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            analyze_footer(can_go_back),
            theme.subtle,
        ))),
        chunks[body_idx],
    );
}

pub fn build_analyze_header(out: &AnalyzeOutput, scanning: bool, theme: &Theme) -> Vec<Line<'static>> {
    if out.overview {
        let mut lines = vec![Line::from(Span::styled(
            "Analyze Disk".to_string(),
            theme.title,
        ))];
        let sub = if scanning {
            "Select a location to explore:  Analyzing disk usage..."
        } else {
            "Select a location to explore:"
        };
        lines.push(Line::from(Span::styled(sub.to_string(), theme.subtle)));
        return lines;
    }

    let mut spans = vec![
        Span::styled("Analyze Disk".to_string(), theme.title),
        Span::styled(format!("  {}", display_path(&out.path)), theme.subtle),
    ];
    if scanning && out.total_size <= 0 {
        spans.push(Span::styled("  — scanning…".to_string(), theme.subtle));
    } else {
        spans.push(Span::styled(
            format!("  |  Total: {}", format_bytes_si(out.total_size.max(0) as u64)),
            theme.value,
        ));
        if let Some(files) = out.total_files {
            spans.push(Span::styled(
                format!("  · {} files", files),
                theme.subtle,
            ));
        }
    }
    vec![Line::from(spans), Line::from("")]
}

pub fn format_analyze_row(
    entry: &AnalyzeEntry,
    idx: usize,
    selected: bool,
    max_size: i64,
    total_size: i64,
    name_width: usize,
    overview: bool,
) -> String {
    let prefix = if selected { " ▶ " } else { "   " };
    let size_val = entry.size.max(0);
    let percent = if total_size > 0 && entry.size >= 0 {
        (entry.size as f64 / total_size as f64) * 100.0
    } else {
        0.0
    };
    let bar = analyze_progress_bar(size_val, max_size);
    let pct = format_percent_label(percent, entry.size >= 0 && total_size > 0);
    let icon = if overview {
        ""
    } else if entry.is_dir {
        "📁 "
    } else {
        "📄 "
    };
    let name = pad_name(&shorten(&entry.name, name_width), name_width);
    let size = if entry.size < 0 {
        " scanning".to_string()
    } else {
        format!("{:>10}", format_bytes_si(entry.size as u64))
    };
    let mut hint = String::new();
    if entry.cleanable {
        hint.push_str("  cleanable");
    } else if let Some(ref last) = entry.last_access {
        if !last.is_empty() {
            hint.push_str("  ");
            hint.push_str(last);
        }
    }
    if overview {
        format!(
            "{}{:>2}. {} {}  |  {}{}{}",
            prefix,
            idx + 1,
            bar,
            pct,
            name,
            size,
            hint
        )
    } else {
        format!(
            "{}{:>2}. {} {}  |  {}{}{}{}",
            prefix,
            idx + 1,
            bar,
            pct,
            icon,
            name,
            size,
            hint
        )
    }
}

fn build_large_files_block(out: &AnalyzeOutput, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        "Large files".to_string(),
        theme.title,
    ))];
    for f in out.large_files.iter().take(4) {
        lines.push(Line::from(format!(
            "  {}  {:>10}",
            shorten(&f.name, 40),
            format_bytes_si(f.size.max(0) as u64)
        )));
    }
    lines
}

fn display_path(path: &str) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        let home = home.to_string_lossy();
        if path.starts_with(home.as_ref()) {
            return path.replacen(home.as_ref(), "~", 1);
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vole_core::vole_proto::AnalyzeFileEntry;

    #[test]
    fn header_directory_mode_has_total() {
        let theme = Theme::default();
        let out = AnalyzeOutput {
            path: "/Users/me/Downloads".into(),
            overview: false,
            total_size: 1_500_000_000,
            total_files: Some(42),
            ..Default::default()
        };
        let lines = build_analyze_header(&out, false, &theme);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("Analyze Disk"));
        assert!(text.contains("Total:"));
        assert!(text.contains("42 files"));
    }

    #[test]
    fn header_overview_prompt() {
        let theme = Theme::default();
        let out = AnalyzeOutput {
            overview: true,
            ..Default::default()
        };
        let lines = build_analyze_header(&out, true, &theme);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("Select a location to explore"));
    }

    #[test]
    fn row_marks_selection_and_percent() {
        let entry = AnalyzeEntry {
            name: "Caches".into(),
            path: "/tmp/Caches".into(),
            size: 50,
            is_dir: true,
            insight: false,
            cleanable: true,
            last_access: None,
        };
        let row = format_analyze_row(&entry, 0, true, 100, 100, 12, false);
        assert!(row.contains('▶'));
        assert!(row.contains("Caches"));
        assert!(row.contains("50.0%") || row.contains("50.0"));
        assert!(row.contains("cleanable"));
        let unsel = format_analyze_row(&entry, 2, false, 100, 100, 12, false);
        assert!(!unsel.contains('▶'));
        assert!(unsel.contains(" 3."));
    }

    #[test]
    fn footer_omits_unwired_actions() {
        let f = analyze_footer(true);
        assert!(!f.contains("Space"));
        assert!(!f.contains("Del"));
        assert!(f.contains("Enter"));
    }

    #[test]
    fn large_files_block_lists_names() {
        let theme = Theme::default();
        let out = AnalyzeOutput {
            large_files: vec![AnalyzeFileEntry {
                name: "big.dmg".into(),
                path: "/tmp/big.dmg".into(),
                size: 2_000_000_000,
            }],
            ..Default::default()
        };
        let lines = build_large_files_block(&out, &theme);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("Large files"));
        assert!(text.contains("big.dmg"));
    }

    #[test]
    fn size_tone_buckets() {
        use super::super::theme::{size_tone, SizeTone};
        assert_eq!(size_tone(50.0), SizeTone::High);
        assert_eq!(size_tone(20.0), SizeTone::Mid);
        assert_eq!(size_tone(5.0), SizeTone::Low);
        assert_eq!(size_tone(1.0), SizeTone::Quiet);
    }
}
