//! Top separator rendering
//!
//! This module renders the top separator showing drive/share names and marked file stats.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::AppState;

/// Render the top separator
pub fn render_top_separator(frame: &mut Frame, area: Rect, state: &AppState) {
    let tab = state.current_tab();
    
    // Get drive/share names for both panes
    let left_drive = get_drive_name(&tab.left_pane.current_location);
    let right_drive = get_drive_name(&tab.right_pane.current_location);
    
    // Get marked file stats
    let marked_count = state.marking.count();
    let marked_size = if marked_count > 0 {
        state.marking.total_size(&tab.left_pane.entries) + 
        state.marking.total_size(&tab.right_pane.entries)
    } else {
        0
    };
    
    // Build the separator line
    let mut spans = Vec::new();
    
    // Left pane drive/share
    spans.push(Span::styled(
        format!(" {} ", left_drive),
        Style::default().fg(Color::Cyan),
    ));
    
    // Separator
    let separator_width = area.width as usize / 2;
    let padding = separator_width.saturating_sub(left_drive.len() + 2);
    spans.push(Span::raw(" ".repeat(padding)));
    
    // Marked file stats (centered)
    if marked_count > 0 {
        let marked_info = format!("Marked: {} ({}) ", marked_count, format_size(marked_size));
        spans.push(Span::styled(
            marked_info,
            Style::default().fg(Color::Yellow),
        ));
    }
    
    // Right pane drive/share (right-aligned)
    let right_padding = (area.width as usize)
        .saturating_sub(spans.iter().map(|s| s.content.len()).sum::<usize>())
        .saturating_sub(right_drive.len() + 2);
    spans.push(Span::raw(" ".repeat(right_padding)));
    spans.push(Span::styled(
        format!(" {} ", right_drive),
        Style::default().fg(Color::Cyan),
    ));
    
    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));
    
    frame.render_widget(paragraph, area);
}

/// Extract drive or share name from a location
fn get_drive_name(location: &rwf_lib::model::Location) -> String {
    use rwf_lib::model::Location;
    
    match location {
        Location::Local(path) => {
            // Extract drive letter on Windows or root on Unix
            #[cfg(windows)]
            {
                if let Some(prefix) = path.components().next() {
                    use std::path::Component;
                    if let Component::Prefix(prefix_component) = prefix {
                        return format!("{:?}", prefix_component.kind());
                    }
                }
            }
            
            #[cfg(not(windows))]
            {
                return "/".to_string();
            }
            
            "Local".to_string()
        }
        Location::Ssh { host, .. } => {
            format!("SSH: {}", host)
        }
        Location::Cloud { provider, bucket, .. } => {
            format!("{}: {}", provider, bucket)
        }
        Location::Archive { archive_path, .. } => {
            format!("Archive: {}", get_drive_name(archive_path))
        }
    }
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
