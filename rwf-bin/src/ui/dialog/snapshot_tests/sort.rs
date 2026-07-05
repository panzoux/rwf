//! Snapshots for `DialogContent::SortDialog`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::{SortMode, SortOrder};

#[test]
fn sort_default_name_ascending() {
    let state = test_state();
    let dialog = Dialog::sort_dialog(SortMode::Name, SortOrder::Ascending);
    snapshot_dialog("sort_name_ascending", &dialog, &state);
}

#[test]
fn sort_size_descending() {
    let state = test_state();
    let dialog = Dialog::sort_dialog(SortMode::Size, SortOrder::Descending);
    snapshot_dialog("sort_size_descending", &dialog, &state);
}
