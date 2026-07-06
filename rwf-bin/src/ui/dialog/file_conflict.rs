//! File conflict dialog: rendering, input handling, and tests.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use crossterm::event::{KeyEvent, KeyModifiers};
use tracing::debug;

use crate::ui::{smart_truncate, SmartText, TruncateMode};

use super::common::{DIALOG_ACCENT_GREEN, DIALOG_ACCENT_YELLOW};
use super::DialogAction;

/// Handle key input for the File Conflict dialog. Thin wrapper over
/// `handle_file_conflict_input`, which threads its 13 mutable fields as separate
/// `&mut` params (kept that way so `test_support::ConflictInputHarness` can
/// override individual fields without constructing a full `FileConflictDialog`).
pub(super) fn handle_input(
    dialog: &mut rwf_lib::model::dialog::FileConflictDialog,
    key: KeyEvent,
) -> DialogAction {
    let rwf_lib::model::dialog::FileConflictDialog {
        conflicts,
        current_index,
        focused_button,
        rename_text,
        rename_cursor,
        rename_scroll,
        edit_mode,
        vi_mode,
        error_message,
        decisions,
        vi_pending_find_backward,
        vi_pending_operator,
        vi_pending_ctrl_x,
        history,
        history_index,
        ..
    } = dialog;
    handle_file_conflict_input(
        conflicts,
        current_index,
        focused_button,
        rename_text,
        rename_cursor,
        rename_scroll,
        edit_mode,
        vi_mode,
        error_message,
        decisions,
        vi_pending_find_backward,
        vi_pending_operator,
        vi_pending_ctrl_x,
        history,
        history_index,
        key,
    )
}

