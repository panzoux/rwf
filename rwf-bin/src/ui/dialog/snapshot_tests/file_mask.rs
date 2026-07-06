//! Snapshots for `DialogContent::FileMask`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;

#[test]
fn file_mask_empty_input() {
    let state = test_state();
    let dialog = Dialog::file_mask(None);
    snapshot_dialog("file_mask_empty_input", &dialog, &state);
}

#[test]
fn file_mask_with_pattern() {
    let state = test_state();
    let dialog = Dialog::file_mask(Some("*.txt"));
    snapshot_dialog("file_mask_with_pattern", &dialog, &state);
}

#[test]
fn file_mask_focused_on_ok() {
    let state = test_state();
    let mut dialog = Dialog::file_mask(Some("test*"));
    if let rwf_lib::model::dialog::DialogContent::FileMask(
        rwf_lib::model::dialog::FileMaskDialog {
            ui: rwf_lib::model::dialog::DialogUiState { focused_field, .. },
            ..
        },
    ) = &mut dialog.content
    {
        *focused_field = 1; // Focus on OK button
    }
    snapshot_dialog("file_mask_focused_ok", &dialog, &state);
}

#[test]
fn file_mask_focused_on_cancel() {
    let state = test_state();
    let mut dialog = Dialog::file_mask(Some("*.rs"));
    if let rwf_lib::model::dialog::DialogContent::FileMask(
        rwf_lib::model::dialog::FileMaskDialog {
            ui: rwf_lib::model::dialog::DialogUiState { focused_field, .. },
            ..
        },
    ) = &mut dialog.content
    {
        *focused_field = 2; // Focus on Cancel button
    }
    snapshot_dialog("file_mask_focused_cancel", &dialog, &state);
}
