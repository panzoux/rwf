//! Dialog system with centralized input handling
//!
//! This module provides a hybrid dialog system with:
//! - Common infrastructure (border, title, buttons, centering)
//! - Content-specific rendering via trait
//! - Centralized input handling with consistent shortcuts

mod compression;
mod extract_confirm;
mod frame;
mod job_manager;

pub use compression::{render_compression_dialog, CompressionDialogState};
pub use extract_confirm::ExtractionConfirmDialog;
pub use frame::{centered_rect, render_dialog_buttons, render_dialog_frame};
pub use job_manager::{
    render_job_manager_dialog, 
    JobManagerDialogState, 
    calculate_job_manager_dialog_min_height,
};

use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::model::dialog::{Dialog, DialogContent};
use tracing::debug;

use super::smart_truncate;

/// Result of dialog input handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DialogAction {
    None,           // Input consumed, no action
    Confirm,        // User pressed Enter/OK
    ConfirmAll,     // Shift+Enter: apply to all remaining
    Cancel,         // User pressed Escape/Cancel
    NextField,      // Tab pressed
    PrevField,      // Shift+Tab pressed
    NavigateUp,     // Arrow up in list
    NavigateDown,   // Arrow down in list
    TextInput(char), // Character typed for text input
    Backspace,      // Backspace in text input
}

/// Trait for dialog content rendering and input handling
pub trait DialogContentRenderer {
    /// Render the content-specific widgets
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);
    
    /// Handle input, return action
    fn handle_input(&mut self, key: KeyEvent) -> DialogAction;
    
    /// Get number of focusable fields (for Tab navigation)
    fn field_count(&self) -> usize { 1 }
    
    /// Get currently focused field index
    fn focused_field(&self) -> usize { 0 }
    
    /// Set focused field index
    fn set_focused_field(&mut self, index: usize);
}

/// Render a dialog overlay centered on screen
pub fn render_dialog(frame: &mut Frame, dialog: &Dialog, state: &rwf_lib::AppState) {
    // Calculate minimum dialog height based on content type BEFORE rendering (Part 1.1, 1.2)
    let min_content_height = match &dialog.content {
        DialogContent::Compression { .. } => {
            // Calculate from actual layout constraints
            crate::ui::dialog::compression::calculate_compression_dialog_min_height()
        }
        DialogContent::ExtractionConfirm { .. } => {
            // Extraction dialog: ~6 lines content
            6u16
        }
        DialogContent::JobManager { .. } => {
            // Job Manager dialog: calculate from constraints (Part 6.2)
            calculate_job_manager_dialog_min_height()
        }
        DialogContent::FileConflict { .. } => {
            // File Conflict dialog: 19 lines content + 5 for buttons = 24 lines
            24u16
        }
        _ => 8u16, // Default
    };

    // Add 2 for borders (top + bottom)
    let min_dialog_height = min_content_height + 2;

    let screen_height = frame.area().height;
    let screen_width = frame.area().width;

    // For compression and job manager dialogs, use exact minimum height (no extra space)
    // For other dialogs, use 70% of screen or minimum, whichever is larger
    let dialog_height = match &dialog.content {
        DialogContent::Compression { .. } | DialogContent::JobManager { .. } => {
            // Use exact minimum height, but ensure it fits on screen
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        DialogContent::FileConflict { .. } => {
            // Use exact minimum height for file conflict dialog
            min_dialog_height.min(screen_height.saturating_sub(2))
        }
        _ => {
            // Use 70% of screen or minimum, whichever is larger
            let percent_height = (screen_height * 70) / 100;
            percent_height.max(min_dialog_height).min(screen_height.saturating_sub(2))
        }
    };

    // Convert to percentage for centered_rect
    let height_percent = if screen_height > 0 {
        ((dialog_height as u32 * 100) / screen_height as u32) as u16
    } else {
        70
    };

    // For Job Manager dialog, use fixed width of 64 (Part 6.2)
    // For File Conflict dialog, use min 64, max 80% of terminal width
    let width_percent = match &dialog.content {
        DialogContent::JobManager { .. } => {
            // Calculate width percent for 64 characters
            let screen_width = frame.area().width;
            if screen_width > 0 {
                // Cap at 95% to leave margins, minimum 60%
                let calculated = ((64u32 * 100) / screen_width as u32) as u16;
                calculated.min(95).max(60)
            } else {
                60
            }
        }
        DialogContent::FileConflict { .. } => {
            // Min 64 chars, max 80% of terminal width
            let screen_width = frame.area().width;
            if screen_width > 0 {
                let max_width = ((screen_width as u32 * 80) / 100) as u16;
                let min_width = 64u16;
                let dialog_width = min_width.max(max_width.min(min_width));
                let calculated = ((dialog_width as u32 * 100) / screen_width as u32) as u16;
                calculated.min(95).max(50)
            } else {
                60
            }
        }
        _ => 60,  // Default 60% for other dialogs
    };

    let area = centered_rect(width_percent, height_percent, frame.area());

    // Render common frame (border, title)
    let content_area = render_dialog_frame(frame, &dialog.title, area);

    // Render dialog based on type
    match &dialog.content {
        DialogContent::Compression { .. } => {
            // Render compression dialog using exact content area (buttons rendered within)
            render_dialog_content(frame, &dialog.content, content_area, true);
        }
        DialogContent::JobManager { selected_index, focused_field } => {
            // Render Job Manager dialog with its own layout (Part 6.2)
            let dialog_state = JobManagerDialogState {
                selected_index: *selected_index,
                focused_field: *focused_field,
                job_list_focus_index: *selected_index,
            };
            render_job_manager_dialog(frame, content_area, state, &dialog_state);
        }
        DialogContent::CloseTabWithActiveJob { tab_name, job_ids, focused_field, .. } => {
            // Render Close Tab confirmation dialog with buttons (compact layout)
            let job_list = if job_ids.len() == 1 {
                format!("Job #{} is still running.", job_ids[0])
            } else {
                let job_strs: Vec<String> = job_ids.iter().map(|id| format!("#{}", id)).collect();
                format!("Jobs {} are still running.", job_strs.join(", "))
            };
            let message = format!("{} {}\nClose this tab and cancel the job(s)?", tab_name, job_list);

            // Use compact layout: message takes remaining space, buttons fixed at 3 lines
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(2),  // Message (compact)
                    Constraint::Length(3), // Buttons
                ])
                .split(content_area);

            let confirmation = Paragraph::new(message)
                .style(Style::default()
                    .fg(Color::Black)
                    .bg(Color::Gray));

            frame.render_widget(confirmation, chunks[0]);

            // Render buttons (OK/Cancel) with proper focus
            render_dialog_buttons(frame, chunks[1], &dialog.content, *focused_field);
        }
        DialogContent::FileConflict { conflicts, current_index, focused_button, rename_text, rename_cursor, rename_scroll, edit_mode, vi_mode, error_message, .. } => {
            // Render File Conflict dialog with TextInput widget
            render_file_conflict_dialog(frame, content_area, conflicts, *current_index, *focused_button, rename_text, *rename_cursor, *rename_scroll, *edit_mode, *vi_mode, error_message);
        }
        _ => {
            // Split content area for buttons (generic layout)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(5),  // Content
                    Constraint::Length(3), // Buttons
                ])
                .split(content_area);

            // Render content-specific widgets
            render_dialog_content(frame, &dialog.content, chunks[0], true);

            // Render buttons (OK/Cancel or custom)
            let focused_button = 0; // Default for other dialog types
            render_dialog_buttons(frame, chunks[1], &dialog.content, focused_button);
        }
    }
}

