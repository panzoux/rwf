//! Snapshots for `DialogContent::PatternRename`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, DialogContent};

#[test]
fn pattern_rename_empty() {
    let state = test_state();
    let dialog = Dialog::pattern_rename();
    snapshot_dialog("pattern_rename_empty", &dialog, &state);
}

#[test]
fn pattern_rename_with_preview() {
    let state = test_state();
    let mut dialog = Dialog::pattern_rename();
    if let DialogContent::PatternRename {
        find,
        find_cursor_pos,
        replace,
        replace_cursor_pos,
        use_regex,
        preview,
        ..
    } = &mut dialog.content
    {
        *find = "file".to_string();
        *find_cursor_pos = 4;
        *replace = "doc".to_string();
        *replace_cursor_pos = 3;
        *use_regex = false;
        *preview = vec![
            ("file1.txt".to_string(), "doc1.txt".to_string()),
            ("file2.txt".to_string(), "doc2.txt".to_string()),
            ("other.txt".to_string(), "other.txt".to_string()),
        ];
    }
    snapshot_dialog("pattern_rename_with_preview", &dialog, &state);
}

#[test]
fn pattern_rename_error_and_focus() {
    let state = test_state();
    let mut dialog = Dialog::pattern_rename();
    if let DialogContent::PatternRename {
        find,
        find_cursor_pos,
        replace,
        focused_field,
        error_message,
        preview,
        ..
    } = &mut dialog.content
    {
        *find = "a.txt".to_string();
        *find_cursor_pos = 5;
        *replace = "b.txt".to_string();
        *focused_field = 1;
        *error_message = Some("Name collision: b.txt".to_string());
        *preview = vec![("a.txt".to_string(), "b.txt".to_string())];
    }
    snapshot_dialog("pattern_rename_error", &dialog, &state);
}
