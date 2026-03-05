//! Volume name line rendering
//!
//! Displays volume names and marked file statistics for both panes
//! **Validates: Requirements 39A.1-39A.14**

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
    Frame,
};
use rwf_lib::{AppState, get_drive_or_share_name, calculate_marked_stats, format_top_separator_info};
use super::parse_color;

/// Render the volume name line showing drive/volume names and marked stats for both panes
/// **Validates: Requirements 39A.1, 39A.13, 39A.14**
pub fn render_volume_line(frame: &mut Frame, area: Rect, state: &AppState) {
    let tab = state.current_tab();
    let colors = &state.config.display.colors;
    
    // Get volume names
    let left_volume = get_drive_or_share_name(&tab.left_pane.current_location);
    let right_volume = get_drive_or_share_name(&tab.right_pane.current_location);
    
    // Calculate marked stats
    let left_stats = calculate_marked_stats(&tab.left_pane.entries, &state.marking);
    let right_stats = calculate_marked_stats(&tab.right_pane.entries, &state.marking);
    
    // Format separator info
    let left_info = format_top_separator_info(&left_volume, &left_stats);
    let right_info = format_top_separator_info(&right_volume, &right_stats);
    
    // Split into two halves
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    
    // Left volume with marked stats
    let left_para = Paragraph::new(Span::raw(format!(" {}", left_info)))
        .style(Style::default()
            .fg(parse_color(&colors.top_separator_foreground_color))
            .bg(parse_color(&colors.top_separator_background_color)));
    frame.render_widget(left_para, halves[0]);
    
    // Right volume with marked stats
    let right_para = Paragraph::new(Span::raw(format!(" {}", right_info)))
        .style(Style::default()
            .fg(parse_color(&colors.top_separator_foreground_color))
            .bg(parse_color(&colors.top_separator_background_color)));
    frame.render_widget(right_para, halves[1]);
}
