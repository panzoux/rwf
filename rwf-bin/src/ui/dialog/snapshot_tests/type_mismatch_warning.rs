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

/// The dialog's min-height is hardcoded to 10 rows regardless of path length
/// (see `calculate_min_dialog_height`'s `TypeMismatchWarning` arm). This
/// snapshot documents current behavior for a long, deeply nested path so a
/// future height-calc change has a baseline to diff against.
#[test]
fn type_mismatch_warning_long_nested_path() {
    let state = test_state();
    let dialog = Dialog::type_mismatch_warning(
        PathBuf::from(
            "/very/deeply/nested/directory/structure/that/keeps/going/on/for/quite/a/while/before/reaching/some_executable_file_with_an_unusually_long_name.exe",
        ),
        DetectedKind::Pe,
        "notepad $F".to_string(),
        Location::Local(PathBuf::from("/test")),
        None,
    );
    snapshot_dialog("type_mismatch_warning_long_path", &dialog, &state);
}
