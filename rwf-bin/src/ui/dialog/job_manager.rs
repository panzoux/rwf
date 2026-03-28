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
use super::DialogAction;

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

impl JobManagerDialogState {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            focused_field: 0,  // Start with Job List focused
            job_list_focus_index: 0,
        }
    }
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
            let truncated_name = smart_truncate(&job.name, max_name_len);

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

/// Smart truncation preserving file extension
fn smart_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    
    // Try to preserve extension
    if let Some(dot_pos) = s.rfind('.') {
        let name = &s[..dot_pos];
        let ext = &s[dot_pos..];
        
        if name.len() <= 3 || max_len <= ext.len() + 4 {
            // Name too short or max_len too small, just truncate
            format!("{}...", &s[..max_len.saturating_sub(3)])
        } else {
            // Preserve extension: "very_long...ame.txt"
            let available = max_len - ext.len() - 3; // 3 for "..."
            format!("{}...{}", &name[..available], ext)
        }
    } else {
        // No extension, simple truncation
        format!("{}...", &s[..max_len.saturating_sub(3)])
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

/// Handle Job Manager Dialog input
/// 
/// Key Bindings (Part 6.6):
/// - Tab: Move focus forward (Job List → Close → Cancel)
/// - Shift+Tab: Move focus backward
/// - Up: Move selection up in job list (when Job List focused)
/// - Down: Move selection down in job list (when Job List focused)
/// - Enter: Activate focused button
/// - Escape: Close dialog
/// - C: Cancel selected job (shortcut, when Job List focused)
pub fn handle_job_manager_input(
    dialog_state: &mut JobManagerDialogState,
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> DialogAction {
    use crossterm::event::{KeyCode, KeyModifiers};
    
    // Escape: Cancel dialog
    if key.code == KeyCode::Esc {
        return DialogAction::Cancel;
    }
    
    // Tab navigation (Part 6.6, 6.7)
    if key.code == KeyCode::Tab {
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            // Shift+Tab: backwards (0→2→1→0)
            dialog_state.focused_field = match dialog_state.focused_field {
                0 => 2,  // Job List → Cancel
                1 => 0,  // Close → Job List
                2 => 1,  // Cancel → Close
                _ => 0,
            };
        } else {
            // Tab: forwards (0→1→2→0)
            dialog_state.focused_field = match dialog_state.focused_field {
                0 => 1,  // Job List → Close
                1 => 2,  // Close → Cancel
                2 => 0,  // Cancel → Job List
                _ => 0,
            };
        }
        return DialogAction::None;
    }
    
    // Up/Down navigation in Job List (Part 6.6)
    if dialog_state.focused_field == 0 {
        let jobs: Vec<BackgroundJob> = state.background_jobs.get_all_jobs()
            .cloned()
            .collect();

        if !jobs.is_empty() {
            // Ensure selected_index is within bounds
            if dialog_state.selected_index >= jobs.len() {
                dialog_state.selected_index = jobs.len() - 1;
            }
            if dialog_state.job_list_focus_index >= jobs.len() {
                dialog_state.job_list_focus_index = jobs.len() - 1;
            }

            if key.code == KeyCode::Up {
                if dialog_state.job_list_focus_index > 0 {
                    dialog_state.job_list_focus_index -= 1;
                }
                dialog_state.selected_index = dialog_state.job_list_focus_index;
                return DialogAction::None;
            }

            if key.code == KeyCode::Down {
                if dialog_state.job_list_focus_index < jobs.len() - 1 {
                    dialog_state.job_list_focus_index += 1;
                }
                dialog_state.selected_index = dialog_state.job_list_focus_index;
                return DialogAction::None;
            }

            // C key: Cancel selected job (Part 6.6)
            if key.code == KeyCode::Char('c') || key.code == KeyCode::Char('C') {
                let job_to_cancel = jobs.get(dialog_state.selected_index);
                if let Some(job) = job_to_cancel {
                    state.background_jobs.cancel_job(job.id.uuid);
                }
                return DialogAction::None;
            }
        }
    }

    // Enter: Activate button
    if key.code == KeyCode::Enter {
        match dialog_state.focused_field {
            1 => return DialogAction::Confirm,     // Close button
            2 => {
                // Terminate Job button
                let jobs: Vec<BackgroundJob> = state.background_jobs.get_all_jobs()
                    .cloned()
                    .collect();
                if let Some(job) = jobs.get(dialog_state.selected_index) {
                    state.background_jobs.cancel_job(job.id.uuid);
                }
                return DialogAction::None;
            }
            _ => {}
        }
    }

    DialogAction::None
}
