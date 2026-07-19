//! Snapshots for `DialogContent::TypeMismatchWarning`.

use super::{snapshot_dialog, test_state};
use rwf_lib::magic::DetectedKind;
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::Location;
use std::path::PathBuf;

#[test]
fn type_mismatch_warning_pe_executable() {
    let state = test_state();
    let dialog = Dialog::type_mismatch_warning(
        PathBuf::from("/test/notes.txt"),
        DetectedKind::Pe,
        "notepad $F".to_string(),
        Location::Local(PathBuf::from("/test")),
        None,
    );
    snapshot_dialog("type_mismatch_warning_pe", &dialog, &state);
}

#[test]
fn type_mismatch_warning_elf_executable() {
    let state = test_state();
    let dialog = Dialog::type_mismatch_warning(
        PathBuf::from("/test/document.docx"),
        DetectedKind::Elf,
        "less $F".to_string(),
        Location::Local(PathBuf::from("/test")),
        Some("bash".to_string()),
    );
    snapshot_dialog("type_mismatch_warning_elf", &dialog, &state);
}
