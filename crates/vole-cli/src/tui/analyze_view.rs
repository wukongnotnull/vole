//! `analyze` TUI 渲染（mole `cmd/analyze/view.go` 区块同构）。

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use vole_core::vole_proto::{AnalyzeEntry, AnalyzeOutput};

use super::theme::Theme;
use super::widgets::{
    analyze_footer, analyze_progress_bar, calculate_name_width, calculate_viewport,
    format_bytes_si, format_percent_label, pad_name, shorten, AnalyzeFooterMode,
};

pub struct AnalyzeRenderOpts<'a> {
    pub selected: usize,
    pub scanning: bool,
    pub local_snapshots_tip: Option<&'a str>,
    pub can_go_back: bool,
    pub show_large_files: bool,
    pub multi_selected: &'a std::collections::BTreeSet<String>,
    pub large_multi_selected: &'a std::collections::BTreeSet<String>,
    pub footer_mode: AnalyzeFooterMode,
    pub status: &'a str,
    pub entry_filter: &'a str,
    pub large_filter: &'a str,
}

pub fn render_analyze(
    frame: &mut Frame,
    out: &AnalyzeOutput,
    theme: &Theme,
    opts: &AnalyzeRenderOpts<'_>,
) {
    let area = frame.area();
    let tip_h = if opts.local_snapshots_tip.is_some() {
        1u16
    } else {
        0
    };
    let status_h = if opts.status.is_empty() { 0u16 } else { 1 };
    let filter_h = if (!opts.show_large_files && !opts.entry_filter.is_empty())
        || (opts.show_large_files && !opts.large_filter.is_empty())
        || matches!(opts.footer_mode, AnalyzeFooterMode::Filtering)
    {
        1u16
    } else {
        0
    };
    let large_h = if !opts.show_large_files && !out.large_files.is_empty() {
        5u16
    } else {
        0
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(tip_h),
            Constraint::Length(filter_h),
            Constraint::Length(status_h),
            Constraint::Min(4),
            Constraint::Length(large_h),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(build_analyze_header(out, opts.scanning, theme)),
        chunks[0],
    );

    let mut body_idx = 1usize;
    if let Some(tip) = opts.local_snapshots_tip {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(tip.to_string(), theme.subtle))),
            chunks[body_idx],
        );
        body_idx += 1;
    }

    if filter_h > 0 {
        let (label, q) = if opts.show_large_files {
            ("Filter", opts.large_filter)
        } else {
            ("Filter", opts.entry_filter)
        };
        let cursor = if matches!(opts.footer_mode, AnalyzeFooterMode::Filtering) {
            "█"
        } else {
            ""
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {label}: {q}{cursor}"),
                theme.primary,
            ))),
            chunks[body_idx],
        );
        body_idx += 1;
    }

    if status_h > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                opts.status.to_string(),
                theme.warn,
            ))),
            chunks[body_idx],
        );
        body_idx += 1;
    }

    let list_area = chunks[body_idx];
    body_idx += 1;

    let name_width = calculate_name_width(area.width);
    let viewport = calculate_viewport(area.height, 6);

    let items: Vec<ListItem> = if opts.show_large_files {
        render_large_items(out, opts, name_width, viewport, theme)
    } else {
        render_entry_items(out, opts, name_width, viewport, theme)
    };
    frame.render_widget(List::new(items), list_area);

    if large_h > 0 {
        let large_lines = build_large_files_block(out, theme);
        frame.render_widget(Paragraph::new(large_lines), chunks[body_idx]);
        body_idx += 1;
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            analyze_footer(opts.footer_mode),
            theme.subtle,
        ))),
        chunks[body_idx],
    );
}

fn render_entry_items(
    out: &AnalyzeOutput,
    opts: &AnalyzeRenderOpts<'_>,
    name_width: usize,
    viewport: usize,
    theme: &Theme,
) -> Vec<ListItem<'static>> {
    let q = opts.entry_filter.to_lowercase();
    let entries: Vec<&AnalyzeEntry> = out
        .entries
        .iter()
        .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
        .collect();
    if entries.is_empty() {
        let empty = if opts.scanning {
            "  Scanning…"
        } else if out.overview {
            "  Select a location to explore"
        } else if !opts.entry_filter.is_empty() {
            "  No matches"
        } else {
            "  Empty directory"
        };
        return vec![ListItem::new(Line::from(Span::styled(
            empty.to_string(),
            theme.subtle,
        )))];
    }
    let max_size = entries
        .iter()
        .map(|e| e.size.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let offset = opts
        .selected
        .saturating_sub(viewport.saturating_sub(1) / 2);
    let end = (offset + viewport).min(entries.len());
    let start = offset.min(end);
    let show_marks = !opts.multi_selected.is_empty();
    entries[start..end]
        .iter()
        .enumerate()
        .map(|(rel, e)| {
            let idx = start + rel;
            let multi = if show_marks {
                Some(opts.multi_selected.contains(&e.path))
            } else {
                None
            };
            let row = format_analyze_row(
                e,
                idx,
                idx == opts.selected,
                max_size,
                out.total_size,
                name_width,
                out.overview,
                multi,
            );
            let style = if idx == opts.selected {
                theme.selected
            } else {
                theme.normal
            };
            ListItem::new(Line::from(row)).style(style)
        })
        .collect()
}

