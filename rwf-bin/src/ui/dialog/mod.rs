//! Dialog system with centralized input handling
//!
//! This module provides a hybrid dialog system with:
//! - Common infrastructure (border, title, buttons, centering)
//! - Content-specific rendering via trait
//! - Centralized input handling with consistent shortcuts

mod compression;
mod extract_confirm;
mod frame;

pub use compression::{render_compression_dialog, CompressionDialogState};
pub use extract_confirm::ExtractionConfirmDialog;
pub use frame::{centered_rect, render_dialog_buttons, render_dialog_frame};

use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use rwf_lib::model::dialog::{Dialog, DialogContent};
use tracing::debug;

/// Result of dialog input handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DialogAction {
    None,           // Input consumed, no action
    Confirm,        // User pressed Enter/OK
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
pub fn render_dialog(frame: &mut Frame, dialog: &Dialog, _state: &rwf_lib::AppState) {
    // Calculate minimum dialog height based on content type BEFORE rendering
    let min_content_height = match &dialog.content {
        DialogContent::Compression { .. } => {
            // Calculate from actual layout constraints
            crate::ui::dialog::compression::calculate_compression_dialog_min_height()
        }
        DialogContent::ExtractionConfirm { .. } => {
            // Extraction dialog: ~6 lines content
            6u16
        }
        _ => 8u16, // Default
    };
    
    // Add 2 for borders (top + bottom)
    let min_dialog_height = min_content_height + 2;
    
    let screen_height = frame.area().height;
    
    // For compression dialog, use exact minimum height (no extra space)
    // For other dialogs, use 70% of screen or minimum, whichever is larger
    let dialog_height = match &dialog.content {
        DialogContent::Compression { .. } => {
            // Use exact minimum height, but ensure it fits on screen
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
    
    let area = centered_rect(60, height_percent, frame.area());

    // Render common frame (border, title)
    let content_area = render_dialog_frame(frame, &dialog.title, area);

    // For compression dialog, render content directly with its own layout (includes buttons)
    // For other dialogs, use generic layout
    match &dialog.content {
        DialogContent::Compression { .. } => {
            // Render compression dialog using exact content area (buttons rendered within)
            render_dialog_content(frame, &dialog.content, content_area, true);
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

/// Handle dialog input centrally
pub fn handle_dialog_input(dialog: &mut Dialog, key: KeyEvent) -> DialogAction {
    // Global shortcuts (Escape = Cancel)
    if key.code == crossterm::event::KeyCode::Esc {
        return DialogAction::Cancel;
    }

    // Enter = Confirm
    if key.code == crossterm::event::KeyCode::Enter {
        return DialogAction::Confirm;
    }

    // Tab navigation - cycles through dialog fields
    if key.code == crossterm::event::KeyCode::Tab {
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

    // Delegate to content-specific handler
    handle_content_input(&mut dialog.content, key)
}

/// Handle content-specific input
fn handle_content_input(content: &mut DialogContent, key: KeyEvent) -> DialogAction {
    match content {
        DialogContent::Compression { 
            focused_field,
            format_focus_index,
            compression_focus_index,
            selected_format_index,
            selected_compression_index,
            archive_name,
            cursor_pos,
            ..
        } => {
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
