//! Snapshots for `DialogContent::DeleteConfirm`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::Location;
use std::path::PathBuf;

#[test]
fn delete_confirm_single_file() {
    let state = test_state();
    let targets = vec![(Location::Local(PathBuf::from("/test/file.txt")), false)];
    // to_trash=false here preserves the pinned "Delete File"/"Delete N Files"
    // titles these snapshots assert on; real to_trash wiring lands in a
    // later task (Phase 7.7 task 11).
    let dialog = Dialog::delete_confirm(targets, false, false);
    snapshot_dialog("delete_confirm_single_file", &dialog, &state);
}

#[test]
fn delete_confirm_single_directory() {
    let state = test_state();
    let targets = vec![(Location::Local(PathBuf::from("/test/directory")), true)];
    // to_trash=false here preserves the pinned "Delete File"/"Delete N Files"
    // titles these snapshots assert on; real to_trash wiring lands in a
    // later task (Phase 7.7 task 11).
    let dialog = Dialog::delete_confirm(targets, false, false);
    snapshot_dialog("delete_confirm_single_dir", &dialog, &state);
}

#[test]
fn delete_confirm_multiple_targets() {
    let state = test_state();
    let targets = vec![
        (Location::Local(PathBuf::from("/test/file1.txt")), false),
        (Location::Local(PathBuf::from("/test/file2.txt")), false),
        (Location::Local(PathBuf::from("/test/folder")), true),
    ];
    // to_trash=false here preserves the pinned "Delete File"/"Delete N Files"
    // titles these snapshots assert on; real to_trash wiring lands in a
    // later task (Phase 7.7 task 11).
    let dialog = Dialog::delete_confirm(targets, false, false);
    snapshot_dialog("delete_confirm_three_targets", &dialog, &state);
}
