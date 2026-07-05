//! Snapshots for `DialogContent::Confirmation`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;

#[test]
fn confirmation_short_message() {
    let state = test_state();
    let dialog = Dialog::confirmation("Confirm Action", "Are you sure?");
    snapshot_dialog("confirmation_short_message", &dialog, &state);
}

#[test]
fn confirmation_long_message() {
    let state = test_state();
    let dialog = Dialog::confirmation(
        "Delete File",
        "This file will be permanently deleted and cannot be recovered. Are you absolutely sure?",
    );
    snapshot_dialog("confirmation_long_message", &dialog, &state);
}

#[test]
fn confirmation_multiline_message() {
    let state = test_state();
    let dialog = Dialog::confirmation(
        "Replace Files",
        "Some files already exist in the destination.\nDo you want to overwrite them?",
    );
    snapshot_dialog("confirmation_multiline_message", &dialog, &state);
}
