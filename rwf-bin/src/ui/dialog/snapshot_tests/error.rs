//! Snapshots for `DialogContent::Error`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;

#[test]
fn error_simple_message() {
    let state = test_state();
    let dialog = Dialog::error("An error occurred");
    snapshot_dialog("error_simple_message", &dialog, &state);
}

#[test]
fn error_permission_denied() {
    let state = test_state();
    let dialog = Dialog::permission_error("Access to file denied");
    snapshot_dialog("error_permission_denied", &dialog, &state);
}

#[test]
fn error_with_details() {
    let state = test_state();
    let dialog = Dialog::error_with_details(
        "Operation failed",
        "The file could not be copied because the destination path is invalid.",
    );
    snapshot_dialog("error_with_details", &dialog, &state);
}

#[test]
fn error_file_not_found() {
    let state = test_state();
    let dialog = Dialog::file_not_found_error("/home/user/nonexistent_file.txt");
    snapshot_dialog("error_file_not_found", &dialog, &state);
}

#[test]
fn error_long_message() {
    let state = test_state();
    let dialog = Dialog::error_with_details(
        "Copy operation failed",
        "This is a detailed error message that spans multiple lines.\nThe operation encountered an unexpected condition.\nPlease check your configuration and try again.",
    );
    snapshot_dialog("error_long_message", &dialog, &state);
}