/// Render File Conflict dialog (compact layout with vertical buttons)
fn render_file_conflict_dialog(
    frame: &mut Frame,
    area: Rect,
    conflicts: &[rwf_lib::model::dialog::ConflictPair],
    current_index: usize,
    focused_button: usize,
    rename_text: &str,
    rename_cursor: usize,
    rename_scroll: usize,
    edit_mode: rwf_lib::config::EditMode,
    vi_mode: Option<rwf_lib::config::ViMode>,
    error_message: &Option<String>,
) {
    let current = &conflicts[current_index];
    let (indicator, message) = current.get_status_message();
    let textbox_width = area.width.saturating_sub(20) as usize;  // Leave space for button

    // Line 0: Filename
    let filename_line = format!("Filename: {}", smart_truncate(&current.source.name, textbox_width, "..."));
    let filename_para = Paragraph::new(filename_line)
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(filename_para, Rect::new(area.x, area.y, area.width, 1));

    // Line 1: "From:" label
    let from_label = Paragraph::new("From:")
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(from_label, Rect::new(area.x, area.y + 1, area.width, 1));

    // Line 2: From path (smart truncate to show beginning and end, no "Path:" prefix)
    let from_path = format!("  {}", smart_truncate_path(&current.source_path.display_path(), textbox_width - 2));
    let from_path_para = Paragraph::new(from_path)
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(from_path_para, Rect::new(area.x, area.y + 2, area.width, 1));

    // Line 3: From size,date
    let from_info = format!("  Size,Date: {} Bytes, {}",
        current.source.size,
        chrono::DateTime::<chrono::Local>::from(current.source.modified).format("%Y-%m-%d %H:%M:%S"));
    let from_info_para = Paragraph::new(from_info)
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(from_info_para, Rect::new(area.x, area.y + 3, area.width, 1));

    // Line 4: Blank

    // Line 5: "To:" label
    let to_label = Paragraph::new("To:")
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(to_label, Rect::new(area.x, area.y + 5, area.width, 1));

    // Line 6: To path (smart truncate to show beginning and end, no "Path:" prefix)
    let to_path = format!("  {}", smart_truncate_path(&current.dest_path.display_path(), textbox_width - 2));
    let to_path_para = Paragraph::new(to_path)
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(to_path_para, Rect::new(area.x, area.y + 6, area.width, 1));

    // Line 7: To size,date
    let to_info = format!("  Size,Date: {} Bytes, {}",
        current.dest.size,
        chrono::DateTime::<chrono::Local>::from(current.dest.modified).format("%Y-%m-%d %H:%M:%S"));
    let to_info_para = Paragraph::new(to_info)
        .style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(to_info_para, Rect::new(area.x, area.y + 7, area.width, 1));

    // Line 8: Blank

    // Line 9: Status indicator
    let status_line = format!("{} {}", indicator, message);
    let status_style = if indicator == "✓" {
        Style::default().fg(Color::Green).bg(Color::Gray)
    } else {
        Style::default().fg(Color::Yellow).bg(Color::Gray)
    };
    let status_para = Paragraph::new(status_line)
        .style(status_style);
    frame.render_widget(status_para, Rect::new(area.x, area.y + 9, area.width, 1));

    // Line 10: Blank

    // Lines 11-15: Buttons (vertical layout)
    // Focus fields: 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename button, 4=Textbox, 5=Cancel
    let button_texts = [
        ("Force Overwrite", false),
        ("Overwrite if New", false),
        ("Skip", false),
        ("Rename", false),
        ("Cancel", false),
    ];

    let mut button_y = area.y + 11;
    for (i, (label, is_default)) in button_texts.iter().enumerate() {
        let button_text = if *is_default {
            format!("[*{}*]", label)
        } else {
            format!("[{}]", label)
        };

        // Map array index to focus field:
        // Array: 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename, 4=Cancel
        // Focus: 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename, 4=Textbox, 5=Cancel
        let button_focus_field = if i >= 4 { i + 1 } else { i };
        let button_is_focused = focused_button == button_focus_field;

        // For Rename button (field 3), add textbox on the right (field 4)
        if i == 3 {
            let button_style = if button_is_focused {
                Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Black).bg(Color::Gray)
            };

            // Left-align button
            let button_para = Paragraph::new(button_text.clone()).style(button_style);
            frame.render_widget(button_para, Rect::new(area.x + 2, button_y, button_text.len() as u16, 1));

            // Render textbox using TextInput widget (dedicated focus field 4)
            let textbox_x = area.x + 2 + button_text.len() as u16 + 2;
            let textbox_is_focused = focused_button == 4;
            let textbox_width_u16 = textbox_width as u16;

            // Create TextInput widget for rendering
            let mut text_input = crate::ui::text_input::TextInput::new(Some(rename_text.to_string()), edit_mode);
            // Restore Vi mode state
            if let Some(vm) = vi_mode {
                text_input.set_vi_mode(vm);
            }
            text_input.set_width(textbox_width_u16);
            text_input.set_cursor(rename_cursor);
            text_input.render(frame, Rect::new(textbox_x, button_y, textbox_width_u16, 1), textbox_is_focused);
        } else {
            let button_style = if button_is_focused {
                Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Black).bg(Color::Gray)
            };
            let button_para = Paragraph::new(button_text.clone()).style(button_style);
            frame.render_widget(button_para, Rect::new(area.x + 2, button_y, button_text.len() as u16, 1));
        }

        button_y += 1;
    }

    // Line after buttons: Hint text (shown when Force, Overwrite if New, or Skip focused)
    let hint_y = button_y;
    if focused_button == 0 || focused_button == 1 || focused_button == 2 {
        let hint = Paragraph::new("(Shift+Enter for the rest)")
            .style(Style::default().fg(Color::DarkGray).bg(Color::Gray));
        frame.render_widget(hint, Rect::new(area.x, hint_y, area.width, 1));
    }

    // Error message line
    let error_y = hint_y + 1;
    if let Some(error) = error_message {
        let error_para = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red).bg(Color::Gray));
        frame.render_widget(error_para, Rect::new(area.x, error_y, area.width, 1));
    }
}

