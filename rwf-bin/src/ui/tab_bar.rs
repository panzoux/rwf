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
use unicode_width::UnicodeWidthStr;

/// Width of the ` < ` / ` > ` overflow indicators.
const INDICATOR_WIDTH: usize = 3;

/// Pick the run of tabs to show, given each tab's rendered width.
///
/// A label carries both pane paths, so it is 30-40 columns wide and only two or
/// three fit an ordinary terminal. Guessing a fixed per-tab width instead of
/// measuring is what silently clipped the tail of the bar: the guess said
/// everything fit, so no `>` was drawn and the hidden tabs looked like they had
/// vanished rather than scrolled off.
///
/// Grows outward from the active tab — right first, so the tabs Ctrl+Tab
/// reaches next stay visible — and charges the window for whichever indicators
/// the resulting window will actually need.
fn visible_window(widths: &[usize], active: usize, avail: usize) -> (usize, usize) {
    let total = widths.len();
    if total == 0 || active >= total {
        return (0, 0);
    }
    if widths.iter().sum::<usize>() <= avail {
        return (0, total);
    }

    let indicators = |start: usize, end: usize| {
        usize::from(start > 0) * INDICATOR_WIDTH + usize::from(end < total) * INDICATOR_WIDTH
    };

    // The active tab is always shown, even when it alone overflows: one clipped
    // tab beats an empty bar.
    let (mut start, mut end) = (active, active + 1);
    let mut used = widths[active];

    loop {
        let mut grew = false;
        if end < total && used + widths[end] + indicators(start, end + 1) <= avail {
            used += widths[end];
            end += 1;
            grew = true;
        }
        if start > 0 && used + widths[start - 1] + indicators(start - 1, end) <= avail {
            used += widths[start - 1];
            start -= 1;
            grew = true;
        }
        if !grew {
            return (start, end);
        }
    }
}

/// Build one tab's label, exactly as it is drawn.
fn tab_label(state: &AppState, idx: usize, spinner: &str) -> String {
    let tab = &state.tabs.tabs[idx];
    let ellipsis = &state.config.ellipsis;
    let is_active = idx == state.tabs.active_index;

    // Background jobs are keyed by tab id, not position: a position names a
    // different tab once one to its left closes.
    let job_count = state.background_jobs.get_active_job_count(tab.id);
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
    if has_jobs {
        let spinner_display = spinner.repeat(job_count.min(3)); // Max 3 spinners
        format!(" [{}{}:{}] ", spinner_display, idx + 1, active_marker)
    } else if is_active {
        format!(" [{}:{}] ", idx + 1, active_marker)
    } else {
        format!(" {}:{} ", idx + 1, active_marker)
    }
}

