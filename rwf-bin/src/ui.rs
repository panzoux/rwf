//! UI rendering module
//!
//! This module handles all UI rendering using ratatui.

mod panes;
mod tab_bar;
pub mod task_panel;
mod filename_line;
mod path_line;
mod volume_line;
mod pane_info_line;
mod leap_bar;
mod colors;
mod unicode_utils;
pub mod dialog;
pub mod text_input;
pub mod smart_text;
mod viewer;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};
use rwf_lib::AppState;
use rwf_lib::model::{UIMode, ViewerLayout};

pub use panes::{render_panes, render_active_pane_only};
pub use tab_bar::render_tab_bar;
pub use task_panel::{render_task_panel, TaskPanel};
pub use filename_line::render_filename_line;
pub use path_line::render_path_line;
pub use volume_line::render_volume_line;
pub use pane_info_line::render_pane_info_line;
pub use colors::parse_color;
pub use unicode_utils::{pad_to_width, shorten_path, smart_truncate};
pub use dialog::render_dialog;
pub use smart_text::{SmartText, TruncateMode};

/// Main UI rendering function
pub fn render_ui(frame: &mut Frame, state: &AppState, task_panel: &TaskPanel) {
    let size = frame.area();

    let task_panel_height = if state.ui.layout.show_task_panel {
        state.ui.layout.task_panel_height as u16
    } else {
        0
    };

    let is_viewer_active = state.ui.mode == UIMode::Viewer
        || state.ui.mode == UIMode::ViewerSearch
        || state.ui.mode == UIMode::ViewerCommand;

    // Full-screen viewer: replaces everything except optionally the task panel.
    if is_viewer_active && state.ui.layout.viewer_layout == ViewerLayout::FullScreen {
        if let Some(viewer) = &state.viewer {
            viewer::render_viewer(
                frame, size, viewer, &state.config.display.colors, state.ui.mode,
                &state.viewer_search_input, &state.viewer_command_input,
                false, // full-screen never needs focus brackets
            );
        }
        return;
    }

    // Outer vertical layout: [tab bar] / [content] / [task panel]
    let tab_bar_h = if state.ui.layout.show_tab_bar { 1 } else { 0 };
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(tab_bar_h),
            Constraint::Min(10),
            Constraint::Length(task_panel_height),
        ])
        .split(size);

    if state.ui.layout.show_tab_bar {
        render_tab_bar(frame, outer[0], state, task_panel.current_spinner());
    }

    let content_area = outer[1];
    let task_area    = outer[2];

    // Side-by-side viewer: split content area left/right.
    if state.viewer.is_some() && state.ui.layout.viewer_layout == ViewerLayout::SideBySide {
        // Use the pane that was active when the viewer opened so the viewer never
        // jumps sides regardless of how ui.active_pane changes later.
        let anchor = state.ui.layout.viewer_anchor_pane;
        let anchor_on_left = anchor == rwf_lib::model::ActivePane::Left;

        // Viewer goes on the opposite side from the anchored file pane.
        let halves = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_area);
        let (pane_area, viewer_area) = if anchor_on_left {
            (halves[0], halves[1]) // file pane left, viewer right
        } else {
            (halves[1], halves[0]) // viewer left, file pane right
        };

        // Render file-pane side: only the anchored pane fills the full 50%.
        let pane_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Path line
                Constraint::Length(1), // Volume line
                Constraint::Min(5),    // File pane (single pane, full width)
                Constraint::Length(1), // Pane info line
                Constraint::Length(1), // Filename line
            ])
            .split(pane_area);

        render_path_line(frame, pane_chunks[0], state, Some(anchor));
        render_volume_line(frame, pane_chunks[1], state, Some(anchor));
        render_active_pane_only(frame, pane_chunks[2], state, anchor);
        render_pane_info_line(frame, pane_chunks[3], state);
        render_filename_line(frame, pane_chunks[4], state);

        // Render viewer side — directory preview or file viewer.
        let tab = state.current_tab();
        let anchor_entry = match anchor {
            rwf_lib::model::ActivePane::Left  => tab.left_pane.current_entry(),
            rwf_lib::model::ActivePane::Right => tab.right_pane.current_entry(),
        };

        if let Some(entry) = anchor_entry {
            if entry.is_dir {
                // Count directory contents inline — std::fs::read_dir on local FS
                // is sub-millisecond and this render only fires on state changes.
                let counts = entry.location.path().and_then(|p| std::fs::read_dir(p).ok()).map(|rd| {
                    let mut files = 0usize;
                    let mut folders = 0usize;
                    for e in rd.flatten() {
                        match e.file_type() {
                            Ok(ft) if ft.is_dir() => folders += 1,
                            Ok(_)                 => files   += 1,
                            Err(_)                => {}
                        }
                    }
                    (files, folders)
                });
                viewer::render_dir_preview(
                    frame, viewer_area, &entry.location, counts,
                    &state.config.display.colors, is_viewer_active,
                );
            } else if let Some(v) = &state.viewer {
                viewer::render_viewer(
                    frame, viewer_area, v, &state.config.display.colors, state.ui.mode,
                    &state.viewer_search_input, &state.viewer_command_input,
                    is_viewer_active,
                );
            }
        } else if let Some(v) = &state.viewer {
            viewer::render_viewer(
                frame, viewer_area, v, &state.config.display.colors, state.ui.mode,
                &state.viewer_search_input, &state.viewer_command_input,
                is_viewer_active,
            );
        }

        if state.ui.layout.show_task_panel {
            render_task_panel(frame, task_area, task_panel, &state.config.display.colors);
        }

        if let Some(dialog) = state.dialogs.current() {
            render_dialog(frame, dialog, state);
        }
        return;
    }

    // Normal mode layout: Path → Volume → Panes → PaneInfo → Filename
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Path line (left | right)
            Constraint::Length(1), // Volume name line (left | right)
            Constraint::Min(10),   // File panes (NO BORDERS)
            Constraint::Length(1), // Pane info line (left | right)
            Constraint::Length(1), // Selected filename line
        ])
        .split(content_area);

    render_path_line(frame, chunks[0], state, None);
    render_volume_line(frame, chunks[1], state, None);
    render_panes(frame, chunks[2], state);
    render_pane_info_line(frame, chunks[3], state);
    render_filename_line(frame, chunks[4], state);

    if state.ui.layout.show_task_panel {
        render_task_panel(frame, task_area, task_panel, &state.config.display.colors);
    }

    // Dialog overlay
    if let Some(dialog) = state.dialogs.current() {
        render_dialog(frame, dialog, state);
    }
}