/// Render textbox with visible cursor and horizontal scrolling
fn render_textbox_with_cursor(
    frame: &mut Frame,
    text: &str,
    cursor_pos: usize,
    scroll: usize,
    width: usize,
    x: u16,
    y: u16,
    style: Style,
    is_focused: bool,
) {
    // Get visible portion of text
    let visible_end = (scroll + width).min(text.len());
    let visible_text = if scroll < text.len() {
        &text[scroll..visible_end]
    } else {
        ""
    };

    // Calculate cursor position within visible text
    let visible_cursor = cursor_pos.saturating_sub(scroll);

    // Build the textbox content with cursor
    let mut spans = Vec::new();

    // Add visible text with cursor position
    if visible_cursor < visible_text.len() {
        // Cursor in middle of visible text
        let before_cursor = &visible_text[..visible_cursor];
        let cursor_char = visible_text.chars().nth(visible_cursor).unwrap_or(' ');
        let after_cursor = if visible_cursor + 1 < visible_text.len() {
            &visible_text[visible_cursor + 1..]
        } else {
            ""
        };

        spans.push(Span::raw(before_cursor));
        if is_focused {
            // Show cursor as underscore block with cyan background for visibility
            spans.push(Span::styled(
                cursor_char.to_string(),
                Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        } else {
            spans.push(Span::raw(cursor_char.to_string()));
        }
        spans.push(Span::raw(after_cursor));
    } else {
        // Cursor at end
        spans.push(Span::raw(visible_text));
        if is_focused {
            // Show cursor block at end
            spans.push(Span::styled("█", Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)));
        }
    }

    // Pad to fill width
    let current_len = spans.iter().map(|s| s.content.len()).sum::<usize>();
    if current_len < width {
        spans.push(Span::raw(" ".repeat(width - current_len)));
    }

    let textbox = Paragraph::new(Line::from(spans)).style(style);
    frame.render_widget(textbox, Rect::new(x, y, width as u16, 1));
}

/// Smart truncate path to show beginning and end of both directory path and filename
/// e.g. "C:\Users\user\source\repos\panzoux\rwf\BUGFIX_COLOR_SPACE_SUMMARY.md"
///   => "C:\Users\us...\panzoux\BUGFIX...CE_SUMMARY.md"
fn smart_truncate_path(path: &str, max_width: usize) -> String {
    if path.len() <= max_width {
        return path.to_string();
    }

    // Split into directory path and filename
    let path_obj = std::path::Path::new(path);
    let filename = path_obj.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir_path = path_obj.parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // Calculate available space for truncation
    // Need space for: "dir.../filename..." with "..." separators
    let separator = "...";
    let min_dir = 10;  // Minimum chars to show from directory
    let min_file_start = 5;  // Minimum chars from filename start
    let min_file_end = 10;  // Minimum chars from filename end

    if max_width < min_dir + min_file_start + min_file_end + separator.len() * 2 {
        // Too small, just truncate normally
        return smart_truncate(path, max_width, "...");
    }

    // Truncate directory path
    let dir_truncated = if dir_path.len() > min_dir + separator.len() {
        format!("{}{}", &dir_path[..min_dir], separator)
    } else {
        dir_path.clone()
    };

    // Truncate filename
    let file_truncated = if filename.len() > min_file_start + min_file_end + separator.len() {
        format!("{}{}{}", &filename[..min_file_start], separator, &filename[filename.len() - min_file_end..])
    } else {
        filename.clone()
    };

    format!("{}{}", dir_truncated, file_truncated)
}

/// Render dialog content based on type
fn render_dialog_content(frame: &mut Frame, content: &DialogContent, area: Rect, focused: bool) {
    match content {
        DialogContent::Compression { 
            archive_name, 
            selected_format_index,
            selected_compression_index,
            focused_field,
            format_focus_index,
            compression_focus_index,
            cursor_pos,
            ..
        } => {
            // Create state from embedded dialog state
            let state = CompressionDialogState {
                archive_name: archive_name.clone(),
                selected_format_index: *selected_format_index,
                selected_compression_index: *selected_compression_index,
                focused_field: *focused_field,
                format_focus_index: *format_focus_index,
                compression_focus_index: *compression_focus_index,
                cursor_pos: *cursor_pos,
            };
            render_compression_dialog(frame, area, &state, focused);
        }
        DialogContent::ExtractionConfirm { archive, dest, file_count } => {
            let dialog = ExtractionConfirmDialog {
                archive_name: archive.display_path(),
                dest_path: dest.display_path(),
                file_count: *file_count,
            };
            dialog.render(frame, area, focused);
        }
        _ => {}
    }
}

/// Validate filename for cross-platform compatibility
/// Returns error message if invalid, None if valid
fn validate_filename(rename_text: &str, original_name: &str) -> Option<String> {
    // Check if empty
    if rename_text.is_empty() {
        return Some("Filename cannot be empty".to_string());
    }
    
    // Check if same as original
    if rename_text == original_name {
        return Some("Same filename - no change needed".to_string());
    }
    
    // Windows invalid characters: < > : " / \ | ? *
    let win_invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if rename_text.chars().any(|c| win_invalid.contains(&c)) {
        return Some("Invalid chars for Windows: < > : \" / \\ | ? *".to_string());
    }
    
    // Mac invalid character: :
    let mac_invalid = [':'];
    if rename_text.chars().any(|c| mac_invalid.contains(&c)) {
        return Some("Invalid char for Mac: :".to_string());
    }
    
    // Linux invalid character: /
    let linux_invalid = ['/'];
    if rename_text.chars().any(|c| linux_invalid.contains(&c)) {
        return Some("Invalid char for Linux: /".to_string());
    }
    
    // Check reserved names (Windows)
    let reserved_names = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
    let name_without_ext = std::path::Path::new(rename_text)
        .file_stem()
        .map(|s| s.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    if reserved_names.contains(&name_without_ext.as_str()) {
        return Some("Reserved system name".to_string());
    }
    
    None  // Valid
}

/// Handle File Conflict dialog input with TextInput widget
/// Focus fields: 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename button, 4=Textbox, 5=Cancel
fn handle_file_conflict_input(
    conflicts: &mut Vec<rwf_lib::model::dialog::ConflictPair>,
    current_index: &mut usize,
    focused_button: &mut usize,
    rename_text: &mut String,
    rename_cursor: &mut usize,
    _rename_scroll: &mut usize,
    edit_mode: &mut rwf_lib::config::EditMode,
    vi_mode: &mut Option<rwf_lib::config::ViMode>,
    error_message: &mut Option<String>,
    decisions: &mut Vec<rwf_lib::model::dialog::ConflictAction>,
    pending_find_backward: &mut Option<bool>,
    pending_operator: &mut Option<u8>,
    pending_ctrl_x: &mut bool,
    key: KeyEvent,
) -> DialogAction {
    use crossterm::event::KeyCode;
    use crate::ui::text_input::{TextInput, TextInputAction};
    use rwf_lib::config::ViMode;

    let is_textbox_focused = *focused_button == 4;

    // Clear error on focus change
    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
        *error_message = None;
    }

    // If textbox is focused, delegate to TextInput
    if is_textbox_focused {
        let mut text_input = TextInput::new(Some(rename_text.clone()), *edit_mode);
        text_input.set_cursor(*rename_cursor);
        // Set original text for Vi U command
        text_input.set_original_text(rename_text.clone());
        // Restore Vi mode state
        if let Some(vm) = vi_mode {
            text_input.set_vi_mode(*vm);
        }
        // Restore pending states (convert u8 to ViOperator)
        text_input.set_pending_find_backward(*pending_find_backward);
        text_input.set_pending_operator(match pending_operator {
            Some(1) => Some(crate::ui::text_input::ViOperator::Change),
            Some(2) => Some(crate::ui::text_input::ViOperator::Delete),
            _ => None,
        });
        text_input.set_pending_ctrl_x(*pending_ctrl_x);

        let action = text_input.handle_input(&key);

        // Always sync state from TextInput (DRY - widget owns all state)
        *rename_text = text_input.text().to_string();
        *rename_cursor = text_input.cursor();
        *edit_mode = text_input.mode();
        // Sync Vi mode state
        *vi_mode = text_input.vi_mode();
        // Sync pending states (convert ViOperator to u8)
        *pending_find_backward = text_input.pending_find_backward();
        *pending_operator = match text_input.pending_operator() {
            Some(crate::ui::text_input::ViOperator::Change) => Some(1),
            Some(crate::ui::text_input::ViOperator::Delete) => Some(2),
            None => None,
        };
        *pending_ctrl_x = text_input.pending_ctrl_x();

        match action {
            TextInputAction::TextChanged => {
                // Validate immediately on text change
                let original_name = if !conflicts.is_empty() {
                    &conflicts[*current_index].source.name
                } else {
                    ""
                };
                *error_message = validate_filename(rename_text, original_name);
                return DialogAction::None;
            }
            TextInputAction::CursorMoved => {
                // Cursor moved, clear error if it was a validation error
                // (user might be navigating to fix the issue)
                return DialogAction::None;
            }
            TextInputAction::ModeToggled | TextInputAction::ModeChanged => {
                // Mode changed, just continue
                debug!("TextInput mode changed in FileConflict dialog (textbox focused), edit_mode={:?}, vi_mode={:?}", edit_mode, vi_mode);
                return DialogAction::None;
            }
            TextInputAction::Confirm => {
                // Validate and confirm
                let original_name = if !conflicts.is_empty() {
                    &conflicts[*current_index].source.name
                } else {
                    ""
                };
                match validate_filename(rename_text, original_name) {
                    Some(err) => {
                        *error_message = Some(err);
                        return DialogAction::None;
                    }
                    None => {
                        *error_message = None;
                        decisions.push(rwf_lib::model::dialog::ConflictAction::Rename {
                            new_name: rename_text.clone()
                        });
                        return DialogAction::Confirm;
                    }
                }
            }
            TextInputAction::Cancel => {
                // In Vi mode, behavior depends on current vi_mode state
                debug!("TextInput Cancel action in FileConflict dialog (textbox focused), edit_mode={:?}, vi_mode={:?}", edit_mode, vi_mode);
                if *edit_mode == rwf_lib::config::EditMode::Vi {
                    match vi_mode {
                        Some(ViMode::Normal) => {
                            // Already in Normal mode - cancel the dialog
                            debug!("Vi-Normal mode, canceling dialog");
                            return DialogAction::Cancel;
                        }
                        Some(ViMode::Insert) | None => {
                            // In Insert mode - switch to Normal mode
                            *vi_mode = Some(ViMode::Normal);
                            debug!("Switching to Normal mode from textbox Cancel");
                            return DialogAction::None;
                        }
                    }
                }
                debug!("Emacs mode, returning Cancel action");
                return DialogAction::Cancel;
            }
            TextInputAction::NextField => {
                *focused_button = (*focused_button + 1) % 6;
                return DialogAction::None;
            }
            TextInputAction::PrevField => {
                *focused_button = if *focused_button == 0 { 5 } else { *focused_button - 1 };
                return DialogAction::None;
            }
            TextInputAction::None => return DialogAction::None,
        }
    }

    // Handle non-textbox focus
    match key.code {
        KeyCode::Esc => {
            // In Vi mode, behavior depends on current vi_mode state
            debug!("Esc pressed in FileConflict dialog (non-textbox), edit_mode={:?}, current vi_mode={:?}", edit_mode, vi_mode);
            if *edit_mode == rwf_lib::config::EditMode::Vi {
                match vi_mode {
                    Some(ViMode::Normal) => {
                        // Already in Normal mode - Esc cancels the dialog
                        debug!("Vi-Normal mode, returning Cancel action");
                        DialogAction::Cancel
                    }
                    Some(ViMode::Insert) | None => {
                        // In Insert mode - switch to Normal mode
                        debug!("Switching to Normal mode (Vi Insert mode active)");
                        *vi_mode = Some(ViMode::Normal);
                        DialogAction::None
                    }
                }
            } else {
                debug!("Emacs mode active, returning Cancel action");
                DialogAction::Cancel
            }
        }
        KeyCode::Char('[') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+[: In Vi mode, switches to Normal mode
            if *edit_mode == rwf_lib::config::EditMode::Vi {
                *vi_mode = Some(ViMode::Normal);
                DialogAction::None
            } else {
                DialogAction::Cancel
            }
        }

        KeyCode::Tab => {
            *focused_button = (*focused_button + 1) % 6;  // 6 fields now
            DialogAction::None
        }

        KeyCode::BackTab => {
            *focused_button = if *focused_button == 0 { 5 } else { *focused_button - 1 };
            DialogAction::None
        }

        KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Enter: Apply to ALL remaining
                let action = button_index_to_action(*focused_button);
                for _ in 0..(conflicts.len() - *current_index) {
                    decisions.push(action.clone());
                }
                DialogAction::ConfirmAll
            } else {
                // Enter: Apply to current only
                match *focused_button {
                    0 | 1 | 2 | 5 => {
                        // Force, OverwriteIfNew, Skip, Cancel buttons
                        let action = button_index_to_action(*focused_button);
                        decisions.push(action);
                        DialogAction::Confirm
                    }
                    3 => {
                        // Rename button: move focus to textbox
                        *focused_button = 4;
                        DialogAction::None
                    }
                    4 => {
                        // Textbox: handled above
                        DialogAction::None
                    }
                    _ => DialogAction::None,
                }
            }
        }

        _ => DialogAction::None,
    }
}

