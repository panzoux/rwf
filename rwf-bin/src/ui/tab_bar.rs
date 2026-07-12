//! Tab bar rendering
//!
//! This module renders the tab bar at the top of the screen.

use super::{parse_color, shorten_path, spinner};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::AppState;

/// Render the tab bar with spinner animation for busy tabs
pub fn render_tab_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let spinner = spinner::current_frame(
        &state.config.display.spinner_frames,
        state.config.display.spinner_frame_ms,
    );
    let colors = &state.config.display.colors;
    let ellipsis = &state.config.ellipsis;
    let mut spans = Vec::new();

    // Calculate how many tabs can fit in the available width
    // Each tab takes approximately 6-8 characters: " [~1~] " or "  1  "
    let avg_tab_width = 8;
    let max_visible_tabs = (area.width as usize / avg_tab_width).max(1);

    let total_tabs = state.tabs.tabs.len();
    let active_idx = state.tabs.active_index;

    // Calculate scroll window to keep active tab visible
    let (start_idx, end_idx) = if total_tabs <= max_visible_tabs {
        // All tabs fit, show them all
        (0, total_tabs)
    } else {
        // Need scrolling - center active tab in the window
        let half_window = max_visible_tabs / 2;
        let start = active_idx.saturating_sub(half_window);
        let end = (start + max_visible_tabs).min(total_tabs);

        // Adjust start if we're at the end
        let start = if end == total_tabs {
            total_tabs.saturating_sub(max_visible_tabs)
        } else {
            start
        };

        (start, end)
    };

    // Add left scroll indicator if needed
    if start_idx > 0 {
        spans.push(Span::styled(
            " < ",
            Style::default().fg(parse_color(&colors.warning_color)),
        ));
    }

    // Render visible tabs
    for idx in start_idx..end_idx {
        let tab = &state.tabs.tabs[idx];
        let is_active = idx == state.tabs.active_index;

        // Check if tab has active jobs using BackgroundJobManager
        let job_count = state.background_jobs.get_active_job_count(idx);
        let has_jobs = job_count > 0;

        // Get shortened paths for left and right panes
        let left_path = shorten_path(&tab.left_pane.current_location.display_path(), 15, ellipsis);
        let right_path = shorten_path(
            &tab.right_pane.current_location.display_path(),
            15,
            ellipsis,
        );

        // Determine which pane is active (only for the active tab)
        let active_marker = if is_active {
            match state.ui.active_pane {
                rwf_lib::model::ActivePane::Left => format!("{}*|{}", left_path, right_path),
                rwf_lib::model::ActivePane::Right => format!("{}|{}*", left_path, right_path),
            }
        } else {
            format!("{}|{}", left_path, right_path)
        };

        // Format tab label with spinner for busy tabs (TWF style)
        // Multiple spinners for multiple jobs: /, //, ///, etc.
        let spinner_display = if has_jobs {
            spinner.repeat(job_count.min(3)) // Max 3 spinners
        } else {
            String::new()
        };

        let label = if has_jobs {
            format!(" [{}{}:{}] ", spinner_display, idx + 1, active_marker)
        } else if is_active {
            format!(" [{}:{}] ", idx + 1, active_marker)
        } else {
            format!(" {}:{} ", idx + 1, active_marker)
        };

        // Apply style
        let style = if is_active {
            Style::default()
                .fg(parse_color(&colors.active_tab_foreground_color))
                .bg(parse_color(&colors.active_tab_background_color))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(parse_color(&colors.inactive_tab_foreground_color))
                .bg(parse_color(&colors.inactive_tab_background_color))
        };

        spans.push(Span::styled(label, style));
    }

    // Add right scroll indicator if needed
    if end_idx < total_tabs {
        spans.push(Span::styled(
            " > ",
            Style::default().fg(parse_color(&colors.warning_color)),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(parse_color(&colors.tabbar_background_color)));

    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rwf_lib::model::Location;
    use rwf_lib::{AppConfig, AppState};

    /// A 3-tab state with fixed, deterministic pane locations (not the real
    /// CWD `TabState::new` defaults to, which vary by machine/CI).
    fn smoke_state() -> AppState {
        let mut state = AppState::new(AppConfig::default());
        state.tabs.create_tab();
        state.tabs.create_tab();
        for (idx, tab) in state.tabs.tabs.iter_mut().enumerate() {
            tab.left_pane.current_location =
                Location::Local(std::path::PathBuf::from(format!("/test/tab{idx}/left")));
            tab.right_pane.current_location =
                Location::Local(std::path::PathBuf::from(format!("/test/tab{idx}/right")));
        }
        state
    }

    /// M7 S2-2: render_tab_bar must not panic with multiple tabs, including
    /// the active/inactive style split.
    #[test]
    fn test_render_tab_bar_does_not_panic() {
        let state = smoke_state();

        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_tab_bar(frame, area, &state);
            })
            .expect("draw");
    }

    /// M7 S2-2: representative snapshot of a 3-tab bar.
    #[test]
    fn test_render_tab_bar_snapshot() {
        let state = smoke_state();

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_tab_bar(frame, area, &state);
            })
            .expect("draw");
        let output = format!("{:?}", terminal.backend().buffer());
        insta::assert_snapshot!("render_tab_bar_smoke", output);
    }
}
