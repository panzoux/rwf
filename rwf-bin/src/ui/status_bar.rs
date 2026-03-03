//! Status bar rendering
//!
//! This module renders the status bar at the bottom of the screen.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::AppState;

/// Render the status bar
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let _tab = state.current_tab();
    let active_pane = state.active_pane();

    // Build status bar content
    let mut spans = Vec::new();

    // Current directory
    spans.push(Span::styled(
        format!(" {} ", active_pane.current_location.display_path()),
        Style::default().fg(Color::White),
    ));

    // File count
    let file_count = active_pane.entries.len();
    spans.push(Span::styled(
        format!("| {} files ", file_count),
        Style::default().fg(Color::Gray),
    ));

    // Marked files
    let marked_count = state.marking.count();
    if marked_count > 0 {
        let marked_size = state.marking.total_size(&active_pane.entries);
        spans.push(Span::styled(
            format!("| {} marked ({}) ", marked_count, format_size(marked_size)),
            Style::default().fg(Color::Yellow),
        ));
    }

    // Active jobs
    let active_jobs = state.jobs.active.len();
    if active_jobs > 0 {
        spans.push(Span::styled(
            format!("| {} jobs ", active_jobs),
            Style::default().fg(Color::Cyan),
        ));
    }

    // Sort mode
    let sort_mode = match active_pane.sort_mode {
        rwf_lib::model::SortMode::Name => "Name",
        rwf_lib::model::SortMode::Size => "Size",
        rwf_lib::model::SortMode::Date => "Date",
        rwf_lib::model::SortMode::Extension => "Ext",
    };
    spans.push(Span::styled(
        format!("| Sort: {} ", sort_mode),
        Style::default().fg(Color::Gray),
    ));

    // File mask
    if let Some(mask) = &active_pane.file_mask {
        spans.push(Span::styled(
            format!("| Filter: {} ", mask),
            Style::default().fg(Color::Green),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes < TB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    }
}
