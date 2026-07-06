//! Snapshots for `DialogContent::SimpleRename`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;

#[test]
fn simple_rename_with_filename() {
    let state = test_state();
    let dialog = Dialog::simple_rename("oldname.txt".to_string());
    snapshot_dialog("simple_rename_with_filename", &dialog, &state);
}

#[test]
fn simple_rename_partial_text() {
    let state = test_state();
    let mut dialog = Dialog::simple_rename("document.pdf".to_string());
    if let rwf_lib::model::dialog::DialogContent::SimpleRename(
        rwf_lib::model::dialog::SimpleRenameDialog {
            input,
            ui: rwf_lib::model::dialog::DialogUiState { cursor_pos, .. },
        },
    ) = &mut dialog.content
    {
        *input = "new_name.pdf".to_string();
        *cursor_pos = 8; // Cursor in the middle
    }
    snapshot_dialog("simple_rename_partial_text", &dialog, &state);
}

#[test]
fn simple_rename_focused_on_ok() {
    let state = test_state();
    let mut dialog = Dialog::simple_rename("file.rs".to_string());
    if let rwf_lib::model::dialog::DialogContent::SimpleRename(
        rwf_lib::model::dialog::SimpleRenameDialog {
            ui: rwf_lib::model::dialog::DialogUiState { focused_field, .. },
            ..
        },
    ) = &mut dialog.content
    {
        *focused_field = 1; // Focus on OK button
    }
    snapshot_dialog("simple_rename_focused_ok", &dialog, &state);
}

#[test]
fn simple_rename_focused_on_cancel() {
    let state = test_state();
    let mut dialog = Dialog::simple_rename("archive.zip".to_string());
    if let rwf_lib::model::dialog::DialogContent::SimpleRename(
        rwf_lib::model::dialog::SimpleRenameDialog {
            ui: rwf_lib::model::dialog::DialogUiState { focused_field, .. },
            ..
        },
    ) = &mut dialog.content
    {
        *focused_field = 2; // Focus on Cancel button
    }
    snapshot_dialog("simple_rename_focused_cancel", &dialog, &state);
}