/// Render the tab bar with spinner animation for busy tabs
pub fn render_tab_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let spinner = spinner::current_frame(
        &state.config.display.spinner_frames,
        state.config.display.spinner_frame_ms,
    );
    let colors = &state.config.display.colors;

    let total_tabs = state.tabs.tabs.len();
    let active_idx = state.tabs.active_index;

    // Build every label first: the window can only be chosen from real widths.
    let labels: Vec<String> = (0..total_tabs)
        .map(|idx| tab_label(state, idx, spinner))
        .collect();
    let widths: Vec<usize> = labels.iter().map(|l| l.width()).collect();

    let (start_idx, end_idx) = visible_window(&widths, active_idx, area.width as usize);

    let mut spans = Vec::new();

    // Add left scroll indicator if needed
    if start_idx > 0 {
        spans.push(Span::styled(
            " < ",
            Style::default().fg(parse_color(&colors.warning_color)),
        ));
    }

    // Render visible tabs
    for (offset, label) in labels[start_idx..end_idx].iter().enumerate() {
        // Apply style
        let style = if start_idx + offset == active_idx {
            Style::default()
                .fg(parse_color(&colors.active_tab_foreground_color))
                .bg(parse_color(&colors.active_tab_background_color))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(parse_color(&colors.inactive_tab_foreground_color))
                .bg(parse_color(&colors.inactive_tab_background_color))
        };

        spans.push(Span::styled(label.clone(), style));
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

    /// A tab bar too narrow for every tab must say so with `>` instead of
    /// letting ratatui clip the tail off in silence. Regression for the
    /// diagnostic bundle `20260920-112645`: 7 restored tabs, 96 columns, and a
    /// header that showed 3 with no hint the other 4 existed.
    #[test]
    fn test_render_tab_bar_marks_hidden_tabs() {
        let mut state = smoke_state();
        for _ in 0..4 {
            state.tabs.create_tab();
        }
        for (idx, tab) in state.tabs.tabs.iter_mut().enumerate() {
            tab.left_pane.current_location =
                Location::Local(std::path::PathBuf::from(format!("/test/tab{idx}/left")));
            tab.right_pane.current_location =
                Location::Local(std::path::PathBuf::from(format!("/test/tab{idx}/right")));
        }
        state.tabs.active_index = 0;

        let backend = TestBackend::new(96, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_tab_bar(frame, area, &state);
            })
            .expect("draw");

        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(
            rendered.contains(" > "),
            "hidden tabs must be announced: {rendered}"
        );
        // The last tab that fits is whole, not sliced mid-path.
        assert!(
            rendered.contains("2:/test/tab1/left|/test/ta…/right "),
            "visible tabs must be rendered whole: {rendered}"
        );
    }

    /// Scrolling right must keep the active tab on screen, with `<` marking the
    /// tabs left behind.
    #[test]
    fn test_render_tab_bar_keeps_active_tab_visible() {
        let mut state = smoke_state();
        for _ in 0..4 {
            state.tabs.create_tab();
        }
        for (idx, tab) in state.tabs.tabs.iter_mut().enumerate() {
            tab.left_pane.current_location =
                Location::Local(std::path::PathBuf::from(format!("/test/tab{idx}/left")));
            tab.right_pane.current_location =
                Location::Local(std::path::PathBuf::from(format!("/test/tab{idx}/right")));
        }
        state.tabs.active_index = 6;

        let backend = TestBackend::new(96, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_tab_bar(frame, area, &state);
            })
            .expect("draw");

        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(
            rendered.contains("[7:"),
            "the active tab must be on screen: {rendered}"
        );
        assert!(
            rendered.contains(" < "),
            "tabs scrolled off the left must be announced: {rendered}"
        );
    }

    #[test]
    fn visible_window_shows_everything_when_it_fits() {
        assert_eq!(visible_window(&[10, 10, 10], 0, 40), (0, 3));
        assert_eq!(visible_window(&[10, 10, 10], 2, 30), (0, 3));
    }

    #[test]
    fn visible_window_reserves_room_for_the_indicators() {
        // 30 columns, three 10-wide tabs, active first: a ` > ` has to fit too,
        // so only two tabs make it.
        assert_eq!(visible_window(&[10, 10, 10], 0, 29), (0, 2));
        // Active in the middle: taking the tab to its right still leaves tab 1
        // hidden, so 10 + 10 + ` < ` must fit before tab 3 joins.
        assert_eq!(visible_window(&[10, 10, 10], 1, 22), (1, 2));
        assert_eq!(visible_window(&[10, 10, 10], 1, 23), (1, 3));
    }

    #[test]
    fn visible_window_always_keeps_the_active_tab() {
        // Narrower than a single tab: show the active one and let it clip,
        // rather than rendering an empty bar.
        assert_eq!(visible_window(&[40, 40, 40], 2, 5), (2, 3));
    }

    #[test]
    fn visible_window_handles_degenerate_input() {
        assert_eq!(visible_window(&[], 0, 80), (0, 0));
        assert_eq!(visible_window(&[10, 10], 9, 80), (0, 0));
    }
}
