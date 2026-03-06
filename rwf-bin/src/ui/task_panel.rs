//! Task panel rendering
//!
//! This module renders the task panel showing active and queued jobs.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem},
    Frame,
};
use rwf_lib::AppState;
use super::parse_color;

/// Render the task panel
pub fn render_task_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    // NO BORDER - render directly to area
    let colors = &state.config.display.colors;
    
    let mut items = Vec::new();

    // Show queued jobs
    for spec in &state.jobs.queue {
        let job_desc = format_job_kind(&spec.kind);
        let line = Line::from(vec![
            Span::styled("[QUEUED] ", Style::default().fg(parse_color(&colors.warning_color))),
            Span::styled(job_desc, Style::default().fg(parse_color(&colors.foreground_color))),
        ]);
        items.push(ListItem::new(line));
    }

    // Show active jobs
    for job in state.jobs.active.values() {
        let job_desc = format_job_kind(&job.spec.kind);
        let progress_pct = (job.progress * 100.0) as u8;
        
        // Create a simple progress bar (e.g., [=====>    ] 50%)
        let bar_width = 20;
        let filled = ((job.progress * bar_width as f64) as usize).min(bar_width);
        let progress_bar = format!(
            "[{}{}] {}%",
            "=".repeat(filled.saturating_sub(1)),
            if filled > 0 { ">" } else { "" },
            progress_pct
        );
        
        let line = Line::from(vec![
            Span::styled("[RUNNING] ", Style::default().fg(parse_color(&colors.directory_color))),
            Span::styled(job_desc, Style::default().fg(parse_color(&colors.foreground_color))),
            Span::raw(" "),
            Span::styled(progress_bar, Style::default().fg(parse_color(&colors.ok_color))),
        ]);
        items.push(ListItem::new(line));
    }

    // Show recent completed jobs (only those completed within last 3 seconds)
    let now = std::time::SystemTime::now();
    let recent_completed: Vec<_> = state
        .jobs
        .completed
        .iter()
        .rev()
        .filter(|result| {
            // Only show jobs completed within last 3 seconds
            if let Ok(elapsed) = now.duration_since(result.completed_at) {
                elapsed.as_secs() < 3
            } else {
                false
            }
        })
        .collect();

    for result in recent_completed {
        let job_desc = format_job_kind(&result.kind);
        let status = match &result.result {
            rwf_lib::job::OpResult::Success(_) => {
                Span::styled("[DONE] ", Style::default().fg(parse_color(&colors.ok_color)))
            }
            rwf_lib::job::OpResult::Failed(err) => {
                Span::styled(format!("[FAILED: {}] ", err), Style::default().fg(parse_color(&colors.error_color)))
            }
            rwf_lib::job::OpResult::Cancelled => {
                Span::styled("[CANCELLED] ", Style::default().fg(parse_color(&colors.foreground_color)))
            }
        };
        let line = Line::from(vec![
            status,
            Span::styled(job_desc, Style::default().fg(parse_color(&colors.foreground_color))),
        ]);
        items.push(ListItem::new(line));
    }

    // If no items, show empty message
    if items.is_empty() {
        let empty_msg = Line::from(Span::styled(
            "No active tasks",
            Style::default().fg(parse_color(&colors.foreground_color)),
        ));
        items.push(ListItem::new(empty_msg));
    }

    // Apply scrolling
    let scroll_offset = state.ui.layout.task_panel_scroll_offset;
    let visible_height = area.height as usize;
    let total_items = items.len();
    
    // Calculate which items to display based on scroll offset
    let start_idx = scroll_offset.min(total_items.saturating_sub(1));
    let end_idx = (start_idx + visible_height).min(total_items);
    let visible_items: Vec<_> = items.into_iter().skip(start_idx).take(end_idx - start_idx).collect();
    
    let list = List::new(visible_items);
    frame.render_widget(list, area);
}

/// Format job kind as a human-readable string
fn format_job_kind(kind: &rwf_lib::job::JobKind) -> String {
    use rwf_lib::job::JobKind;

    match kind {
        JobKind::ReadDirectory { location } => {
            format!("Reading {}", location.display_path())
        }
        JobKind::Copy { sources, dest } => {
            format!("Copying {} file(s) to {}", sources.len(), dest.display_path())
        }
        JobKind::Move { sources, dest } => {
            format!("Moving {} file(s) to {}", sources.len(), dest.display_path())
        }
        JobKind::Delete { targets } => {
            format!("Deleting {} file(s)", targets.len())
        }
        JobKind::Mkdir { location } => {
            format!("Creating directory {}", location.display_path())
        }
        JobKind::Rename { from, to } => {
            format!("Renaming {} to {}", from.display_path(), to.display_path())
        }
        JobKind::CalculateSize { location } => {
            format!("Calculating size of {}", location.display_path())
        }
        JobKind::ExtractArchive { archive, dest } => {
            format!("Extracting {} to {}", archive.display_path(), dest.display_path())
        }
        JobKind::CreateArchive { sources, dest } => {
            format!("Creating archive {} with {} file(s)", dest.display_path(), sources.len())
        }
        JobKind::ExecuteCustomFunction { command, .. } => {
            format!("Executing: {}", command)
        }
        JobKind::Search { location, pattern, .. } => {
            format!("Searching for '{}' in {}", pattern, location.display_path())
        }
        JobKind::LoadFileForViewer { location } => {
            format!("Loading file {}", location.display_path())
        }
        JobKind::PatternRename { pattern, .. } => {
            format!("Pattern rename: {}", pattern)
        }
        JobKind::CompareFiles { left, right } => {
            format!("Comparing {} and {}", left.display_path(), right.display_path())
        }
        JobKind::SplitFile { source, .. } => {
            format!("Splitting file {}", source.display_path())
        }
        JobKind::JoinFiles { parts, dest } => {
            format!("Joining {} parts to {}", parts.len(), dest.display_path())
        }
    }
}
