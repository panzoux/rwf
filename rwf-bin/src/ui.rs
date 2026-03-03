//! UI rendering module
//!
//! This module handles all UI rendering using ratatui.

mod panes;
mod status_bar;
mod tab_bar;
mod task_panel;
mod top_separator;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};
use rwf_lib::AppState;

pub use panes::render_panes;
pub use status_bar::render_status_bar;
pub use tab_bar::render_tab_bar;
pub use task_panel::render_task_panel;
pub use top_separator::render_top_separator;

/// Main UI rendering function
pub fn render_ui(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Create main layout: tab bar, top separator, panes, task panel, status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if state.ui.layout.show_tab_bar { 1 } else { 0 }),
            Constraint::Length(1), // Top separator (always shown)
            Constraint::Min(10), // Panes area
            Constraint::Length(if state.ui.layout.show_task_panel { 5 } else { 0 }),
            Constraint::Length(if state.ui.layout.show_status_bar { 1 } else { 0 }),
        ])
        .split(size);

    let mut chunk_idx = 0;

    // Render tab bar
    if state.ui.layout.show_tab_bar {
        render_tab_bar(frame, chunks[chunk_idx], state);
        chunk_idx += 1;
    }

    // Render top separator
    render_top_separator(frame, chunks[chunk_idx], state);
    chunk_idx += 1;

    // Render panes
    render_panes(frame, chunks[chunk_idx], state);
    chunk_idx += 1;

    // Render task panel
    if state.ui.layout.show_task_panel {
        render_task_panel(frame, chunks[chunk_idx], state);
        chunk_idx += 1;
    }

    // Render status bar
    if state.ui.layout.show_status_bar {
        render_status_bar(frame, chunks[chunk_idx], state);
    }
}
