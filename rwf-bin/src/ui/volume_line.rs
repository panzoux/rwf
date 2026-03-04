//! Volume name line rendering
//!
//! Displays volume names for both panes

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
    Frame,
};
use rwf_lib::{AppState, model::Location};
use std::path::Component;
use super::parse_color;

#[cfg(windows)]
use std::path::Prefix;

/// Render the volume name line showing drive/volume names for both panes
pub fn render_volume_line(frame: &mut Frame, area: Rect, state: &AppState) {
    let tab = state.current_tab();
    let colors = &state.config.display.colors;
    
    // Get volume names
    let left_volume = get_volume_name(&tab.left_pane.current_location);
    let right_volume = get_volume_name(&tab.right_pane.current_location);
    
    // Split into two halves
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    
    // Left volume
    let left_para = Paragraph::new(Span::raw(format!(" {}", left_volume)))
        .style(Style::default()
            .fg(parse_color(&colors.directory_color))
            .bg(parse_color(&colors.background_color)));
    frame.render_widget(left_para, halves[0]);
    
    // Right volume
    let right_para = Paragraph::new(Span::raw(format!(" {}", right_volume)))
        .style(Style::default()
            .fg(parse_color(&colors.directory_color))
            .bg(parse_color(&colors.background_color)));
    frame.render_widget(right_para, halves[1]);
}

/// Get volume name for a location
fn get_volume_name(location: &Location) -> String {
    match location {
        Location::Local(path) => {
            #[cfg(windows)]
            {
                if let Some(Component::Prefix(prefix)) = path.components().next() {
                    match prefix.kind() {
                        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                            return format!("{}:", (letter as char).to_uppercase());
                        }
                        _ => {}
                    }
                }
            }
            "Local".to_string()
        }
        Location::Ssh { host, .. } => format!("SSH: {}", host),
        Location::Cloud { provider, bucket, .. } => format!("{}: {}", provider, bucket),
        Location::Archive { archive_path, .. } => {
            format!("Archive: {}", get_volume_name(archive_path))
        }
    }
}
