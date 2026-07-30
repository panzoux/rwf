//! Snapshots for `DialogContent::CreateLink`.
//!
//! Uses synthetic nonexistent paths (project convention — see other
//! snapshot modules): `CreateLinkDialog::new()` can't stat them, so
//! Hardlink/Junction always render as unavailable here. That's an accepted
//! simplification for snapshot determinism (a real TempDir path would
//! differ per test run and break the snapshot); the availability logic
//! itself is covered by real-file unit tests in
//! `rwf-lib/src/model/dialog/create_link.rs`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::Location;
use std::path::PathBuf;

#[test]
fn create_link_default() {
    let state = test_state();
    let dialog = Dialog::create_link(
        Location::Local(PathBuf::from("/test/report.docx")),
        PathBuf::from("/test/dest"),
    );
    snapshot_dialog("create_link_default", &dialog, &state);
}

#[test]
fn create_link_focused_on_name() {
    let state = test_state();
    let mut dialog = Dialog::create_link(
        Location::Local(PathBuf::from("/test/report.docx")),
        PathBuf::from("/test/dest"),
    );
    if let rwf_lib::model::dialog::DialogContent::CreateLink(d) = &mut dialog.content {
        d.focused_field = 1;
        d.link_name = "renamed_link.docx".to_string();
        d.link_name_cursor_pos = d.link_name.chars().count();
    }
    snapshot_dialog("create_link_focused_on_name", &dialog, &state);
}

#[test]
fn create_link_focused_on_ok() {
    let state = test_state();
    let mut dialog = Dialog::create_link(
        Location::Local(PathBuf::from("/test/report.docx")),
        PathBuf::from("/test/dest"),
    );
    if let rwf_lib::model::dialog::DialogContent::CreateLink(d) = &mut dialog.content {
        let ok = d.ok_index();
        d.focused_field = ok;
    }
    snapshot_dialog("create_link_focused_on_ok", &dialog, &state);
}