/// Convert button index to ConflictAction
fn button_index_to_action(index: usize) -> rwf_lib::model::dialog::ConflictAction {
    match index {
        0 => rwf_lib::model::dialog::ConflictAction::Force,
        1 => rwf_lib::model::dialog::ConflictAction::OverwriteIfNewer,
        2 => rwf_lib::model::dialog::ConflictAction::Skip,
        3 => rwf_lib::model::dialog::ConflictAction::Rename { new_name: String::new() },
        4 => rwf_lib::model::dialog::ConflictAction::Rename { new_name: String::new() },  // Textbox
        5 => rwf_lib::model::dialog::ConflictAction::Skip,  // Cancel = skip this file
        _ => rwf_lib::model::dialog::ConflictAction::Skip,
    }
}

/// Handle dialog input centrally
pub fn handle_dialog_input(dialog: &mut Dialog, key: KeyEvent) -> DialogAction {
    // Note: Esc handling is delegated to individual dialog handlers
    // - FileConflict: Esc cancels (Emacs) or switches to Normal mode (Vi)
    // - Other dialogs: Esc cancels

    // Enter = Confirm (but depends on focused field for JobManager)
    if key.code == crossterm::event::KeyCode::Enter {
        // For JobManager dialog, check which field has focus
        if let DialogContent::JobManager { focused_field, .. } = &dialog.content {
            match *focused_field {
                1 => return DialogAction::Confirm,  // Close button focused
                2 => return DialogAction::Confirm,  // Cancel Job button focused
                _ => {}                              // Job List focused, Enter does nothing
            }
        } else if let DialogContent::FileConflict { .. } = &dialog.content {
            // FileConflict dialog handles Enter internally (for buttons and textbox)
            // Don't return here, let it be handled below
        } else {
            return DialogAction::Confirm;
        }
    }

    // CloseTabWithActiveJob dialog - Enter confirms, Escape cancels, Tab cycles
    if let DialogContent::CloseTabWithActiveJob { focused_field, .. } = &mut dialog.content {
        if key.code == crossterm::event::KeyCode::Enter {
            return DialogAction::Confirm;
        }
        if key.code == crossterm::event::KeyCode::Esc {
            return DialogAction::Cancel;
        }
        // Tab key cycles between OK (field 0) and Cancel (field 1) buttons
        if key.code == crossterm::event::KeyCode::Tab {
            // Cycle: 0→1→0 (OK→Cancel→OK)
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Tab: backwards
                *focused_field = if *focused_field == 0 { 1 } else { 0 };
            } else {
                // Tab: forwards
                *focused_field = if *focused_field == 0 { 1 } else { 0 };
            }
            return DialogAction::None;
        }
    }

    // FileConflict dialog - custom input handling with textbox
    if let DialogContent::FileConflict { conflicts, current_index, focused_button, rename_text, rename_cursor, rename_scroll, edit_mode, vi_mode, error_message, decisions, vi_pending_find_backward, vi_pending_operator, vi_pending_ctrl_x, .. } = &mut dialog.content {
        return handle_file_conflict_input(conflicts, current_index, focused_button, rename_text, rename_cursor, rename_scroll, edit_mode, vi_mode, error_message, decisions, vi_pending_find_backward, vi_pending_operator, vi_pending_ctrl_x, key);
    }

    // Compression dialog - Vi mode support for Esc
    if let DialogContent::Compression { edit_mode, vi_mode, .. } = &mut dialog.content {
        if key.code == crossterm::event::KeyCode::Esc {
            debug!("Esc pressed in Compression dialog, edit_mode={:?}, current vi_mode={:?}", edit_mode, vi_mode);
            if *edit_mode == rwf_lib::config::EditMode::Vi {
                debug!("Switching to Normal mode in Compression dialog");
                *vi_mode = Some(rwf_lib::config::ViMode::Normal);
                return DialogAction::None;
            } else {
                debug!("Emacs mode active in Compression dialog, returning Cancel");
                return DialogAction::Cancel;
            }
        }
    }

    // Tab navigation - cycles through dialog fields
    if key.code == crossterm::event::KeyCode::Tab {
        // Handle JobManager dialog Tab navigation (Part 6.6, 6.7)
        if let DialogContent::JobManager { focused_field, .. } = &mut dialog.content {
            // Cycle: 0→1→2→0 (Job List→Close→Cancel→Job List)
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Tab: backwards
                *focused_field = match *focused_field {
                    0 => 2,  // Job List → Cancel
                    1 => 0,  // Close → Job List
                    2 => 1,  // Cancel → Close
                    _ => 0,
                };
            } else {
                // Tab: forwards
                *focused_field = match *focused_field {
                    0 => 1,  // Job List → Close
                    1 => 2,  // Close → Cancel
                    2 => 0,  // Cancel → Job List
                    _ => 0,
                };
            }
            return DialogAction::None; // State updated, no further action
        }

        // Handle Compression dialog Tab navigation
        if let DialogContent::Compression { focused_field, .. } = &mut dialog.content {
            // Cycle: 0→1→2→3→4→0 (format→compression→name→OK→Cancel→format)
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Tab: backwards
                *focused_field = if *focused_field == 0 { 4 } else { *focused_field - 1 };
            } else {
                // Tab: forwards
                *focused_field = (*focused_field + 1) % 5;
            }
            return DialogAction::None; // State updated, no further action
        }
        return if key.modifiers.contains(KeyModifiers::SHIFT) {
            DialogAction::PrevField
        } else {
            DialogAction::NextField
        };
    }

    // Esc cancels dialog (for dialogs that don't handle it themselves)
    // Note: FileConflict handles Esc in its own handler (Vi mode → Normal mode)
    if key.code == crossterm::event::KeyCode::Esc {
        return DialogAction::Cancel;
    }

    // Delegate to content-specific handler
    handle_content_input(&mut dialog.content, key)
}

