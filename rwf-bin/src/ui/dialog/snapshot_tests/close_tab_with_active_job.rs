//! Snapshots for `DialogContent::CloseTabWithActiveJob`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::{Dialog, DialogContent};

#[test]
fn close_tab_with_one_job_ok_focused() {
    let state = test_state();
    let dialog = Dialog {
        title: "Close Tab".to_string(),
        content: DialogContent::CloseTabWithActiveJob {
            tab_index: 0,
            tab_name: "Tab 1".to_string(),
            job_ids: vec![1],
            focused_field: 0, // OK focused
        },
    };
    snapshot_dialog("close_tab_one_job_ok", &dialog, &state);
}

#[test]
fn close_tab_with_one_job_cancel_focused() {
    let state = test_state();
    let dialog = Dialog {
        title: "Close Tab".to_string(),
        content: DialogContent::CloseTabWithActiveJob {
            tab_index: 0,
            tab_name: "Tab 1".to_string(),
            job_ids: vec![1],
            focused_field: 1, // Cancel focused
        },
    };
    snapshot_dialog("close_tab_one_job_cancel", &dialog, &state);
}

#[test]
fn close_tab_with_multiple_jobs_ok_focused() {
    let state = test_state();
    let dialog = Dialog {
        title: "Close Tab".to_string(),
        content: DialogContent::CloseTabWithActiveJob {
            tab_index: 1,
            tab_name: "Tab 2".to_string(),
            job_ids: vec![1, 2],
            focused_field: 0, // OK focused
        },
    };
    snapshot_dialog("close_tab_two_jobs_ok", &dialog, &state);
}

#[test]
fn close_tab_with_multiple_jobs_cancel_focused() {
    let state = test_state();
    let dialog = Dialog {
        title: "Close Tab".to_string(),
        content: DialogContent::CloseTabWithActiveJob {
            tab_index: 1,
            tab_name: "Tab 2".to_string(),
            job_ids: vec![1, 2],
            focused_field: 1, // Cancel focused
        },
    };
    snapshot_dialog("close_tab_two_jobs_cancel", &dialog, &state);
}
