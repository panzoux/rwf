//! Snapshots for `DialogContent::AttrTimestamp`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::Location;
use std::path::PathBuf;

#[cfg(windows)]
#[test]
fn attr_timestamp_default() {
    let state = test_state();
    // Nonexistent synthetic path (project convention): fields stay in their
    // empty/mixed default since `AttrTimestampDialog::new()` can't stat it.
    let dialog = Dialog::attr_timestamp(vec![Location::Local(PathBuf::from("/test/file.txt"))]);
    snapshot_dialog("attr_timestamp_default", &dialog, &state);
}

#[cfg(windows)]
#[test]
fn attr_timestamp_hidden_checked() {
    let state = test_state();
    let mut dialog = Dialog::attr_timestamp(vec![Location::Local(PathBuf::from("/test/file.txt"))]);
    if let rwf_lib::model::dialog::DialogContent::AttrTimestamp(d) = &mut dialog.content {
        d.hidden.current = Some(true);
        d.focused_field = 1; // Hidden checkbox
    }
    snapshot_dialog("attr_timestamp_hidden_checked", &dialog, &state);
}

#[cfg(windows)]
#[test]
fn attr_timestamp_focused_ok() {
    let state = test_state();
    let mut dialog = Dialog::attr_timestamp(vec![Location::Local(PathBuf::from("/test/file.txt"))]);
    if let rwf_lib::model::dialog::DialogContent::AttrTimestamp(d) = &mut dialog.content {
        let ok = d.ok_index();
        d.focused_field = ok;
    }
    snapshot_dialog("attr_timestamp_focused_ok", &dialog, &state);
}

#[cfg(windows)]
#[test]
fn attr_timestamp_modified_focused_mid_edit() {
    let state = test_state();
    let mut dialog = Dialog::attr_timestamp(vec![Location::Local(PathBuf::from("/test/file.txt"))]);
    if let rwf_lib::model::dialog::DialogContent::AttrTimestamp(d) = &mut dialog.content {
        d.focused_field = 4; // Modified field
        d.modified.set_now();
        d.modified.text = "2026-07-30 12:34:56".to_string();
        d.modified.cursor_pos = 5; // month tens digit, mid-edit
    }
    snapshot_dialog("attr_timestamp_modified_focused_mid_edit", &dialog, &state);
}

#[cfg(windows)]
#[test]
fn attr_timestamp_multi_target_mixed() {
    let state = test_state();
    let mut dialog = Dialog::attr_timestamp(vec![
        Location::Local(PathBuf::from("/test/a.txt")),
        Location::Local(PathBuf::from("/test/b.txt")),
    ]);
    if let rwf_lib::model::dialog::DialogContent::AttrTimestamp(d) = &mut dialog.content {
        // Simulate a real mixed state across targets (both fields default to
        // Mixed already since the synthetic paths don't exist, but set it
        // explicitly for clarity/robustness against future default changes).
        d.hidden.current = None;
    }
    snapshot_dialog("attr_timestamp_multi_target_mixed", &dialog, &state);
}