/// Render File Conflict dialog (compact layout with vertical buttons)
#[allow(clippy::too_many_arguments)]
pub(super) fn render_file_conflict_dialog(
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
    let content_width = area.width.saturating_sub(4) as usize; // 2 chars margin on each side
    let textbox_width = area.width.saturating_sub(20) as usize; // Leave space for button

    // Line 0: Filename
    let filename_line = format!(
        "Filename: {}",
        smart_truncate(
            &current.source.name,
            content_width.saturating_sub(10),
            "..."
        )
    );
    let filename_para = Paragraph::new(filename_line).style(crate::ui::dialog::common::DIALOG_TEXT);
    frame.render_widget(
        filename_para,
        Rect::new(area.x + 2, area.y, content_width as u16, 1),
    );

    // --- FROM SECTION (Lines 1-4) ---
    // Line 1: "From:" label
    let from_label = Paragraph::new("From:").style(crate::ui::dialog::common::DIALOG_TEXT);
    frame.render_widget(
        from_label,
        Rect::new(area.x + 2, area.y + 1, content_width as u16, 1),
    );

    // Line 2-3: From path (2 lines) using SmartText
    let from_full_path = current.source_path.display_path();
    let from_path_widget = SmartText::new(&from_full_path)
        .style(crate::ui::dialog::common::DIALOG_TEXT)
        .max_lines(2)
        .mode(TruncateMode::Path);
    from_path_widget.render(
        frame,
        Rect::new(
            area.x + 4,
            area.y + 2,
            content_width.saturating_sub(2) as u16,
            2,
        ),
    );

    // Line 4: From size,date
    let from_info = format!(
        "  Size,Date: {} Bytes, {}",
        current.source.size,
        chrono::DateTime::<chrono::Local>::from(current.source.modified)
            .format("%Y-%m-%d %H:%M:%S")
    );
    let from_info_para = Paragraph::new(from_info).style(crate::ui::dialog::common::DIALOG_TEXT);
    frame.render_widget(
        from_info_para,
        Rect::new(area.x + 2, area.y + 4, content_width as u16, 1),
    );

    // Line 5: Blank

    // --- TO SECTION (Lines 6-9) ---
    // Line 6: "To:" label
    let to_label = Paragraph::new("To:").style(crate::ui::dialog::common::DIALOG_TEXT);
    frame.render_widget(
        to_label,
        Rect::new(area.x + 2, area.y + 6, content_width as u16, 1),
    );

    // Line 7-8: To path (2 lines) using SmartText
    let to_full_path = current.dest_path.display_path();
    let to_path_widget = SmartText::new(&to_full_path)
        .style(crate::ui::dialog::common::DIALOG_TEXT)
        .max_lines(2)
        .mode(TruncateMode::Path);
    to_path_widget.render(
        frame,
        Rect::new(
            area.x + 4,
            area.y + 7,
            content_width.saturating_sub(2) as u16,
            2,
        ),
    );

    // Line 9: To size,date
    let to_info = format!(
        "  Size,Date: {} Bytes, {}",
        current.dest.size,
        chrono::DateTime::<chrono::Local>::from(current.dest.modified).format("%Y-%m-%d %H:%M:%S")
    );
    let to_info_para = Paragraph::new(to_info).style(crate::ui::dialog::common::DIALOG_TEXT);
    frame.render_widget(
        to_info_para,
        Rect::new(area.x + 2, area.y + 9, content_width as u16, 1),
    );

    // Line 10: Blank

    // Line 11: Status indicator
    let status_line = format!("{} {}", indicator, message);
    let status_style = if indicator == "✓" {
        DIALOG_ACCENT_GREEN
    } else {
        DIALOG_ACCENT_YELLOW
    };
    let status_para = Paragraph::new(status_line).style(status_style);
    frame.render_widget(
        status_para,
        Rect::new(area.x + 2, area.y + 11, content_width as u16, 1),
    );

    // Line 12: Blank

    // Lines 13-16: Buttons (vertical layout)
    // Focus fields: 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename (Textbox), 4=Cancel
    let button_texts = [
        ("Force Overwrite", false),
        ("Overwrite if New", false),
        ("Skip", false),
        ("Rename:", false), // Label for textbox
        ("Cancel", false),
    ];

    let mut button_y = area.y + 13;
    for (i, (label, is_default)) in button_texts.iter().enumerate() {
        let button_text = if *is_default {
            format!("[*{}*]", label)
        } else {
            format!("[{}]", label)
        };

        let button_is_focused = focused_button == i;

        if i == 3 {
            // Rename label + Textbox
            let label_style = if button_is_focused {
                crate::ui::dialog::common::DIALOG_ACCENT_YELLOW.add_modifier(Modifier::BOLD)
            } else {
                crate::ui::dialog::common::DIALOG_TEXT
            };

            // Render "Rename:" label
            let label_para = Paragraph::new(label.to_string()).style(label_style);
            frame.render_widget(
                label_para,
                Rect::new(area.x + 2, button_y, label.len() as u16, 1),
            );

            // Render textbox using TextInput widget (dedicated focus field 3)
            let textbox_x = area.x + 2 + label.len() as u16 + 1;
            let textbox_width_u16 = textbox_width.saturating_sub(label.len()) as u16;

            // Create TextInput widget for rendering
            let mut text_input =
                crate::ui::text_input::TextInput::new(Some(rename_text.to_string()), edit_mode);
            // Restore Vi mode state
            if let Some(vm) = vi_mode {
                text_input.set_vi_mode(vm);
            }
            text_input.set_width(textbox_width_u16);
            text_input.set_cursor(rename_cursor);
            text_input.set_scroll(rename_scroll);
            text_input.render(
                frame,
                Rect::new(textbox_x, button_y, textbox_width_u16, 1),
                button_is_focused,
            );
        } else {
            let button_style = if button_is_focused {
                crate::ui::dialog::common::DIALOG_SELECTED.add_modifier(Modifier::BOLD)
            } else {
                crate::ui::dialog::common::DIALOG_TEXT
            };
            let button_para = Paragraph::new(button_text.clone()).style(button_style);
            frame.render_widget(
                button_para,
                Rect::new(area.x + 2, button_y, button_text.len() as u16, 1),
            );
        }

        button_y += 1;
    }

    // Hint text (shown when Force, Overwrite if New, or Skip focused)
    if focused_button == 0 || focused_button == 1 || focused_button == 2 {
        let hint = Paragraph::new("(Shift+Enter for the rest)")
            .style(crate::ui::dialog::common::DIALOG_DIM);
        frame.render_widget(
            hint,
            Rect::new(area.x + 2, button_y, content_width as u16, 1),
        );
    }

    // Error message line - ALWAYS AT BOTTOM OF DIALOG AREA
    if let Some(error) = error_message {
        let error_para = Paragraph::new(format!("Error: {}", error)).style(
            Style::default()
                .fg(Color::Red)
                .bg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        );
        // Place at the last line of the provided area
        frame.render_widget(
            error_para,
            Rect::new(
                area.x + 2,
                area.y + area.height - 1,
                content_width as u16,
                1,
            ),
        );
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
    let reserved_names = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let name_without_ext = std::path::Path::new(rename_text)
        .file_stem()
        .map(|s| s.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    if reserved_names.contains(&name_without_ext.as_str()) {
        return Some("Reserved system name".to_string());
    }

    None // Valid
}

/// Handle File Conflict dialog input with TextInput widget
/// Focus fields: 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename button, 4=Textbox, 5=Cancel
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_file_conflict_input(
    conflicts: &mut [rwf_lib::model::dialog::ConflictPair],
    current_index: &mut usize,
    focused_button: &mut usize,
    rename_text: &mut String,
    rename_cursor: &mut usize,
    rename_scroll: &mut usize,
    edit_mode: &mut rwf_lib::config::EditMode,
    vi_mode: &mut Option<rwf_lib::config::ViMode>,
    error_message: &mut Option<String>,
    decisions: &mut Vec<rwf_lib::model::dialog::ConflictAction>,
    pending_find_backward: &mut Option<bool>,
    pending_operator: &mut Option<u8>,
    pending_ctrl_x: &mut bool,
    history: &mut Vec<String>,
    history_index: &mut usize,
    key: KeyEvent,
) -> DialogAction {
    use crate::ui::text_input::{TextInput, TextInputAction};
    use crossterm::event::KeyCode;
    use rwf_lib::config::ViMode;

    let is_textbox_focused = *focused_button == 3;

    // Clear error on focus change
    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
        *error_message = None;
    }

    // If textbox is focused, delegate to TextInput
    if is_textbox_focused {
        let mut text_input = TextInput::new(Some(rename_text.clone()), *edit_mode);
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
        text_input.set_history(history.clone());
        text_input.set_history_index(*history_index);
        // set_cursor/set_scroll AFTER set_history_index to prevent cursor reset to end
        text_input.set_cursor(*rename_cursor);
        text_input.set_scroll(*rename_scroll);

        let action = text_input.handle_input(&key);

        // Always sync state from TextInput (DRY - widget owns all state)
        *rename_text = text_input.text().to_string();
        *rename_cursor = text_input.cursor();
        *rename_scroll = text_input.scroll();
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
        *history = text_input.history().clone();
        *history_index = text_input.history_index();

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
                            new_name: rename_text.clone(),
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
                *focused_button = (*focused_button + 1) % 5;
                return DialogAction::None;
            }
            TextInputAction::PrevField => {
                *focused_button = if *focused_button == 0 {
                    4
                } else {
                    *focused_button - 1
                };
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
            *focused_button = (*focused_button + 1) % 5; // 5 fields now
            DialogAction::None
        }

        KeyCode::BackTab => {
            *focused_button = if *focused_button == 0 {
                4
            } else {
                *focused_button - 1
            };
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
                    0 | 1 | 2 | 4 => {
                        // Force, OverwriteIfNew, Skip, Cancel buttons
                        let action = button_index_to_action(*focused_button);
                        decisions.push(action);
                        DialogAction::Confirm
                    }
                    3 => {
                        // Textbox: handled above in the is_textbox_focused block
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
        3 => rwf_lib::model::dialog::ConflictAction::Rename {
            new_name: String::new(),
        }, // Placeholder, name synced from textbox
        4 => rwf_lib::model::dialog::ConflictAction::Skip, // Cancel = skip this file
        _ => rwf_lib::model::dialog::ConflictAction::Skip,
    }
}

#[cfg(test)]
mod conflict_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use rwf_lib::model::dialog::{ConflictAction, ConflictPair};
    use rwf_lib::model::{FileEntry, Location};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn make_conflict(src_name: &str, dst_name: &str) -> ConflictPair {
        let src = FileEntry {
            name: src_name.to_string(),
            location: Location::Local(PathBuf::from(format!("/src/{}", src_name))),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        };
        let dst = FileEntry {
            name: dst_name.to_string(),
            location: Location::Local(PathBuf::from(format!("/dst/{}", dst_name))),
            size: 200,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        };
        ConflictPair {
            source: src.clone(),
            dest: dst.clone(),
            source_path: src.location.clone(),
            dest_path: dst.location.clone(),
            is_directory: false,
        }
    }

    fn enter_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }
    fn shift_enter_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)
    }
    fn esc_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
    }

    // ---- validate_filename tests -------------------------------------------

    #[test]
    fn test_validate_filename_empty() {
        assert!(validate_filename("", "original.txt").is_some());
    }

    #[test]
    fn test_validate_filename_same_as_original() {
        assert!(validate_filename("file.txt", "file.txt").is_some());
    }

    #[test]
    fn test_validate_filename_windows_invalid_chars() {
        for ch in ['<', '>', ':', '"', '|', '?', '*'] {
            let bad = format!("fi{}le.txt", ch);
            assert!(
                validate_filename(&bad, "orig.txt").is_some(),
                "char '{}' should be rejected",
                ch
            );
        }
    }

    #[test]
    fn test_validate_filename_reserved_name() {
        assert!(validate_filename("CON", "orig.txt").is_some());
        assert!(validate_filename("NUL.txt", "orig.txt").is_some());
    }

    #[test]
    fn test_validate_filename_valid() {
        assert!(validate_filename("new_name.txt", "old_name.txt").is_none());
    }

    // ---- handle_file_conflict_input: Force button (index 0) ----------------

    #[test]
    fn test_force_button_enter_pushes_force_decision() {
        // Uses the M2 harness (test_support::ConflictInputHarness); the
        // remaining conflict tests migrate to it in M3.
        let mut harness =
            super::super::test_support::ConflictInputHarness::new(vec![make_conflict(
                "a.txt", "a.txt",
            )]);
        harness.focused_button = 0; // Force

        let action = harness.send(enter_key());

        assert_eq!(action, DialogAction::Confirm);
        assert_eq!(harness.decisions.len(), 1);
        assert!(matches!(harness.decisions[0], ConflictAction::Force));
    }

    // ---- Skip button (index 2) ---------------------------------------------

    #[test]
    fn test_skip_button_enter_pushes_skip_decision() {
        let mut harness =
            super::super::test_support::ConflictInputHarness::new(vec![make_conflict(
                "b.txt", "b.txt",
            )]);
        harness.focused_button = 2; // Skip

        let action = harness.send(enter_key());

        assert_eq!(action, DialogAction::Confirm);
        assert!(matches!(harness.decisions[0], ConflictAction::Skip));
    }

    // ---- Cancel button (index 4) -------------------------------------------

    #[test]
    fn test_cancel_button_enter_returns_confirm_with_skip_decision() {
        let mut harness =
            super::super::test_support::ConflictInputHarness::new(vec![make_conflict(
                "c.txt", "c.txt",
            )]);
        harness.focused_button = 4; // Cancel

        let action = harness.send(enter_key());

        assert_eq!(action, DialogAction::Confirm);
        assert!(matches!(harness.decisions[0], ConflictAction::Skip));
    }

    // ---- Esc cancels dialog ------------------------------------------------

    #[test]
    fn test_esc_cancels_dialog() {
        let mut harness =
            super::super::test_support::ConflictInputHarness::new(vec![make_conflict(
                "d.txt", "d.txt",
            )]);

        let action = harness.send(esc_key());

        assert_eq!(action, DialogAction::Cancel);
        assert!(
            harness.decisions.is_empty(),
            "Cancel should not push a decision"
        );
    }

    // ---- Shift+Enter applies to all remaining ------------------------------

    #[test]
    fn test_shift_enter_applies_to_all_remaining() {
        let mut harness = super::super::test_support::ConflictInputHarness::new(vec![
            make_conflict("e1.txt", "e1.txt"),
            make_conflict("e2.txt", "e2.txt"),
            make_conflict("e3.txt", "e3.txt"),
        ]);
        harness.current_index = 1; // at second conflict
        harness.focused_button = 2; // Skip
        harness.rename_text = "e2.txt".to_string();
        harness.rename_cursor = 6;
        harness.decisions = vec![ConflictAction::Force]; // first conflict already decided
        harness.history = vec!["e2.txt".to_string()];

        let action = harness.send(shift_enter_key());

        assert_eq!(action, DialogAction::ConfirmAll);
        // decisions: 1 (pre-existing) + 2 (remaining from current_index=1 to end)
        assert_eq!(
            harness.decisions.len(),
            3,
            "all 3 decisions must be present"
        );
        assert!(matches!(harness.decisions[1], ConflictAction::Skip));
        assert!(matches!(harness.decisions[2], ConflictAction::Skip));
    }

    // ---- Tab cycle stays within 0..4 ---------------------------------------

    #[test]
    fn test_tab_cycles_0_to_4() {
        let mut harness =
            super::super::test_support::ConflictInputHarness::new(vec![make_conflict(
                "f.txt", "f.txt",
            )]);
        harness.focused_button = 4; // last field

        harness.send(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(
            harness.focused_button, 0,
            "Tab from last field (4) should wrap to 0"
        );
    }
}
