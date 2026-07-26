//! Snapshots for `DialogContent::OpenWithPicker`.

use super::{snapshot_dialog, test_state};
use rwf_lib::config::ExtensionAssociation;
use rwf_lib::model::dialog::{Dialog, DialogContent, OpenWithPickerDialog};
use std::path::PathBuf;

#[test]
fn open_with_picker_two_candidates_first_selected() {
    let state = test_state();
    let candidates = vec![
        ExtensionAssociation {
            extension: Some("log".to_string()),
            file_type: None,
            command: "less $F".to_string(),
            description: Some("View with less".to_string()),
            shell: None,
        },
        ExtensionAssociation {
            extension: Some("log".to_string()),
            file_type: None,
            command: "notepad $F".to_string(),
            description: Some("Edit with Notepad".to_string()),
            shell: None,
        },
    ];
    let dialog = Dialog::open_with_picker(vec![PathBuf::from("/test/server.log")], candidates);
    snapshot_dialog("open_with_picker_two_first", &dialog, &state);
}

#[test]
fn open_with_picker_three_candidates_middle_selected() {
    let state = test_state();
    let candidates = vec![
        ExtensionAssociation {
            extension: Some("txt".to_string()),
            file_type: None,
            command: "less $F".to_string(),
            description: Some("View with less".to_string()),
            shell: None,
        },
        ExtensionAssociation {
            extension: Some("txt".to_string()),
            file_type: None,
            command: "notepad $F".to_string(),
            description: Some("Edit with Notepad".to_string()),
            shell: None,
        },
        ExtensionAssociation {
            extension: Some("txt".to_string()),
            file_type: None,
            command: "vim $F".to_string(),
            description: None, // falls back to raw command string
            shell: Some("bash".to_string()),
        },
    ];
    let mut dialog = Dialog::open_with_picker(vec![PathBuf::from("/test/notes.txt")], candidates);
    if let DialogContent::OpenWithPicker(OpenWithPickerDialog {
        ref mut selected_index,
        ..
    }) = dialog.content
    {
        *selected_index = 1;
    }
    snapshot_dialog("open_with_picker_three_middle", &dialog, &state);
}

/// Batch "Open With..." (Phase 7.3 §3, multi-select): the picker's title notes the
/// file count when it's showing candidates for a whole marked-file group instead
/// of a single cursor file.
#[test]
fn open_with_picker_multi_file_group_title_shows_count() {
    let state = test_state();
    let candidates = vec![
        ExtensionAssociation {
            extension: Some("log".to_string()),
            file_type: None,
            command: "less $F".to_string(),
            description: Some("View with less".to_string()),
            shell: None,
        },
        ExtensionAssociation {
            extension: Some("log".to_string()),
            file_type: None,
            command: "notepad $F".to_string(),
            description: Some("Edit with Notepad".to_string()),
            shell: None,
        },
    ];
    let dialog = Dialog::open_with_picker(
        vec![
            PathBuf::from("/test/a.log"),
            PathBuf::from("/test/b.log"),
            PathBuf::from("/test/c.log"),
        ],
        candidates,
    );
    assert_eq!(dialog.title, "Open With... (3 files)");
    snapshot_dialog("open_with_picker_multi_file_group", &dialog, &state);
}
