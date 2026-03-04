//! UI rendering module
//!
//! This module handles all UI rendering using ratatui.

mod panes;
mod tab_bar;
mod task_panel;
mod filename_line;
mod path_line;
mod volume_line;
mod pane_info_line;
mod colors;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};
use rwf_lib::AppState;

pub use panes::render_panes;
pub use tab_bar::render_tab_bar;
pub use task_panel::render_task_panel;
pub use filename_line::render_filename_line;
pub use path_line::render_path_line;
pub use volume_line::render_volume_line;
pub use pane_info_line::render_pane_info_line;
pub use colors::parse_color;

/// Main UI rendering function
pub fn render_ui(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Create main layout matching TWF exactly:
    // Tabs (1 line) → Path line (1 line) → Volume name line (1 line) → 
    // File panes (Min 10, NO BORDERS) → Pane info line (1 line) → 
    // Selected filename line (1 line) → Task view (5 lines, NO BORDER)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if state.ui.layout.show_tab_bar { 1 } else { 0 }), // Tabs
            Constraint::Length(1), // Path line (left | right)
            Constraint::Length(1), // Volume name line (left | right)
            Constraint::Min(10),   // File panes (NO BORDERS)
            Constraint::Length(1), // Pane info line (left | right)
            Constraint::Length(1), // Selected filename line
            Constraint::Length(if state.ui.layout.show_task_panel { 5 } else { 0 }), // Task view (NO BORDER)
        ])
        .split(size);

    let mut chunk_idx = 0;

    // Render tab bar
    if state.ui.layout.show_tab_bar {
        render_tab_bar(frame, chunks[chunk_idx], state);
        chunk_idx += 1;
    }

    // Render path line
    render_path_line(frame, chunks[chunk_idx], state);
    chunk_idx += 1;

    // Render volume name line
    render_volume_line(frame, chunks[chunk_idx], state);
    chunk_idx += 1;

    // Render panes (NO BORDERS)
    render_panes(frame, chunks[chunk_idx], state);
    chunk_idx += 1;

    // Render pane info line
    render_pane_info_line(frame, chunks[chunk_idx], state);
    chunk_idx += 1;

    // Render filename line
    render_filename_line(frame, chunks[chunk_idx], state);
    chunk_idx += 1;

    // Render task panel (NO BORDER)
    if state.ui.layout.show_task_panel {
        render_task_panel(frame, chunks[chunk_idx], state);
    }
}
