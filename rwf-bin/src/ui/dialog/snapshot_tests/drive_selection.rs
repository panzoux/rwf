//! Snapshots for `DialogContent::DriveSelection`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, DriveInfo, DriveType};
use rwf_lib::model::ui::ActivePane;

#[test]
fn drive_selection_two_drives_first_selected() {
    let state = test_state();
    let drives = vec![
        DriveInfo {
            path: "C:\\".to_string(),
            label: "System".to_string(),
            drive_type: DriveType::Local,
            total_space: Some(1000000000000),
            free_space: Some(500000000000),
        },
        DriveInfo {
            path: "D:\\".to_string(),
            label: "Data".to_string(),
            drive_type: DriveType::Local,
            total_space: Some(2000000000000),
            free_space: Some(1500000000000),
        },
    ];
    let dialog = Dialog::drive_selection(drives, ActivePane::Left);
    snapshot_dialog("drive_selection_two_drives_first", &dialog, &state);
}

#[test]
fn drive_selection_mixed_types_middle_selected() {
    let state = test_state();
    let drives = vec![
        DriveInfo {
            path: "C:\\".to_string(),
            label: "Windows".to_string(),
            drive_type: DriveType::Local,
            total_space: Some(512000000000),
            free_space: Some(256000000000),
        },
        DriveInfo {
            path: "E:\\".to_string(),
            label: "USB".to_string(),
            drive_type: DriveType::Removable,
            total_space: Some(32000000000),
            free_space: Some(16000000000),
        },
        DriveInfo {
            path: "\\\\server\\share".to_string(),
            label: "".to_string(),
            drive_type: DriveType::Network,
            total_space: None,
            free_space: None,
        },
    ];
    let mut dialog = Dialog::drive_selection(drives, ActivePane::Right);
    if let rwf_lib::model::dialog::DialogContent::DriveSelection {
        ref mut selected_index,
        ..
    } = dialog.content
    {
        *selected_index = 1;
    }
    snapshot_dialog("drive_selection_mixed_types_middle", &dialog, &state);
}

#[test]
fn drive_selection_with_filter() {
    let state = test_state();
    let drives = vec![
        DriveInfo {
            path: "C:\\".to_string(),
            label: "System".to_string(),
            drive_type: DriveType::Local,
            total_space: Some(1000000000000),
            free_space: Some(500000000000),
        },
        DriveInfo {
            path: "D:\\".to_string(),
            label: "Data".to_string(),
            drive_type: DriveType::Local,
            total_space: Some(2000000000000),
            free_space: Some(1500000000000),
        },
        DriveInfo {
            path: "E:\\".to_string(),
            label: "Backup".to_string(),
            drive_type: DriveType::Removable,
            total_space: Some(1000000000000),
            free_space: Some(800000000000),
        },
    ];
    let mut dialog = Dialog::drive_selection(drives, ActivePane::Left);
    if let rwf_lib::model::dialog::DialogContent::DriveSelection { ref mut filter, .. } =
        dialog.content
    {
        *filter = "d".to_string();
    }
    snapshot_dialog("drive_selection_with_filter", &dialog, &state);
}

#[test]
fn drive_selection_empty_drives() {
    let state = test_state();
    let drives = vec![];
    let dialog = Dialog::drive_selection(drives, ActivePane::Right);
    snapshot_dialog("drive_selection_empty_drives", &dialog, &state);
}
