//! Snapshots for `DialogContent::Input`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;

#[test]
fn input_empty_default() {
    let state = test_state();
    let dialog = Dialog::input("Enter Name", "Name:", "");
    snapshot_dialog("input_empty_default", &dialog, &state);
}

#[test]
fn input_with_default_value() {
    let state = test_state();
    let dialog = Dialog::input("Rename File", "New name:", "original_file.txt");
    snapshot_dialog("input_with_default_value", &dialog, &state);
}

#[test]
fn input_cursor_at_end() {
    let state = test_state();
    let dialog = Dialog::input("Change Directory", "Path:", "/home/user/documents");
    snapshot_dialog("input_cursor_at_end", &dialog, &state);
}

#[test]
fn input_with_scrolling() {
    let state = test_state();
    let dialog = Dialog::input(
        "Edit Description",
        "Description:",
        "This is a very long input string that will need to scroll horizontally when displayed",
    );
    snapshot_dialog("input_with_scrolling", &dialog, &state);
}
