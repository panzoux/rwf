//! Path line rendering
//!
//! Displays the current path for both panes side by side

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
    Frame,
};
use rwf_lib::{AppState, model::ActivePane};
use super::parse_color;

/// Render the path line showing left and right pane paths
pub fn render_path_line(frame: &mut Frame, area: Rect, state: &AppState) {
    let tab = state.current_tab();
    let is_left_active = state.ui.active_pane == ActivePane::Left;
    let is_right_active = state.ui.active_pane == ActivePane::Right;
    let colors = &state.config.display.colors;
    
    // Split into two halves
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    
    // Left path
    let left_prefix = if is_left_active { ">" } else { " " };
    let left_path = format!("{}{}", left_prefix, tab.left_pane.current_location.display_path());
    let left_para = Paragraph::new(Span::raw(left_path))
        .style(Style::default()
            .fg(parse_color(&colors.filename_label_foreground_color))
            .bg(parse_color(&colors.filename_label_background_color)));
    frame.render_widget(left_para, halves[0]);
    
    // Right path
    let right_prefix = if is_right_active { ">" } else { " " };
    let right_path = format!("{}{}", right_prefix, tab.right_pane.current_location.display_path());
    let right_para = Paragraph::new(Span::raw(right_path))
        .style(Style::default()
            .fg(parse_color(&colors.filename_label_foreground_color))
            .bg(parse_color(&colors.filename_label_background_color)));
    frame.render_widget(right_para, halves[1]);
}
