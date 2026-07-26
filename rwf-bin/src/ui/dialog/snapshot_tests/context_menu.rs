//! Snapshots for `DialogContent::ContextMenu`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{ContextMenuAction, ContextMenuDialog, ContextMenuOption, Dialog};

#[test]
fn context_menu_default_options_first_selected() {
    let state = test_state();
    let dialog = Dialog::context_menu();
    snapshot_dialog("context_menu_default_options_first", &dialog, &state);
}

#[test]
fn context_menu_default_options_middle_selected() {
    let state = test_state();
    let mut dialog = Dialog::context_menu();
    if let rwf_lib::model::dialog::DialogContent::ContextMenu(ContextMenuDialog {
        ref mut selected_index,
        ..
    }) = dialog.content
    {
        *selected_index = 3;
    }
    snapshot_dialog("context_menu_default_options_middle", &dialog, &state);
}

#[test]
fn context_menu_custom_options() {
    let state = test_state();
    let options = vec![
        ContextMenuOption {
            label: "Compress".to_string(),
            action: ContextMenuAction::CustomFunction("compress".to_string()),
        },
        ContextMenuOption {
            label: "─────".to_string(),
            action: ContextMenuAction::Separator,
        },
        ContextMenuOption {
            label: "Extract".to_string(),
            action: ContextMenuAction::CustomFunction("extract".to_string()),
        },
    ];
    let dialog = Dialog::context_menu_with_options(options);
    snapshot_dialog("context_menu_custom_options", &dialog, &state);
}

#[test]
fn context_menu_extended_options_last_selected() {
    let state = test_state();
    let options = vec![
        ContextMenuOption {
            label: "Edit".to_string(),
            action: ContextMenuAction::CustomFunction("edit".to_string()),
        },
        ContextMenuOption {
            label: "─────".to_string(),
            action: ContextMenuAction::Separator,
        },
        ContextMenuOption {
            label: "Copy".to_string(),
            action: ContextMenuAction::Copy,
        },
        ContextMenuOption {
            label: "Move".to_string(),
            action: ContextMenuAction::Move,
        },
        ContextMenuOption {
            label: "─────".to_string(),
            action: ContextMenuAction::Separator,
        },
        ContextMenuOption {
            label: "Delete".to_string(),
            action: ContextMenuAction::Delete,
        },
    ];
    let mut dialog = Dialog::context_menu_with_options(options);
    if let rwf_lib::model::dialog::DialogContent::ContextMenu(ContextMenuDialog {
        ref mut selected_index,
        ..
    }) = dialog.content
    {
        *selected_index = 5;
    }
    snapshot_dialog("context_menu_extended_options_last", &dialog, &state);
}

/// Phase 7.3b Task 9: once live content-type detection completes, the
/// "Open With..." row shows the detected type appended to its label — e.g.
/// "Open With... (PNG image)" — so the user sees why a picker would show up
/// without opening File Info first.
#[test]
fn context_menu_open_with_row_shows_detected_type() {
    let state = test_state();
    let mut dialog = Dialog::context_menu();
    if let rwf_lib::model::dialog::DialogContent::ContextMenu(ContextMenuDialog {
        ref mut detected_type_label,
        ..
    }) = dialog.content
    {
        *detected_type_label = Some("PNG image".to_string());
    }
    snapshot_dialog("context_menu_open_with_row_detected_type", &dialog, &state);
}

/// While the detect job is still in flight, the row shows "(detecting...)"
/// instead of a resolved label.
#[test]
fn context_menu_open_with_row_shows_detecting_placeholder() {
    let state = test_state();
    let mut dialog = Dialog::context_menu();
    if let rwf_lib::model::dialog::DialogContent::ContextMenu(ContextMenuDialog {
        ref mut detected_type_job_id,
        ..
    }) = dialog.content
    {
        *detected_type_job_id = Some(rwf_lib::job::JobId::new());
    }
    snapshot_dialog("context_menu_open_with_row_detecting", &dialog, &state);
}
