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

/// Regression guard for a long/deeply nested path: the dialog's min-height
/// (the `TypeMismatchWarning` arm of the inline match in `render_dialog`,
/// see mod.rs) sizes itself from how many rows the path wraps into. Before
/// that fix this was hardcoded to 10 rows regardless of path length, which
/// silently clipped the warning message off the bottom of the dialog.
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
