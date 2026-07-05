//! Snapshots for `DialogContent::DeleteConfirm`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::Location;
use std::path::PathBuf;

#[test]
fn delete_confirm_single_file() {
    let state = test_state();
    let targets = vec![(Location::Local(PathBuf::from("/test/file.txt")), false)];
    let dialog = Dialog::delete_confirm(targets);
    snapshot_dialog("delete_confirm_single_file", &dialog, &state);
}

#[test]
fn delete_confirm_single_directory() {
    let state = test_state();
    let targets = vec![(Location::Local(PathBuf::from("/test/directory")), true)];
    let dialog = Dialog::delete_confirm(targets);
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
    let dialog = Dialog::delete_confirm(targets);
    snapshot_dialog("delete_confirm_three_targets", &dialog, &state);
}