fn render_large_items(
    out: &AnalyzeOutput,
    opts: &AnalyzeRenderOpts<'_>,
    name_width: usize,
    viewport: usize,
    theme: &Theme,
) -> Vec<ListItem<'static>> {
    let q = opts.large_filter.to_lowercase();
    let files: Vec<_> = out
        .large_files
        .iter()
        .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
        .collect();
    if files.is_empty() {
        return vec![ListItem::new(Line::from(Span::styled(
            "  No large files".to_string(),
            theme.subtle,
        )))];
    }
    let max_size = files
        .iter()
        .map(|e| e.size.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let offset = opts
        .selected
        .saturating_sub(viewport.saturating_sub(1) / 2);
    let end = (offset + viewport).min(files.len());
    let start = offset.min(end);
    let show_marks = !opts.large_multi_selected.is_empty();
    files[start..end]
        .iter()
        .enumerate()
        .map(|(rel, f)| {
            let idx = start + rel;
            let prefix = if show_marks {
                if opts.large_multi_selected.contains(&f.path) {
                    " ● "
                } else {
                    " ○ "
                }
            } else if idx == opts.selected {
                " ▶ "
            } else {
                "   "
            };
            let name = pad_name(&shorten(&f.name, name_width), name_width);
            let size = format!("{:>10}", format_bytes_si(f.size.max(0) as u64));
            let bar = analyze_progress_bar(f.size.max(0), max_size);
            let row = format!("{prefix}{:>2}. {bar}  |  📄 {name}{size}", idx + 1);
            let style = if idx == opts.selected {
                theme.selected
            } else {
                theme.normal
            };
            ListItem::new(Line::from(row)).style(style)
        })
        .collect()
}

pub fn build_analyze_header(
    out: &AnalyzeOutput,
    scanning: bool,
    theme: &Theme,
) -> Vec<Line<'static>> {
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
            format!(
                "  |  Total: {}",
                format_bytes_si(out.total_size.max(0) as u64)
            ),
            theme.value,
        ));
        if let Some(files) = out.total_files {
            spans.push(Span::styled(format!("  · {} files", files), theme.subtle));
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
    multi_marked: Option<bool>,
) -> String {
    let prefix = match multi_marked {
        Some(true) => " ● ",
        Some(false) => " ○ ",
        None if selected => " ▶ ",
        None => "   ",
    };
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
        let row = format_analyze_row(&entry, 0, true, 100, 100, 12, false, None);
        assert!(row.contains('▶'));
        assert!(row.contains("Caches"));
        assert!(row.contains("50.0%") || row.contains("50.0"));
        assert!(row.contains("cleanable"));
        let unsel = format_analyze_row(&entry, 2, false, 100, 100, 12, false, None);
        assert!(!unsel.contains('▶'));
        assert!(unsel.contains(" 3."));
    }

    #[test]
    fn row_shows_multi_select_marks() {
        let entry = AnalyzeEntry {
            name: "Caches".into(),
            path: "/tmp/Caches".into(),
            size: 50,
            is_dir: true,
            ..Default::default()
        };
        let marked = format_analyze_row(&entry, 0, true, 100, 100, 12, false, Some(true));
        assert!(marked.contains('●'));
        let unmarked = format_analyze_row(&entry, 0, false, 100, 100, 12, false, Some(false));
        assert!(unmarked.contains('○'));
    }

    #[test]
    fn footer_declares_wired_keys() {
        let f = analyze_footer(AnalyzeFooterMode::Directory {
            can_go_back: true,
            selected_count: 0,
            large_count: 1,
        });
        assert!(f.contains("Space"));
        assert!(f.contains("Enter"));
        assert!(!f.contains("F File"));
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
