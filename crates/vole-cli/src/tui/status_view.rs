//! `status` TUI 渲染。

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::Frame;

use vole_core::vole_proto::status::StatusSnapshot;

use super::theme::Theme;
use super::widgets::{card, line_pair, progress_bar};

pub fn render_status(frame: &mut Frame, snap: &StatusSnapshot, theme: &Theme) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(area);

    let title = format!(
        "vole status — {} — uptime {}",
        snap.hardware.model, snap.uptime
    );
    let header = Line::from(format!(
        "{}  Health {}: {}",
        title, snap.health_score, snap.health_score_msg
    ));
    frame.render_widget(
        ratatui::widgets::Paragraph::new(header).style(theme.title),
        chunks[0],
    );

    frame.render_widget(progress_bar("CPU", snap.cpu.usage), chunks[1]);
    frame.render_widget(progress_bar("Memory", snap.memory.used_percent), chunks[2]);

    let disk_lines: Vec<Line> = snap
        .disks
        .iter()
        .take(6)
        .map(|d| {
            line_pair(
                theme,
                &d.mount,
                &format!(
                    "{:.1}% ({}/{})",
                    d.used_percent,
                    human(d.used),
                    human(d.total)
                ),
            )
        })
        .collect();
    frame.render_widget(card("Disks", disk_lines), chunks[3]);
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
