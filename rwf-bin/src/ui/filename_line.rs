//! Filename line rendering
//!
//! This module renders the filename line showing the selected file in the active pane.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::AppState;
use super::{parse_color, smart_truncate};

/// Render the filename line
pub fn render_filename_line(frame: &mut Frame, area: Rect, state: &AppState) {
    let active_pane = state.active_pane();
    let colors = &state.config.display.colors;
    let ellipsis = &state.config.ellipsis;
    
    // Get the current entry (at cursor position)
    let filename = if let Some(entry) = active_pane.current_entry() {
        entry.name.clone()
    } else {
        String::new()
    };
    
    // Show full filename using entire line width
    // Truncate if too long to fit in the area
    let max_width = area.width.saturating_sub(2) as usize; // Leave space for padding
    let display_name = smart_truncate(&filename, max_width, ellipsis);
    
    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", display_name),
            Style::default()
                .fg(parse_color(&colors.filename_label_foreground_color))
                .bg(parse_color(&colors.filename_label_background_color)),
        ),
    ]);
    
    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(parse_color(&colors.filename_label_background_color)));
    frame.render_widget(paragraph, area);
}
