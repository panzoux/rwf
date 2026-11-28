//! Snapshots for `DialogContent::WildcardMark`.

use super::{snapshot_dialog, test_state};
use rwf_lib::dialog::Dialog;

#[test]
fn wildcard_mark_empty_input() {
    let state = test_state();
    let dialog = Dialog::wildcard_mark();
    snapshot_dialog("wildcard_mark_empty_input", &dialog, &state);
}

#[test]
fn wildcard_mark_with_pattern() {
    let state = test_state();
    let mut dialog = Dialog::wildcard_mark();
    if let rwf_lib::dialog::DialogContent::WildcardMark(rwf_lib::dialog::WildcardMarkDialog {
        input,
        ui: rwf_lib::dialog::DialogUiState { cursor_pos, .. },
    }) = &mut dialog.content
    {
        *input = "*.log".to_string();
        *cursor_pos = 5;
    }
    snapshot_dialog("wildcard_mark_with_pattern", &dialog, &state);
}

#[test]
fn wildcard_mark_focused_on_ok() {
    let state = test_state();
    let mut dialog = Dialog::wildcard_mark();
    if let rwf_lib::dialog::DialogContent::WildcardMark(rwf_lib::dialog::WildcardMarkDialog {
        input,
        ui:
            rwf_lib::dialog::DialogUiState {
                cursor_pos,
                focused_field,
                ..
            },
    }) = &mut dialog.content
    {
        *input = "test*".to_string();
        *cursor_pos = 5;
        *focused_field = 1; // Focus on OK button
    }
    snapshot_dialog("wildcard_mark_focused_ok", &dialog, &state);
}

#[test]
fn wildcard_mark_focused_on_cancel() {
    let state = test_state();
    let mut dialog = Dialog::wildcard_mark();
    if let rwf_lib::dialog::DialogContent::WildcardMark(rwf_lib::dialog::WildcardMarkDialog {
        input,
        ui:
            rwf_lib::dialog::DialogUiState {
                cursor_pos,
                focused_field,
                ..
            },
    }) = &mut dialog.content
    {
        *input = "[a-z]*".to_string();
        *cursor_pos = 6;
        *focused_field = 2; // Focus on Cancel button
    }
    snapshot_dialog("wildcard_mark_focused_cancel", &dialog, &state);
}
