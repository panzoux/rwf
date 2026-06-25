//! Pane info line rendering
//!
//! Displays file/directory counts and sizes for both panes

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
    Frame,
};
use rwf_lib::{AppState, model::PaneModel, model::UIMode, model::ActivePane};
use super::{leap_bar, parse_color};

/// Render the pane info line showing stats for both panes
pub fn render_pane_info_line(frame: &mut Frame, area: Rect, state: &AppState) {
    let tab = state.current_tab();
    let colors = &state.config.display.colors;
    
    // Split into two halves
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    
    // Left pane info
    if state.ui.mode == UIMode::Leap && state.ui.active_pane == ActivePane::Left {
        if let Some(ref leap) = state.leap {
            let visible = &tab.left_pane.entries;
            leap_bar::render_leap_bar(frame, halves[0], leap, visible,
                &state.config.jump_nav.no_match_feedback, tab.left_pane.is_loading);
        }
    } else if state.ui.mode == UIMode::Search && state.ui.active_pane == ActivePane::Left {
        render_search_bar(frame, halves[0], &state.search.query, colors);
    } else {
        let left_info = calculate_pane_info(&tab.left_pane);
        let left_para = Paragraph::new(Span::raw(left_info))
            .style(Style::default()
                .fg(parse_color(colors.get_pane_info_foreground()))
                .bg(parse_color(colors.get_pane_info_background())));
        frame.render_widget(left_para, halves[0]);
    }

    // Right pane info
    if state.ui.mode == UIMode::Leap && state.ui.active_pane == ActivePane::Right {
        if let Some(ref leap) = state.leap {
            let visible = &tab.right_pane.entries;
            leap_bar::render_leap_bar(frame, halves[1], leap, visible,
                &state.config.jump_nav.no_match_feedback, tab.right_pane.is_loading);
        }
    } else if state.ui.mode == UIMode::Search && state.ui.active_pane == ActivePane::Right {
        render_search_bar(frame, halves[1], &state.search.query, colors);
    } else {
        let right_info = calculate_pane_info(&tab.right_pane);
        let right_para = Paragraph::new(Span::raw(right_info))
            .style(Style::default()
                .fg(parse_color(colors.get_pane_info_foreground()))
                .bg(parse_color(colors.get_pane_info_background())));
        frame.render_widget(right_para, halves[1]);
    }
}

/// Render the search bar (replacement for stats)
fn render_search_bar(frame: &mut Frame, area: Rect, query: &str, colors: &rwf_lib::config::ColorScheme) {
    use ratatui::text::Line;
    use ratatui::style::Color;

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
    let total_size: u64 = pane.entries.iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.size)
        .sum();
    
    format!(" {} {} {} {}  {}", 
        dir_count, dir_text, file_count, file_text, format_size(total_size))
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
