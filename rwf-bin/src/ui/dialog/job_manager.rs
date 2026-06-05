//! Job Manager Dialog
//!
//! Displays active and recent background jobs with cancel functionality.
//! Following DIALOG_DESIGN_SPEC.md Part 6 specifications.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Color},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use rwf_lib::{AppState, job::BackgroundJob};
use crate::ui::unicode_utils::smart_truncate;

/// Job Manager Dialog state
/// 
/// Focus Fields (Part 6.7):
/// - 0: Job List (Up/Down moves selection)
/// - 1: Close Button
/// - 2: Cancel Job Button
/// 
/// Tab Order: 0 → 1 → 2 → 0 (wraps)
pub struct JobManagerDialogState {
    pub selected_index: usize,      // Selected job index in list
    pub focused_field: usize,       // 0=Job List, 1=Close, 2=Cancel
    pub job_list_focus_index: usize, // Which job has focus in the list
}


/// Get layout constraints for Job Manager Dialog (Part 6.2)
/// 
/// Content Area Height: 21 lines (inside the frame)
/// Frame: 2 lines (top border+title + bottom border)
/// Total Dialog Height: 23 lines (21 + 2)
pub fn get_job_manager_dialog_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Length(1),   // Spacing after title
        Constraint::Length(6),   // Job List: 4 items minimum + borders removed = 6 lines
        Constraint::Length(1),   // Detail label
        Constraint::Length(10),  // Detail view
        Constraint::Length(1),   // Spacing
        Constraint::Length(1),   // Buttons (WITHIN content area)
    ]
}

/// Calculate minimum dialog height from constraints
pub fn calculate_job_manager_dialog_min_height() -> u16 {
    get_job_manager_dialog_constraints()
        .iter()
        .map(|c| match c {
            Constraint::Length(n) => *n,
            _ => 0,
        })
        .sum::<u16>() + 2  // Add 2 for top and bottom borders
}

/// Render the Job Manager dialog
///
/// Layout (Part 6.2):
/// - Job List: 6 lines (4 items minimum, NO borders)
/// - Detail Label: 1 line
/// - Detail View: 10 lines (gray background, black text)
/// - Buttons: 1 line
pub fn render_job_manager_dialog(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    dialog_state: &JobManagerDialogState,
) {
    let colors = &state.config.display.colors;

    // Get all jobs
    let jobs: Vec<BackgroundJob> = state.background_jobs.get_all_jobs()
        .cloned()
        .collect();

    // Split area using constraints (buttons WITHIN content area - Part 1.3)
    // Constraints: [0]=Spacing, [1]=Job List, [2]=Detail label,
    //              [3]=Detail view, [4]=Spacing, [5]=Buttons
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(get_job_manager_dialog_constraints())
        .split(area);

    // Render job list (chunk[1]) - NO borders per user feedback
    render_job_list(
        frame,
        chunks[1],
        &jobs,
        dialog_state.job_list_focus_index,
        dialog_state.focused_field == 0,  // Is job list focused?
        colors,
    );

    // Render detail label (chunk[2])
    let detail_label = Paragraph::new("Selected Job Details:")
        .style(Style::default()
            .fg(Color::Black)
            .bg(Color::Gray));
    frame.render_widget(detail_label, chunks[2]);

    // Render detail view (chunk[3]) - Gray background, Black text (Part 2.2, 6.11)
    render_job_detail(
        frame,
        chunks[3],
        &jobs,
        dialog_state.selected_index,
        colors,
    );

    // Render buttons (chunk[5] - WITHIN content area per Part 1.3)
    render_buttons(
        frame,
        chunks[5],
        dialog_state.focused_field,
        colors,
    );
}

