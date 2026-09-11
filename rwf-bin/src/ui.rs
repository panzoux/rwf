//! UI rendering module
//!
//! This module handles all UI rendering using ratatui.

mod colors;
mod diag_badge;
pub mod dialog;
mod filename_line;
mod leap_bar;
pub mod multiline_text_input;
mod pane_info_line;
mod panes;
mod path_line;
pub mod screen_text;
pub mod smart_text;
mod spinner;
mod tab_bar;
pub mod task_panel;
pub mod text_input;
mod unicode_utils;
mod viewer;
mod volume_line;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};
use rwf_lib::model::{UIMode, ViewerLayout};
use rwf_lib::AppState;

pub use colors::parse_color;
pub use dialog::render_dialog;
pub use filename_line::render_filename_line;
pub use pane_info_line::render_pane_info_line;
pub use panes::{render_active_pane_only, render_panes};
pub use path_line::render_path_line;
pub use smart_text::{SmartText, TruncateMode};
pub use tab_bar::render_tab_bar;
pub use task_panel::{render_task_panel, TaskPanel};
pub use unicode_utils::{
    pad_to_width, sanitize_for_display, shorten_path, smart_truncate, truncate_to_width,
};
pub use volume_line::render_volume_line;

/// Main UI rendering function
/// Minimum visible file-listing rows (path+volume+pane_info+filename take the other 4).
const FILE_PANE_MIN: u16 = 3;
/// Fixed 1-line sections surrounding the file pane: path, volume, pane_info, filename.
const CONTENT_FIXED_LINES: u16 = 4;

pub fn render_ui(frame: &mut Frame, state: &AppState, task_panel: &TaskPanel) {
    render_ui_inner(frame, state, task_panel);

    // Drawn last, over whatever layout was chosen. `render_ui_inner` returns
    // early for the full-screen viewer, so anything that must appear in every
    // mode has to live out here rather than inside it.
    diag_badge::render_diag_badge(frame, frame.area());
}

fn render_ui_inner(frame: &mut Frame, state: &AppState, task_panel: &TaskPanel) {
    let size = frame.area();

    let tab_bar_h = if state.ui.layout.show_tab_bar {
        1u16
    } else {
        0
    };

    // Compute how much the task panel can actually use without hiding pane_info / filename.
    let desired_task_h = if state.ui.layout.show_task_panel {
        state.ui.layout.task_panel_height as u16
    } else {
        0
    };
    let available_below_fixed = size
        .height
        .saturating_sub(tab_bar_h)
        .saturating_sub(CONTENT_FIXED_LINES)
        .saturating_sub(FILE_PANE_MIN);
    let task_panel_height = desired_task_h.min(available_below_fixed);

    let is_viewer_active = state.ui.mode == UIMode::Viewer
        || state.ui.mode == UIMode::ViewerSearch
        || state.ui.mode == UIMode::ViewerCommand;

    // Full-screen viewer: replaces everything except optionally the task panel.
    if is_viewer_active && state.ui.layout.viewer_layout == ViewerLayout::FullScreen {
        if let Some(viewer) = &state.viewer {
            viewer::render_viewer(
                frame,
                size,
                viewer,
                &state.config.display.colors,
                state.ui.mode,
                &state.viewer_search_input,
                &state.viewer_command_input,
                false, // full-screen never needs focus brackets
            );
        }
        return;
    }

    // Outer vertical layout: [tab bar] / [content] / [task panel]
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(tab_bar_h),
            Constraint::Min(FILE_PANE_MIN + CONTENT_FIXED_LINES),
            Constraint::Length(task_panel_height),
        ])
        .split(size);

    if state.ui.layout.show_tab_bar {
        render_tab_bar(frame, outer[0], state);
    }

    let content_area = outer[1];
    let task_area = outer[2];

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
                Constraint::Length(1),          // Path line
                Constraint::Length(1),          // Volume line
                Constraint::Min(FILE_PANE_MIN), // File pane (single pane, full width)
                Constraint::Length(1),          // Pane info line
                Constraint::Length(1),          // Filename line
            ])
            .split(pane_area);

        render_path_line(frame, pane_chunks[0], state, Some(anchor));
        render_volume_line(frame, pane_chunks[1], state, Some(anchor));
        render_active_pane_only(frame, pane_chunks[2], state, anchor);
        render_pane_info_line(frame, pane_chunks[3], state, Some(anchor));
        render_filename_line(frame, pane_chunks[4], state);

        // Render viewer side — directory preview or file viewer.
        let tab = state.current_tab();
        let anchor_entry = match anchor {
            rwf_lib::model::ActivePane::Left => tab.left_pane.current_entry(),
            rwf_lib::model::ActivePane::Right => tab.right_pane.current_entry(),
        };

        if let Some(entry) = anchor_entry {
            if entry.is_dir {
                // Looked up, never counted here. This used to call std::fs::read_dir
                // from inside the draw path, on the entry under the cursor — which can
                // live on a network share, and which the old comment wrongly assumed
                // was both local and rarely redrawn. `JobKind::CountDirectoryEntries`
                // fills this map from a worker; `None` simply renders no counts until
                // it arrives.
                let counts = state.dir_preview_counts.get(&entry.location).copied();
                viewer::render_dir_preview(
                    frame,
                    viewer_area,
                    &entry.location,
                    counts,
                    &state.config.display.colors,
                    is_viewer_active,
                );
            } else if let Some(v) = &state.viewer {
                viewer::render_viewer(
                    frame,
                    viewer_area,
                    v,
                    &state.config.display.colors,
                    state.ui.mode,
                    &state.viewer_search_input,
                    &state.viewer_command_input,
                    is_viewer_active,
                );
            }
        } else if let Some(v) = &state.viewer {
            viewer::render_viewer(
                frame,
                viewer_area,
                v,
                &state.config.display.colors,
                state.ui.mode,
                &state.viewer_search_input,
                &state.viewer_command_input,
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
            Constraint::Length(1),          // Path line (left | right)
            Constraint::Length(1),          // Volume name line (left | right)
            Constraint::Min(FILE_PANE_MIN), // File panes (NO BORDERS)
            Constraint::Length(1),          // Pane info line (left | right)
            Constraint::Length(1),          // Selected filename line
        ])
        .split(content_area);

    render_path_line(frame, chunks[0], state, None);
    render_volume_line(frame, chunks[1], state, None);
    render_panes(frame, chunks[2], state);
    render_pane_info_line(frame, chunks[3], state, None);
    render_filename_line(frame, chunks[4], state);

    if state.ui.layout.show_task_panel {
        render_task_panel(frame, task_area, task_panel, &state.config.display.colors);
    }

    // Dialog overlay
    if let Some(dialog) = state.dialogs.current() {
        render_dialog(frame, dialog, state);
    }
}

