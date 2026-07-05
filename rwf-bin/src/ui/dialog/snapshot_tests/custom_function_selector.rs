//! Snapshots for `DialogContent::CustomFunctionSelector`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{CustomFunction, Dialog};

#[test]
fn custom_function_selector_two_items_first_selected() {
    let state = test_state();
    let functions = vec![
        CustomFunction::new("PrintFile", "cat $F").with_description("Display file contents"),
        CustomFunction::new("CountLines", "wc -l $F").with_description("Count lines in file"),
    ];
    let dialog = Dialog::custom_function_selector(functions);
    snapshot_dialog("custom_function_selector_two_items_first", &dialog, &state);
}

#[test]
fn custom_function_selector_five_items_middle_selected() {
    let state = test_state();
    let functions = vec![
        CustomFunction::new("Alpha", "echo alpha"),
        CustomFunction::new("Beta", "echo beta").with_description("Beta function"),
        CustomFunction::new("Gamma", "echo gamma"),
        CustomFunction::new("Delta", "echo delta").with_description("Delta function"),
        CustomFunction::new("Epsilon", "echo epsilon"),
    ];
    let mut dialog = Dialog::custom_function_selector(functions);
    if let rwf_lib::model::dialog::DialogContent::CustomFunctionSelector {
        ref mut selected_index,
        ..
    } = dialog.content
    {
        *selected_index = 2;
    }
    snapshot_dialog(
        "custom_function_selector_five_items_middle",
        &dialog,
        &state,
    );
}

#[test]
fn custom_function_selector_with_filter() {
    let state = test_state();
    let functions = vec![
        CustomFunction::new("ListFiles", "ls"),
        CustomFunction::new("ListDirs", "ls -d").with_description("Directories only"),
        CustomFunction::new("ListAll", "ls -a"),
        CustomFunction::new("SearchText", "grep").with_description("Find text in files"),
    ];
    let mut dialog = Dialog::custom_function_selector(functions);
    if let rwf_lib::model::dialog::DialogContent::CustomFunctionSelector {
        ref mut filter,
        ref mut selected_index,
        ..
    } = dialog.content
    {
        *filter = "list".to_string();
        *selected_index = 0;
    }
    snapshot_dialog("custom_function_selector_with_filter", &dialog, &state);
}

#[test]
fn custom_function_selector_four_items_last_selected() {
    let state = test_state();
    let functions = vec![
        CustomFunction::new("First", "cmd1"),
        CustomFunction::new("Second", "cmd2"),
        CustomFunction::new("Third", "cmd3"),
        CustomFunction::new("Fourth", "cmd4"),
    ];
    let mut dialog = Dialog::custom_function_selector(functions);
    if let rwf_lib::model::dialog::DialogContent::CustomFunctionSelector {
        ref mut selected_index,
        ..
    } = dialog.content
    {
        *selected_index = 3;
    }
    snapshot_dialog("custom_function_selector_four_items_last", &dialog, &state);
}
