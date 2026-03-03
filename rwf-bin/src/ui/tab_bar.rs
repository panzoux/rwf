//! Tab bar rendering
//!
//! This module renders the tab bar at the top of the screen.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::AppState;

/// Render the tab bar
pub fn render_tab_bar(frame: &mut Frame, area: Rect, state: &AppState) {
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
            Style::default().fg(Color::Yellow),
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

        // Format tab label
        let label = if has_jobs {
            format!(" [~{}~] ", idx + 1)
        } else if is_active {
            format!(" [{}] ", idx + 1)
        } else {
            format!("  {}  ", idx + 1)
        };

        // Apply style
        let style = if is_active {
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray).bg(Color::Black)
        };

        spans.push(Span::styled(label, style));
    }
    
    // Add right scroll indicator if needed
    if end_idx < total_tabs {
        spans.push(Span::styled(
            " > ",
            Style::default().fg(Color::Yellow),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::Black));

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
