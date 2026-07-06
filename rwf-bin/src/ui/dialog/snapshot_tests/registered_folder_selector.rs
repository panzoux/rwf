//! Snapshots for `DialogContent::RegisteredFolderSelector`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, RegisteredFolder, RegisteredFolderSelectorContent};

#[test]
fn registered_folder_selector_two_items_first_selected() {
    let state = test_state();
    let folders = vec![
        RegisteredFolder::new("Projects", "/home/user/projects")
            .with_description("Active development projects"),
        RegisteredFolder::new("Downloads", "/home/user/downloads"),
    ];
    let dialog = Dialog::registered_folder_selector(folders);
    snapshot_dialog(
        "registered_folder_selector_two_items_first",
        &dialog,
        &state,
    );
}

#[test]
fn registered_folder_selector_five_items_middle_selected() {
    let state = test_state();
    let folders = vec![
        RegisteredFolder::new("Desktop", "/home/user/desktop"),
        RegisteredFolder::new("Documents", "/home/user/documents")
            .with_description("Personal documents"),
        RegisteredFolder::new("Music", "/home/user/music"),
        RegisteredFolder::new("Pictures", "/home/user/pictures").with_description("Photo library"),
        RegisteredFolder::new("Videos", "/home/user/videos"),
    ];
    let mut dialog = Dialog::registered_folder_selector(folders);
    if let rwf_lib::model::dialog::DialogContent::RegisteredFolderSelector(
        RegisteredFolderSelectorContent {
            ref mut selected_index,
            ..
        },
    ) = dialog.content
    {
        *selected_index = 2;
    }
    snapshot_dialog(
        "registered_folder_selector_five_items_middle",
        &dialog,
        &state,
    );
}

#[test]
fn registered_folder_selector_with_filter() {
    let state = test_state();
    let folders = vec![
        RegisteredFolder::new("Configuration", "/etc/config"),
        RegisteredFolder::new("CacheData", "/var/cache"),
        RegisteredFolder::new("Configs", "/home/user/.config"),
        RegisteredFolder::new("Templates", "/home/user/templates"),
    ];
    let mut dialog = Dialog::registered_folder_selector(folders);
    if let rwf_lib::model::dialog::DialogContent::RegisteredFolderSelector(
        RegisteredFolderSelectorContent {
            ref mut filter,
            ref mut selected_index,
            ..
        },
    ) = dialog.content
    {
        *filter = "config".to_string();
        *selected_index = 0;
    }
    snapshot_dialog("registered_folder_selector_with_filter", &dialog, &state);
}

#[test]
fn registered_folder_selector_three_items_last_selected() {
    let state = test_state();
    let folders = vec![
        RegisteredFolder::new("Work", "/data/work"),
        RegisteredFolder::new("Personal", "/data/personal"),
        RegisteredFolder::new("Archive", "/data/archive").with_description("Old projects"),
    ];
    let mut dialog = Dialog::registered_folder_selector(folders);
    if let rwf_lib::model::dialog::DialogContent::RegisteredFolderSelector(
        RegisteredFolderSelectorContent {
            ref mut selected_index,
            ..
        },
    ) = dialog.content
    {
        *selected_index = 2;
    }
    snapshot_dialog(
        "registered_folder_selector_three_items_last",
        &dialog,
        &state,
    );
}
