//! `analyze` TUI 渲染。

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use vole_core::vole_proto::AnalyzeOutput;

use super::theme::Theme;

pub fn render_analyze(
    frame: &mut Frame,
    out: &AnalyzeOutput,
    selected: usize,
    scanning: bool,
    theme: &Theme,
    local_snapshots_tip: Option<&str>,
) {
    let area = frame.area();
    let tip_h = if local_snapshots_tip.is_some() { 1u16 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(tip_h),
            Constraint::Min(6),
            Constraint::Length(5),
        ])
        .split(area);

    let header = if scanning {
        Line::from(vec![
            Span::styled("vole analyze — ", theme.title),
            Span::raw(out.path.clone()),
            Span::raw(" — scanning…"),
        ])
    } else {
        Line::from(vec![
            Span::styled("vole analyze — ", theme.title),
            Span::raw(out.path.clone()),
            Span::raw(format!(
                " — {} files, {}",
                out.total_files.unwrap_or(0),
                human(out.total_size as u64)
            )),
        ])
    };
    frame.render_widget(Paragraph::new(header), chunks[0]);

    if let Some(tip) = local_snapshots_tip {
        frame.render_widget(Paragraph::new(Line::from(tip.to_string())), chunks[1]);
    }

    let items: Vec<ListItem> = out
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let prefix = if e.is_dir { "▸ " } else { "  " };
            let label = format!("{}{} {}", prefix, e.name, human(e.size as u64));
            let style = if i == selected {
                theme.selected
            } else {
                theme.normal
            };
            ListItem::new(Line::from(label)).style(style)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, chunks[2]);

    let large_lines: Vec<Line> = out
        .large_files
        .iter()
        .take(4)
        .map(|f| Line::from(format!("{} {}", f.name, human(f.size as u64))))
        .collect();
    let large_title = Line::from(Span::styled("Large files", theme.title));
    let mut block_lines = vec![large_title];
    block_lines.extend(large_lines);
    if out.large_files.is_empty() {
        block_lines.push(Line::from("—"));
    }
    frame.render_widget(Paragraph::new(block_lines), chunks[3]);
}

fn human(bytes: u64) -> String {
    if bytes >= 1_000_000_000_000 {
        format!("{:.1}TB", bytes as f64 / 1_000_000_000_000.0)
    } else if bytes >= 1_000_000_000 {
        format!("{:.1}GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1000 {
        format!("{}KB", bytes / 1000)
    } else {
        format!("{}B", bytes)
    }
}