/// Render the job list (Part 6.3)
///
/// Display Format: [#{short_id}] [{status_char}] {truncated_name} - {percent}%
/// NO borders - items rendered directly
/// Selection: > = Selected item, white bg = focused item
fn render_job_list(
    frame: &mut Frame,
    area: Rect,
    jobs: &[BackgroundJob],
    focus_index: usize,
    is_focused: bool,
    _colors: &rwf_lib::config::ColorScheme,
) {
    let mut y_offset = 0;
    
    if jobs.is_empty() {
        // Show "No active jobs" with focus (Part 6.10 - Empty State)
        let style = if is_focused {
            // Focused item: Black on White (Part 2.2)
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            // Unfocused: Black on Gray (Part 2.2)
            Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
        };
        
        let line = Line::from(Span::styled("> No active jobs", style));
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, area.y + y_offset, area.width, 1));
        y_offset += 1;
    } else {
        for (idx, job) in jobs.iter().enumerate() {
            if y_offset >= area.height {
                break;
            }
            
            // Show progress percentage (always show, even at 0%)
            let percent = format!(" - {:.0}%", job.progress_percent * 100.0);

            let status_str = match job.status {
                rwf_lib::job::JobStatus::Pending => "Pending",
                rwf_lib::job::JobStatus::Running => "Running",
                rwf_lib::job::JobStatus::Completed => "Completed",
                rwf_lib::job::JobStatus::Failed => "Failed",
                rwf_lib::job::JobStatus::Cancelled => "Cancelled",
            };

            // Selection indicator (Part 6.3) - always show > for selected/focused
            let prefix = if idx == focus_index { "> " } else { "  " };

            // Smart truncation preserving extension
            let max_name_len = (area.width.saturating_sub(20) as usize).min(35);
            let truncated_name = smart_truncate(&job.name, max_name_len, "...");

            let line_text = format!(
                "{}#{} [{}] {}{}",
                prefix, job.id.short_id, status_str, truncated_name, percent
            );

            // Focus style (Part 2.2, 6.11 - only ONE item has white background)
            let style = if is_focused && idx == focus_index {
                // Focused item: Black on White (Part 2.2)
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                // Unfocused: Black on Gray (Part 2.2) - NOT white on dark gray!
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Gray)
            };

            let line = Line::from(Span::styled(line_text, style));
            frame.render_widget(Paragraph::new(line), Rect::new(area.x, area.y + y_offset, area.width, 1));
            y_offset += 1;
        }
    }
    
    // Fill remaining space with gray background
    for _ in y_offset..area.height {
        let blank = Line::from(Span::styled(
            " ".repeat(area.width as usize),
            Style::default().bg(Color::Gray),
        ));
        frame.render_widget(Paragraph::new(blank), Rect::new(area.x, area.y + y_offset, area.width, 1));
        y_offset += 1;
    }
}

/// Render job detail view (Part 6.4)
/// Gray background, Black text (Part 2.2, 6.11)
fn render_job_detail(
    frame: &mut Frame,
    area: Rect,
    jobs: &[BackgroundJob],
    selected_index: usize,
    _colors: &rwf_lib::config::ColorScheme,
) {
    let detail_text = if let Some(job) = jobs.get(selected_index) {
        // Format start time as HH:MM:SS in local timezone (JST)
        let start_time_local = chrono::DateTime::<chrono::Local>::from(job.start_time);
        let time_str = start_time_local.format("%H:%M:%S").to_string();

        // Show which tab the job is running on
        let location_info = format!("Tab: {}", job.tab_id + 1);

        format!(
            "Job ID: {:?}\n\
             Started: {}\n\
             Status: {:?}\n\
             Progress: {}\n\
             Current: {}\n\
             {}",
            job.id.uuid,
            time_str,
            job.status,
            job.progress_message,
            job.current_operation_detail,
            location_info,
        )
    } else {
        "No job selected".to_string()
    };

    let detail = Paragraph::new(detail_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Black)))
        .style(Style::default()
            .fg(Color::Black)
            .bg(Color::Gray))  // Gray background, Black text (Part 2.2, 6.11)
        .wrap(ratatui::widgets::Wrap { trim: false });  // Enable text wrapping

    frame.render_widget(detail, area);
}

/// Render buttons (Part 6.5)
///
/// Layout: [*Close*]  [Terminate Job]
/// Position: Bottom of content area, centered horizontally
/// Spacing: 2 spaces between buttons
///
/// [*Close*] is ALWAYS the default button (invoked on Enter)
/// Focus is indicated by WHITE background (Part 2.2, 2.3)
///
/// Focus Fields: 1=Close, 2=Terminate
fn render_buttons(
    frame: &mut Frame,
    area: Rect,
    focused_field: usize,
    _colors: &rwf_lib::config::ColorScheme,
) {
    // Button focus: 1=Close, 2=Terminate
    let close_focused = focused_field == 1;
    let terminate_focused = focused_field == 2;

    // Button style (Part 2.2, 6.11)
    let close_style = if close_focused {
        // Focused: Black on White
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        // Unfocused: Black on Gray
        Style::default()
            .fg(Color::Black)
            .bg(Color::Gray)
    };

    let terminate_style = if terminate_focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Gray)
    };

    // Button display: [*Close*] is ALWAYS default (Part 2.3)
    // Focus is shown by WHITE background, NOT asterisks
    let close_text = "[*Close*]";  // Always default
    let terminate_text = "[Terminate Job]";  // Never default

    // Center buttons horizontally
    let total_width = close_text.len() + 2 + terminate_text.len();
    let padding = (area.width as usize).saturating_sub(total_width) / 2;

    let line = Line::from(vec![
        Span::raw(" ".repeat(padding)),
        Span::styled(close_text, close_style),
        Span::raw("  "),  // 2 spaces between buttons (Part 6.5)
        Span::styled(terminate_text, terminate_style),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

