//! Snapshots for `DialogContent::CloseTabWithActiveJob`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{CloseTabWithActiveJobDialog, Dialog, DialogContent};

#[test]
fn close_tab_with_one_job_ok_focused() {
    let state = test_state();
    let dialog = Dialog {
        title: "Close Tab".to_string(),
        content: DialogContent::CloseTabWithActiveJob(CloseTabWithActiveJobDialog::new(
            0,
            "Tab 1".to_string(),
            vec![1],
            0, // OK focused
        )),
    };
    snapshot_dialog("close_tab_one_job_ok", &dialog, &state);
}

#[test]
fn close_tab_with_one_job_cancel_focused() {
    let state = test_state();
    let dialog = Dialog {
        title: "Close Tab".to_string(),
        content: DialogContent::CloseTabWithActiveJob(CloseTabWithActiveJobDialog::new(
            0,
            "Tab 1".to_string(),
            vec![1],
            1, // Cancel focused
        )),
    };
    snapshot_dialog("close_tab_one_job_cancel", &dialog, &state);
}

#[test]
fn close_tab_with_multiple_jobs_ok_focused() {
    let state = test_state();
    let dialog = Dialog {
        title: "Close Tab".to_string(),
        content: DialogContent::CloseTabWithActiveJob(CloseTabWithActiveJobDialog::new(
            1,
            "Tab 2".to_string(),
            vec![1, 2],
            0, // OK focused
        )),
    };
    snapshot_dialog("close_tab_two_jobs_ok", &dialog, &state);
}

#[test]
fn close_tab_with_multiple_jobs_cancel_focused() {
    let state = test_state();
    let dialog = Dialog {
        title: "Close Tab".to_string(),
        content: DialogContent::CloseTabWithActiveJob(CloseTabWithActiveJobDialog::new(
            1,
            "Tab 2".to_string(),
            vec![1, 2],
            1, // Cancel focused
        )),
    };
    snapshot_dialog("close_tab_two_jobs_cancel", &dialog, &state);
}
