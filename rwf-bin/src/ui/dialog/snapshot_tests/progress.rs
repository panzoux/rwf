//! Snapshots for `DialogContent::Progress`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;

#[test]
fn progress_starting() {
    let state = test_state();
    let dialog = Dialog::progress("Copy Files", "Copying files...", 0.0, "Starting operation");
    snapshot_dialog("progress_starting", &dialog, &state);
}

#[test]
fn progress_midway() {
    let state = test_state();
    let dialog = Dialog::progress(
        "Copy Files",
        "Copying files...",
        0.45,
        "45 of 100 files copied (12.3 MB)",
    );
    snapshot_dialog("progress_midway", &dialog, &state);
}

#[test]
fn progress_nearly_complete() {
    let state = test_state();
    let dialog = Dialog::progress(
        "Copy Files",
        "Copying files...",
        0.98,
        "98 of 100 files copied (486.5 MB)",
    );
    snapshot_dialog("progress_nearly_complete", &dialog, &state);
}

#[test]
fn progress_delete_operation() {
    let state = test_state();
    let dialog = Dialog::progress(
        "Delete Files",
        "Removing files...",
        0.60,
        "Deleted 60 items",
    );
    snapshot_dialog("progress_delete_operation", &dialog, &state);
}