#[cfg(test)]
mod side_by_side_layout_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rwf_lib::model::{ActivePane, FileEntry, Location, ViewerLayout, ViewerMode, ViewerState};
    use rwf_lib::{AppConfig, AppState};
    use std::path::PathBuf;

    fn entry(name: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(format!("/test/{name}"))),
            size: 0,
            is_dir,
            is_hidden: false,
            modified: std::time::SystemTime::UNIX_EPOCH,
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        }
    }

    /// SideBySide with the viewer on the side opposite `anchor`. The left pane summarises as
    /// "2 Dirs 0 Files" and the right as "0 Dirs 1 File", so each is identifiable by a
    /// substring the other cannot produce.
    fn sbs_state(anchor: ActivePane) -> AppState {
        let mut state = AppState::new(AppConfig::default());
        {
            let tab = state.current_tab_mut();
            tab.left_pane.entries = vec![entry("d1", true), entry("d2", true)];
            tab.right_pane.entries = vec![entry("only.txt", false)];
        }
        state.ui.active_pane = anchor;
        state.ui.layout.viewer_layout = ViewerLayout::SideBySide;
        state.ui.layout.viewer_anchor_pane = anchor;
        let mut viewer = ViewerState::new(Location::Local(PathBuf::from("/test/only.txt")));
        viewer.mode = ViewerMode::Text;
        state.viewer = Some(viewer);
        state
    }

    fn draw(state: &AppState) -> String {
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let task_panel = task_panel::TaskPanel::new();
        terminal
            .draw(|frame| render_ui(frame, state, &task_panel))
            .expect("draw");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// The bug from diagnostic bundle 20260908-212740: with the viewer on the left, the
    /// pane-info line still painted the hidden left pane's counts next to the right pane's.
    #[test]
    fn viewer_on_left_shows_only_the_right_pane_counts() {
        let out = draw(&sbs_state(ActivePane::Right));
        assert!(out.contains("1 File"), "visible pane's counts missing");
        assert!(
            !out.contains("2 Dirs"),
            "hidden left pane's counts rendered beside the viewer"
        );
    }

    /// The mirror case the reporter suspected but had not tested.
    #[test]
    fn viewer_on_right_shows_only_the_left_pane_counts() {
        let out = draw(&sbs_state(ActivePane::Left));
        assert!(out.contains("2 Dirs"), "visible pane's counts missing");
        assert!(
            !out.contains("1 File"),
            "hidden right pane's counts rendered beside the viewer"
        );
    }

    /// Diagnostic bundle `20260909-172206` at the level it was reported: what is on
    /// screen after a tab round trip. Two tabs hold SideBySide viewers on opposite
    /// sides; returning to the first must redraw it on the side it was left on, not on
    /// the side the *other* tab happened to be using.
    ///
    /// The tabs are told apart by their pane summaries: tab 0's visible (right) pane
    /// reads "1 File", tab 1's visible (left) pane reads "2 Dirs".
    #[test]
    fn a_tab_round_trip_redraws_each_viewer_on_its_own_side() {
        use rwf_lib::state::{update_state, Transition};

        // Tab 0 anchors right -> viewer on the left.
        let mut state = sbs_state(ActivePane::Right);
        state.tabs.create_tab();
        {
            let tab = &mut state.tabs.tabs[1];
            tab.left_pane.entries = vec![entry("d1", true), entry("d2", true)];
            tab.right_pane.entries = vec![entry("only.txt", false)];
        }

        // Tab 1 anchors left -> viewer on the right.
        update_state(&mut state, Transition::NextTab);
        state.ui.active_pane = ActivePane::Left;
        update_state(
            &mut state,
            Transition::OpenSideBySideViewer {
                location: Location::Local(PathBuf::from("/test/d1")),
                mode: ViewerMode::Text,
            },
        );
        let on_tab1 = draw(&state);
        assert!(
            on_tab1.contains("2 Dirs") && !on_tab1.contains("1 File"),
            "tab 1 should render its left pane"
        );

        // Back to tab 0: its own right pane again, not tab 1's left.
        update_state(&mut state, Transition::PrevTab);
        let back_on_tab0 = draw(&state);
        assert!(
            back_on_tab0.contains("1 File"),
            "tab 0's viewer came back on the wrong side: its right pane is not drawn"
        );
        assert!(
            !back_on_tab0.contains("2 Dirs"),
            "tab 0 redrew the left pane -- the anchor followed tab 1"
        );
    }
}
