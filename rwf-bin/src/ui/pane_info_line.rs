//! Pane info line rendering
//!
//! Displays file/directory counts and sizes for both panes

use super::{leap_bar, parse_color};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
    Frame,
};
use rwf_lib::{model::ActivePane, model::PaneModel, model::UIMode, AppState};

/// Render the pane info line.
/// `single_pane`: when `Some(pane)`, render only that pane's stats at full width
/// (used in SideBySide mode where the other half is the viewer).
pub fn render_pane_info_line(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    single_pane: Option<ActivePane>,
) {
    let tab = state.current_tab();
    let colors = &state.config.display.colors;

    let render_one = |frame: &mut Frame, rect: Rect, pane: ActivePane| {
        let pane_model = match pane {
            ActivePane::Left => &tab.left_pane,
            ActivePane::Right => &tab.right_pane,
        };
        let is_active = state.ui.active_pane == pane;

        if state.ui.mode == UIMode::Leap && is_active {
            if let Some(ref leap) = state.leap {
                leap_bar::render_leap_bar(
                    frame,
                    rect,
                    leap,
                    &pane_model.entries,
                    &state.config.jump_nav.no_match_feedback,
                    pane_model.is_loading,
                    &state.config.display.spinner_frames,
                    state.config.display.spinner_frame_ms,
                );
            }
        } else if state.ui.mode == UIMode::Search && is_active {
            render_search_bar(frame, rect, &state.search.query, colors);
        } else {
            let info = calculate_pane_info(pane_model);
            let para = Paragraph::new(Span::raw(info)).style(
                Style::default()
                    .fg(parse_color(colors.get_pane_info_foreground()))
                    .bg(parse_color(colors.get_pane_info_background())),
            );
            frame.render_widget(para, rect);
        }
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

/// Render the search bar (replacement for stats)
fn render_search_bar(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    colors: &rwf_lib::config::ColorScheme,
) {
    use ratatui::style::Color;
    use ratatui::text::Line;

    let search_style = Style::default()
        .fg(parse_color(colors.get_pane_info_foreground()))
        .bg(parse_color(colors.get_pane_info_background()));

    let query_style = Style::default()
        .fg(Color::Yellow) // Distinct color for search query
        .bg(parse_color(colors.get_pane_info_background()));

    let content = Line::from(vec![
        Span::styled("/", search_style),
        Span::styled(query, query_style),
        Span::styled(" ", search_style), // Cursor placeholder
    ]);

    let para = Paragraph::new(content).style(search_style);
    frame.render_widget(para, area);
}

/// Calculate pane information string
fn calculate_pane_info(pane: &PaneModel) -> String {
    let dir_count = pane.entries.iter().filter(|e| e.is_dir).count();
    let file_count = pane.entries.len() - dir_count;

    let dir_text = if dir_count == 1 { "Dir" } else { "Dirs" };
    let file_text = if file_count == 1 { "File" } else { "Files" };

    // Calculate total size
    let total_size: u64 = pane
        .entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.size)
        .sum();

    format!(
        " {} {} {} {}  {}",
        dir_count,
        dir_text,
        file_count,
        file_text,
        format_size(total_size)
    )
}

/// Format size in human-readable format
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes < TB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rwf_lib::model::{FileEntry, Location};
    use rwf_lib::{AppConfig, AppState};
    use std::path::PathBuf;

    // rwf-lib's shared fixtures are `#[cfg(test)]`-gated and so invisible from rwf-bin;
    // this mirrors the local helpers in `panes.rs`.
    fn entry(name: &str, is_dir: bool, size: u64) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(format!("/test/{name}"))),
            size,
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

    /// Left pane reads "2 Dirs 0 Files", right pane "0 Dirs 1 File" — each summary carries a
    /// substring the other cannot produce, so presence/absence is unambiguous.
    fn state_with_distinct_panes() -> AppState {
        let mut state = AppState::new(AppConfig::default());
        let tab = state.current_tab_mut();
        tab.left_pane.entries = vec![entry("a", true, 0), entry("b", true, 0)];
        tab.right_pane.entries = vec![entry("c.txt", false, 1234)];
        state
    }

    fn render(state: &AppState, single_pane: Option<ActivePane>) -> String {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_pane_info_line(frame, area, state, single_pane);
            })
            .expect("draw");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// The reported bug: with the viewer on the left, the info line is handed only the
    /// right-hand column, but used to sub-split it and paint the hidden left pane's counts
    /// alongside the visible ones.
    #[test]
    fn single_pane_right_shows_only_the_right_summary() {
        let out = render(&state_with_distinct_panes(), Some(ActivePane::Right));
        assert!(
            out.contains("1 File"),
            "right pane summary missing: {out:?}"
        );
        assert!(
            !out.contains("2 Dirs"),
            "hidden left pane's summary leaked in: {out:?}"
        );
    }

    /// The mirror image: viewer on the right, file pane on the left.
    #[test]
    fn single_pane_left_shows_only_the_left_summary() {
        let out = render(&state_with_distinct_panes(), Some(ActivePane::Left));
        assert!(out.contains("2 Dirs"), "left pane summary missing: {out:?}");
        assert!(
            !out.contains("1 File"),
            "hidden right pane's summary leaked in: {out:?}"
        );
    }

    /// Normal two-pane mode is unchanged: both summaries, side by side.
    #[test]
    fn none_shows_both_summaries() {
        let out = render(&state_with_distinct_panes(), None);
        assert!(out.contains("2 Dirs"), "left pane summary missing: {out:?}");
        assert!(
            out.contains("1 File"),
            "right pane summary missing: {out:?}"
        );
    }
}
