//! Snapshots for `DialogContent::ExtractionConfirm`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::Location;
use std::path::PathBuf;

#[test]
fn extraction_confirm_single_file() {
    let state = test_state();
    let dialog = Dialog::extraction_confirm(
        Location::Local(PathBuf::from("/test/archive.zip")),
        Location::Local(PathBuf::from("/test/out")),
        1,
    );
    snapshot_dialog("extraction_confirm_single", &dialog, &state);
}

#[test]
fn extraction_confirm_many_files() {
    let state = test_state();
    let dialog = Dialog::extraction_confirm(
        Location::Local(PathBuf::from("/test/backup.tar.gz")),
        Location::Local(PathBuf::from("/test/restore/target")),
        1234,
    );
    snapshot_dialog("extraction_confirm_many", &dialog, &state);
}