/// Handle content-specific input
fn handle_content_input(content: &mut DialogContent, key: KeyEvent) -> DialogAction {
    match content {
        DialogContent::JobManager { selected_index, focused_field } => {
            // Job Manager dialog input handling (Part 6.6, 6.7)

            // Up/Down navigation in Job List (only when Job List is focused)
            if *focused_field == 0 {
                match key.code {
                    crossterm::event::KeyCode::Up => {
                        if *selected_index > 0 {
                            *selected_index -= 1;
                        }
                        return DialogAction::None;
                    }
                    crossterm::event::KeyCode::Down => {
                        *selected_index += 1;
                        return DialogAction::None;
                    }
                    // C key: Cancel selected job directly (Part 6.6)
                    crossterm::event::KeyCode::Char('c') | crossterm::event::KeyCode::Char('C') => {
                        // Return Confirm to trigger cancellation (focused_field will be checked by caller)
                        // We temporarily set focused_field to 2 (Cancel Job button) to trigger cancellation
                        *focused_field = 2;
                        return DialogAction::Confirm;
                    }
                    _ => {}
                }
            }
            return DialogAction::None;
        }
        DialogContent::Compression {
            focused_field,
            format_focus_index,
            compression_focus_index,
            selected_format_index,
            selected_compression_index,
            archive_name,
            cursor_pos,
            edit_mode,
            vi_mode,
            ..
        } => {
            // Handle Esc for Vi mode first
            if key.code == crossterm::event::KeyCode::Esc {
                if *edit_mode == rwf_lib::config::EditMode::Vi {
                    *vi_mode = Some(rwf_lib::config::ViMode::Normal);
                    return DialogAction::None;
                } else {
                    return DialogAction::Cancel;
                }
            }
            
            match *focused_field {
                0 => {
                    // Format list has focus - Up/Down moves focus, Space sets selection
                    match key.code {
                        crossterm::event::KeyCode::Up => {
                            if *format_focus_index > 0 {
                                *format_focus_index -= 1;
                            }
                        }
                        crossterm::event::KeyCode::Down => {
                            if *format_focus_index < 7 {
                                *format_focus_index += 1;
                            }
                        }
                        crossterm::event::KeyCode::Char(' ') => {
                            // Set selection to current focus position
                            *selected_format_index = *format_focus_index;
                        }
                        _ => {}
                    }
                }
                1 => {
                    // Compression list has focus - Up/Down moves focus, Space sets selection
                    match key.code {
                        crossterm::event::KeyCode::Up => {
                            if *compression_focus_index > 0 {
                                *compression_focus_index -= 1;
                            }
                        }
                        crossterm::event::KeyCode::Down => {
                            if *compression_focus_index < 5 {
                                *compression_focus_index += 1;
                            }
                        }
                        crossterm::event::KeyCode::Char(' ') => {
                            // Set selection to current focus position
                            *selected_compression_index = *compression_focus_index;
                        }
                        _ => {}
                    }
                }
                2 => {
                    // Name input has focus - character input and cursor movement
                    match key.code {
                        crossterm::event::KeyCode::Char(c) => {
                            archive_name.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                        }
                        crossterm::event::KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                                archive_name.remove(*cursor_pos);
                            }
                        }
                        crossterm::event::KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        crossterm::event::KeyCode::Right => {
                            if *cursor_pos < archive_name.len() {
                                *cursor_pos += 1;
                            }
                        }
                        crossterm::event::KeyCode::Home => {
                            *cursor_pos = 0;
                        }
                        crossterm::event::KeyCode::End => {
                            *cursor_pos = archive_name.len();
                        }
                        _ => {}
                    }
                }
                _ => {} // Buttons don't handle input here
            }
            DialogAction::None
        }
        DialogContent::ExtractionConfirm { .. } => {
            // Simple confirmation - only global shortcuts apply
            DialogAction::None
        }
        _ => DialogAction::None,
    }
}

