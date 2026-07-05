//! Snapshots for `DialogContent::HistoryDialog`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::ui::ActivePane;
use rwf_lib::model::Location;
use std::path::PathBuf;

#[test]
fn history_dialog_left_pane_active() {
    let state = test_state();
    let left_entries = vec![
        Location::Local(PathBuf::from("/test/a")),
        Location::Local(PathBuf::from("/test/b")),
        Location::Local(PathBuf::from("/test/c")),
    ];
    let right_entries = vec![
        Location::Local(PathBuf::from("/home/user")),
        Location::Local(PathBuf::from("/home/documents")),
    ];
    let dialog = Dialog::history_dialog(0, ActivePane::Left, left_entries, 1, right_entries, 0);
    snapshot_dialog("history_dialog_left_pane_active", &dialog, &state);
}

#[test]
fn history_dialog_right_pane_active() {
    let state = test_state();
    let left_entries = vec![
        Location::Local(PathBuf::from("/test/a")),
        Location::Local(PathBuf::from("/test/b")),
    ];
    let right_entries = vec![
        Location::Local(PathBuf::from("/home/user")),
        Location::Local(PathBuf::from("/home/documents")),
        Location::Local(PathBuf::from("/home/downloads")),
    ];
    let dialog = Dialog::history_dialog(1, ActivePane::Right, left_entries, 0, right_entries, 2);
    snapshot_dialog("history_dialog_right_pane_active", &dialog, &state);
}

#[test]
fn history_dialog_multiple_entries() {
    let state = test_state();
    let left_entries = vec![
        Location::Local(PathBuf::from("/test/first")),
        Location::Local(PathBuf::from("/test/second")),
        Location::Local(PathBuf::from("/test/third")),
        Location::Local(PathBuf::from("/test/fourth")),
    ];
    let right_entries = vec![
        Location::Local(PathBuf::from("/home/a")),
        Location::Local(PathBuf::from("/home/b")),
        Location::Local(PathBuf::from("/home/c")),
        Location::Local(PathBuf::from("/home/d")),
        Location::Local(PathBuf::from("/home/e")),
    ];
    let dialog = Dialog::history_dialog(2, ActivePane::Left, left_entries, 2, right_entries, 3);
    snapshot_dialog("history_dialog_multiple_entries", &dialog, &state);
}
