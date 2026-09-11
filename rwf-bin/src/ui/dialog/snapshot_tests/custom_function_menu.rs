//! Snapshots for `DialogContent::CustomFunctionMenu`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{CustomFunctionMenuDialog, Dialog, MenuItem};

fn described(name: &str, action: &str, description: &str) -> MenuItem {
    MenuItem {
        description: Some(description.to_string()),
        ..MenuItem::new(name, action)
    }
}

#[test]
fn custom_function_menu_two_items_first_selected() {
    let state = test_state();
    let items = vec![
        MenuItem::new("OpenText", "open_text"),
        MenuItem::new("EditBinary", "edit_binary"),
    ];
    let dialog = Dialog::custom_function_menu("File Menu".to_string(), items);
    snapshot_dialog("custom_function_menu_two_items_first", &dialog, &state);
}

#[test]
fn custom_function_menu_with_separators() {
    let state = test_state();
    let items = vec![
        MenuItem::new("Copy", "copy"),
        MenuItem::new("-----", ""),
        MenuItem::new("Paste", "paste"),
        MenuItem::new("-----", ""),
        MenuItem::new("Delete", "delete"),
    ];
    let dialog = Dialog::custom_function_menu("Edit Menu".to_string(), items);
    snapshot_dialog("custom_function_menu_with_separators", &dialog, &state);
}

#[test]
fn custom_function_menu_four_items_middle_selected() {
    let state = test_state();
    let items = vec![
        MenuItem::new("First", "first"),
        MenuItem::new("Second", "second"),
        MenuItem::new("Third", "third"),
        MenuItem::new("Fourth", "fourth"),
    ];
    let mut dialog = Dialog::custom_function_menu("Main".to_string(), items);
    if let rwf_lib::model::dialog::DialogContent::CustomFunctionMenu(CustomFunctionMenuDialog {
        ref mut selected_index,
        ..
    }) = dialog.content
    {
        *selected_index = 2;
    }
    snapshot_dialog("custom_function_menu_four_items_middle", &dialog, &state);
}

#[test]
fn custom_function_menu_three_items_last_selected() {
    let state = test_state();
    let items = vec![
        MenuItem::new("Action1", "action1"),
        MenuItem::new("Action2", "action2"),
        MenuItem::new("Action3", "action3"),
    ];
    let mut dialog = Dialog::custom_function_menu("Tools".to_string(), items);
    if let rwf_lib::model::dialog::DialogContent::CustomFunctionMenu(CustomFunctionMenuDialog {
        ref mut selected_index,
        ..
    }) = dialog.content
    {
        *selected_index = 2;
    }
    snapshot_dialog("custom_function_menu_three_items_last", &dialog, &state);
}

/// The shipped clip menu: names in one column, descriptions dimmed in a second,
/// a separator with no description, and an item (a built-in action) without one.
#[test]
fn custom_function_menu_with_descriptions() {
    let state = test_state();
    let items = vec![
        described("file name", "file name", "the cursor entry's name"),
        described("full path", "full path", "the cursor entry's full path"),
        described(
            "current directory",
            "current directory",
            "the active pane's directory",
        ),
        MenuItem::new("-----", ""),
        described(
            "marked file names",
            "marked file names",
            "every marked entry's name, one per line",
        ),
        described(
            "marked full paths",
            "marked full paths",
            "every marked entry's full path, one per line",
        ),
        MenuItem::new("reload config", "ReloadConfig"),
    ];
    let dialog = Dialog::custom_function_menu("clip menu".to_string(), items);
    snapshot_dialog("custom_function_menu_with_descriptions", &dialog, &state);
}

/// Width is measured in display columns: a CJK name is two columns per character,
/// so the description column must start after it, not after its byte length.
#[test]
fn custom_function_menu_with_cjk_names() {
    let state = test_state();
    let items = vec![
        described("ファイル名", "a", "the cursor entry's name"),
        described("カレントディレクトリ", "b", "the active pane's directory"),
    ];
    let dialog = Dialog::custom_function_menu("クリップ".to_string(), items);
    snapshot_dialog("custom_function_menu_with_cjk_names", &dialog, &state);
}

/// A name so long that the description column would be under the minimum at 80
/// columns: the descriptions are dropped there, and shown (truncated) at 120.
#[test]
fn custom_function_menu_too_narrow_for_descriptions() {
    let state = test_state();
    let items = vec![
        described(
            "open the selected entry with the configured external editor",
            "a",
            "hands the file to the editor named in config.json and waits for it",
        ),
        described("short", "b", "a short one"),
    ];
    let dialog = Dialog::custom_function_menu("Tools".to_string(), items);
    snapshot_dialog(
        "custom_function_menu_too_narrow_for_descriptions",
        &dialog,
        &state,
    );
}
