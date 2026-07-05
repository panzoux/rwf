//! Snapshots for `DialogContent::ContextMenu`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{ContextMenuAction, ContextMenuOption, Dialog};

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
    if let rwf_lib::model::dialog::DialogContent::ContextMenu {
        ref mut selected_index,
        ..
    } = dialog.content
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
    if let rwf_lib::model::dialog::DialogContent::ContextMenu {
        ref mut selected_index,
        ..
    } = dialog.content
    {
        *selected_index = 5;
    }
    snapshot_dialog("context_menu_extended_options_last", &dialog, &state);
}
