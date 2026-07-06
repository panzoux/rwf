//! Snapshots for `DialogContent::Compression`.

use super::{snapshot_dialog, test_state};
use rwf_lib::config::EditMode;
use rwf_lib::model::dialog::{CompressionDialog, Dialog, DialogContent};
use rwf_lib::model::Location;
use std::path::PathBuf;

fn sources() -> Vec<Location> {
    vec![
        Location::Local(PathBuf::from("/test/docs")),
        Location::Local(PathBuf::from("/test/readme.md")),
    ]
}

#[test]
fn compression_default() {
    let state = test_state();
    let dialog = Dialog::compression(sources(), EditMode::Emacs);
    snapshot_dialog("compression_default", &dialog, &state);
}

#[test]
fn compression_name_focused_with_text() {
    let state = test_state();
    let mut dialog = Dialog::compression(
        vec![Location::Local(PathBuf::from("/test/single_dir"))],
        EditMode::Emacs,
    );
    if let DialogContent::Compression(CompressionDialog {
        archive_name,
        cursor_pos,
        focused_field,
        ..
    }) = &mut dialog.content
    {
        *archive_name = "backup".to_string();
        *cursor_pos = 6;
        *focused_field = 2; // name textbox
    }
    snapshot_dialog("compression_name_focused", &dialog, &state);
}

#[test]
fn compression_ok_focused() {
    let state = test_state();
    let mut dialog = Dialog::compression(sources(), EditMode::Emacs);
    if let DialogContent::Compression(CompressionDialog { focused_field, .. }) = &mut dialog.content
    {
        *focused_field = 3; // OK button
    }
    snapshot_dialog("compression_ok_focused", &dialog, &state);
}