/// Process dialog confirmation and create transitions
/// Returns the job spec if a job was created, so it can be submitted to the worker pool
pub fn process_dialog_confirmation(state: &mut rwf_lib::AppState) -> Option<rwf_lib::job::JobSpec> {
    debug!("process_dialog_confirmation called");
    if let Some(dialog) = state.dialogs.current() {
        debug!("Dialog content type: {:?}", std::mem::discriminant(&dialog.content));
        match &dialog.content {
            DialogContent::Compression { 
                sources, 
                archive_name, 
                selected_format_index,
                compression_level,
                ..
            } => {
                debug!("Compression dialog confirmed: {} sources, archive_name='{}'", sources.len(), archive_name);
                debug!("Selected format index: {}, compression level: {}", selected_format_index, compression_level);

                // Ensure archive name has .zip extension
                let archive_name_with_ext = if archive_name.to_lowercase().ends_with(".zip") {
                    archive_name.clone()
                } else {
                    format!("{}.zip", archive_name)
                };
                debug!("Archive name with extension: '{}'", archive_name_with_ext);

                // Build destination path in opposite pane
                let dest_path = state.opposite_pane().current_location.path()
                    .unwrap_or_else(|| std::path::Path::new(".")).to_path_buf();
                let dest = rwf_lib::Location::Local(dest_path.join(&archive_name_with_ext));
                debug!("Destination path: {:?}", dest_path.join(&archive_name_with_ext));

                // Calculate original size for compression ratio
                let original_size: u64 = sources.iter()
                    .filter_map(|loc| {
                        state.active_pane()
                            .entries
                            .iter()
                            .find(|e| &e.location == loc)
                    })
                    .filter(|e| !e.is_dir)
                    .map(|e| e.size)
                    .sum();
                debug!("Original size: {} bytes", original_size);

                let job_spec = rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::CreateArchive {
                        sources: sources.clone(),
                        dest,
                        original_size,
                    }
                );
                debug!("Job spec created: {:?}", job_spec.kind);

                return Some(job_spec);
            }
            DialogContent::ExtractionConfirm { archive, dest, .. } => {
                // Create extraction job - dest is already a Location
                let job_spec = rwf_lib::job::JobSpec::new(
                    rwf_lib::job::JobKind::ExtractArchive {
                        archive: archive.clone(),
                        dest: dest.clone(),
                    }
                );

                return Some(job_spec);
            }
            _ => {
                debug!("Unknown dialog content type");
            }
        }
    } else {
        debug!("No dialog found");
    }
    
    None
}
