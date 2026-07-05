//! Volume name line rendering
//!
//! Displays volume names and marked file statistics for both panes
//! **Validates: Requirements 39A.1-39A.14**

use super::parse_color;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
    Frame,
};
use rwf_lib::{
    calculate_marked_stats, format_top_separator_info, get_drive_or_share_name, AppState,
};

/// Render the volume name line.
/// `single_pane`: when `Some(pane)`, render only that pane at full width
/// (used in SideBySide mode where the other half is the viewer).
pub fn render_volume_line(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    single_pane: Option<rwf_lib::model::ActivePane>,
) {
    let tab = state.current_tab();
    let colors = &state.config.display.colors;
    let style = Style::default()
        .fg(parse_color(&colors.top_separator_foreground_color))
        .bg(parse_color(&colors.top_separator_background_color));

    let render_one = |frame: &mut Frame, rect: Rect, pane: rwf_lib::model::ActivePane| {
        let pane_model = match pane {
            rwf_lib::model::ActivePane::Left => &tab.left_pane,
            rwf_lib::model::ActivePane::Right => &tab.right_pane,
        };
        let volume = get_drive_or_share_name(&pane_model.current_location);
        let stats = calculate_marked_stats(&pane_model.entries, &pane_model.marking);
        let info = format_top_separator_info(&volume, &stats);
        frame.render_widget(
            Paragraph::new(Span::raw(format!(" {}", info))).style(style),
            rect,
        );
    };

    if let Some(pane) = single_pane {
        render_one(frame, area, pane);
    } else {
        let halves = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        render_one(frame, halves[0], rwf_lib::model::ActivePane::Left);
        render_one(frame, halves[1], rwf_lib::model::ActivePane::Right);
    }
}
