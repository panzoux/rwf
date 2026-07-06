//! Snapshots for `DialogContent::JumpToPath`.
//!
//! Built directly with fixed candidate lists — never via transitions, which
//! scan the real filesystem.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, DialogContent};

#[test]
fn jump_to_path_empty() {
    let state = test_state();
    let dialog = Dialog::jump_to_path("/test".to_string(), vec![]);
    snapshot_dialog("jump_to_path_empty", &dialog, &state);
}

#[test]
fn jump_to_path_suggestions() {
    let state = test_state();
    let dialog = Dialog::jump_to_path(
        "/test".to_string(),
        vec![
            "/test/alpha".to_string(),
            "/test/beta".to_string(),
            "/test/gamma/delta".to_string(),
        ],
    );
    snapshot_dialog("jump_to_path_suggestions", &dialog, &state);
}

#[test]
fn jump_to_path_query_and_selection() {
    let state = test_state();
    let mut dialog = Dialog::jump_to_path(
        "/test".to_string(),
        vec![
            "/test/alpha".to_string(),
            "/test/albatross".to_string(),
            "/test/beta".to_string(),
        ],
    );
    if let DialogContent::JumpToPath(rwf_lib::model::dialog::JumpToPathDialog {
        query,
        cursor_pos,
        suggestions,
        selected_index,
        ..
    }) = &mut dialog.content
    {
        *query = "al".to_string();
        *cursor_pos = 2;
        *suggestions = vec!["/test/alpha".to_string(), "/test/albatross".to_string()];
        *selected_index = 1;
    }
    snapshot_dialog("jump_to_path_query_selected", &dialog, &state);
}
