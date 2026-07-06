//! Snapshots for `DialogContent::JumpToFile`.
//!
//! Built directly with fixed candidate lists — never via transitions, which
//! scan the real filesystem.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, DialogContent};

#[test]
fn jump_to_file_empty() {
    let state = test_state();
    let dialog = Dialog::jump_to_file("/test".to_string(), vec![]);
    snapshot_dialog("jump_to_file_empty", &dialog, &state);
}

#[test]
fn jump_to_file_suggestions() {
    let state = test_state();
    let dialog = Dialog::jump_to_file(
        "/test".to_string(),
        vec![
            "/test/readme.md".to_string(),
            "/test/main.rs".to_string(),
            "/test/docs/guide.md".to_string(),
            "/test/docs/spec.md".to_string(),
        ],
    );
    snapshot_dialog("jump_to_file_suggestions", &dialog, &state);
}

#[test]
fn jump_to_file_query_and_selection() {
    let state = test_state();
    let mut dialog = Dialog::jump_to_file(
        "/test".to_string(),
        vec![
            "/test/readme.md".to_string(),
            "/test/main.rs".to_string(),
            "/test/docs/guide.md".to_string(),
        ],
    );
    if let DialogContent::JumpToFile(rwf_lib::model::dialog::JumpToFileDialog {
        query,
        cursor_pos,
        suggestions,
        selected_index,
        ..
    }) = &mut dialog.content
    {
        *query = "md".to_string();
        *cursor_pos = 2;
        *suggestions = vec![
            "/test/readme.md".to_string(),
            "/test/docs/guide.md".to_string(),
        ];
        *selected_index = 0;
    }
    snapshot_dialog("jump_to_file_query_selected", &dialog, &state);
}
