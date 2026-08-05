//! Snapshots for `DialogContent::TrashBrowser`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::{Location, TrashLocation, TrashRecord};
use std::path::PathBuf;
use std::time::SystemTime;

fn record(name: &str, deleted_at: i64, size: u64) -> TrashRecord {
    TrashRecord {
        original: Location::Local(PathBuf::from(format!("C:\\Users\\test\\{name}"))),
        trash_location: TrashLocation::Fallback {
            trash_path: PathBuf::from(format!("C:\\.rwf-trash\\{name}")),
            trashed_at: deleted_at,
        },
        size,
        modified: SystemTime::UNIX_EPOCH,
    }
}

#[test]
fn trash_browser_two_items_first_selected() {
    let state = test_state();
    let records = vec![
        record("report.docx", 1_700_000_000, 45_000),
        record("old_photo.png", 1_699_000_000, 2_500_000),
    ];
    let dialog = Dialog::trash_browser(records);
    snapshot_dialog("trash_browser_two_items_first", &dialog, &state);
}

#[test]
fn trash_browser_second_item_selected() {
    let state = test_state();
    let records = vec![
        record("report.docx", 1_700_000_000, 45_000),
        record("old_photo.png", 1_699_000_000, 2_500_000),
    ];
    let mut dialog = Dialog::trash_browser(records);
    if let rwf_lib::model::dialog::DialogContent::TrashBrowser(
        rwf_lib::model::dialog::TrashBrowserDialog {
            ref mut selected_index,
            ..
        },
    ) = dialog.content
    {
        *selected_index = 1;
    }
    snapshot_dialog("trash_browser_second_item_selected", &dialog, &state);
}
