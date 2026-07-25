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
            extension: "log".to_string(),
            command: "less $F".to_string(),
            description: Some("View with less".to_string()),
            shell: None,
        },
        ExtensionAssociation {
            extension: "log".to_string(),
            command: "notepad $F".to_string(),
            description: Some("Edit with Notepad".to_string()),
            shell: None,
        },
    ];
    let dialog = Dialog::open_with_picker(PathBuf::from("/test/server.log"), candidates);
    snapshot_dialog("open_with_picker_two_first", &dialog, &state);
}

#[test]
fn open_with_picker_three_candidates_middle_selected() {
    let state = test_state();
    let candidates = vec![
        ExtensionAssociation {
            extension: "txt".to_string(),
            command: "less $F".to_string(),
            description: Some("View with less".to_string()),
            shell: None,
        },
        ExtensionAssociation {
            extension: "txt".to_string(),
            command: "notepad $F".to_string(),
            description: Some("Edit with Notepad".to_string()),
            shell: None,
        },
        ExtensionAssociation {
            extension: "txt".to_string(),
            command: "vim $F".to_string(),
            description: None, // falls back to raw command string
            shell: Some("bash".to_string()),
        },
    ];
    let mut dialog = Dialog::open_with_picker(PathBuf::from("/test/notes.txt"), candidates);
    if let DialogContent::OpenWithPicker(OpenWithPickerDialog {
        ref mut selected_index,
        ..
    }) = dialog.content
    {
        *selected_index = 1;
    }
    snapshot_dialog("open_with_picker_three_middle", &dialog, &state);
}
