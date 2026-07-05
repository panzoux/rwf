//! Snapshots for `DialogContent::CustomFunctionMenu`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, MenuItem};

#[test]
fn custom_function_menu_two_items_first_selected() {
    let state = test_state();
    let items = vec![
        MenuItem {
            name: "OpenText".to_string(),
            action: "open_text".to_string(),
        },
        MenuItem {
            name: "EditBinary".to_string(),
            action: "edit_binary".to_string(),
        },
    ];
    let dialog = Dialog::custom_function_menu("File Menu".to_string(), items);
    snapshot_dialog("custom_function_menu_two_items_first", &dialog, &state);
}

#[test]
fn custom_function_menu_with_separators() {
    let state = test_state();
    let items = vec![
        MenuItem {
            name: "Copy".to_string(),
            action: "copy".to_string(),
        },
        MenuItem {
            name: "-----".to_string(),
            action: "".to_string(),
        },
        MenuItem {
            name: "Paste".to_string(),
            action: "paste".to_string(),
        },
        MenuItem {
            name: "-----".to_string(),
            action: "".to_string(),
        },
        MenuItem {
            name: "Delete".to_string(),
            action: "delete".to_string(),
        },
    ];
    let dialog = Dialog::custom_function_menu("Edit Menu".to_string(), items);
    snapshot_dialog("custom_function_menu_with_separators", &dialog, &state);
}

#[test]
fn custom_function_menu_four_items_middle_selected() {
    let state = test_state();
    let items = vec![
        MenuItem {
            name: "First".to_string(),
            action: "first".to_string(),
        },
        MenuItem {
            name: "Second".to_string(),
            action: "second".to_string(),
        },
        MenuItem {
            name: "Third".to_string(),
            action: "third".to_string(),
        },
        MenuItem {
            name: "Fourth".to_string(),
            action: "fourth".to_string(),
        },
    ];
    let mut dialog = Dialog::custom_function_menu("Main".to_string(), items);
    if let rwf_lib::model::dialog::DialogContent::CustomFunctionMenu {
        ref mut selected_index,
        ..
    } = dialog.content
    {
        *selected_index = 2;
    }
    snapshot_dialog("custom_function_menu_four_items_middle", &dialog, &state);
}

#[test]
fn custom_function_menu_three_items_last_selected() {
    let state = test_state();
    let items = vec![
        MenuItem {
            name: "Action1".to_string(),
            action: "action1".to_string(),
        },
        MenuItem {
            name: "Action2".to_string(),
            action: "action2".to_string(),
        },
        MenuItem {
            name: "Action3".to_string(),
            action: "action3".to_string(),
        },
    ];
    let mut dialog = Dialog::custom_function_menu("Tools".to_string(), items);
    if let rwf_lib::model::dialog::DialogContent::CustomFunctionMenu {
        ref mut selected_index,
        ..
    } = dialog.content
    {
        *selected_index = 2;
    }
    snapshot_dialog("custom_function_menu_three_items_last", &dialog, &state);
}
