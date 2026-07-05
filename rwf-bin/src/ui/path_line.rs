//! Path line rendering
//!
//! Displays the current path for both panes side by side

use super::{parse_color, shorten_path};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
    Frame,
};
use rwf_lib::{model::ActivePane, AppState};

/// Render the path line.
/// `single_pane`: when `Some(pane)`, render only that pane's path at full width
/// (used in SideBySide mode where the other half is the viewer).
pub fn render_path_line(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    single_pane: Option<ActivePane>,
) {
    let tab = state.current_tab();
    let colors = &state.config.display.colors;
    let style = Style::default()
        .fg(parse_color(&colors.filename_label_foreground_color))
        .bg(parse_color(&colors.filename_label_background_color));

    let render_one = |frame: &mut Frame, rect: Rect, pane: ActivePane| {
        let is_active = state.ui.active_pane == pane;
        let pane_model = match pane {
            ActivePane::Left => &tab.left_pane,
            ActivePane::Right => &tab.right_pane,
        };
        let prefix = if is_active { ">" } else { " " };
        let mask = pane_model
            .file_mask
            .as_deref()
            .map(|m| format!(" [{}]", m))
            .unwrap_or_default();
        let display_path = pane_model.current_location.display_path();
        let avail = rect
            .width
            .saturating_sub(prefix.len() as u16 + mask.len() as u16) as usize;
        let shortened = shorten_path(&display_path, avail, "…");
        frame.render_widget(
            Paragraph::new(Span::raw(format!("{}{}{}", prefix, shortened, mask))).style(style),
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
        render_one(frame, halves[0], ActivePane::Left);
        render_one(frame, halves[1], ActivePane::Right);
    }
}
