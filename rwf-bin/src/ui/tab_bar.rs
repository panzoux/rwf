//! Tab bar rendering
//!
//! This module renders the tab bar at the top of the screen.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::AppState;
use super::parse_color;

/// Render the tab bar
pub fn render_tab_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let colors = &state.config.display.colors;
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

        // Check if tab has active jobs
        let has_jobs = state.jobs.active.values().any(|job| {
            matches_tab_location(job, &tab.left_pane.current_location)
                || matches_tab_location(job, &tab.right_pane.current_location)
        });

        // Get shortened paths for left and right panes
        let left_path = shorten_path(&tab.left_pane.current_location.display_path(), 15);
        let right_path = shorten_path(&tab.right_pane.current_location.display_path(), 15);
        
        // Determine which pane is active (only for the active tab)
        let active_marker = if is_active {
            match state.ui.active_pane {
                rwf_lib::model::ActivePane::Left => format!("{}*|{}", left_path, right_path),
                rwf_lib::model::ActivePane::Right => format!("{}|{}*", left_path, right_path),
            }
        } else {
            format!("{}|{}", left_path, right_path)
        };

        // Format tab label with pane paths
        let label = if has_jobs {
            format!(" [~{}:{}~] ", idx + 1, active_marker)
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

/// Check if a job matches a tab location
fn matches_tab_location(
    job: &rwf_lib::job::Job,
    location: &rwf_lib::model::Location,
) -> bool {
    use rwf_lib::job::JobKind;

    match &job.spec.kind {
        JobKind::ReadDirectory { location: loc } => loc == location,
        JobKind::Copy { sources, dest } => {
            sources.iter().any(|s| s == location) || dest == location
        }
        JobKind::Move { sources, dest } => {
            sources.iter().any(|s| s == location) || dest == location
        }
        JobKind::Delete { targets } => targets.iter().any(|t| t == location),
        JobKind::Mkdir { location: loc } => loc == location,
        JobKind::Rename { from, .. } => from == location,
        JobKind::CalculateSize { location: loc } => loc == location,
        _ => false,
    }
}

/// Shorten a path to fit within a maximum length
fn shorten_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }
    
    // Try to show the last component (filename/directory)
    if let Some(last_sep) = path.rfind(|c| c == '/' || c == '\\') {
        let last_component = &path[last_sep + 1..];
        if last_component.len() <= max_len {
            return format!("...{}", last_component);
        }
    }
    
    // If even the last component is too long, truncate it
    format!("...{}", &path[path.len().saturating_sub(max_len - 3)..])
}
