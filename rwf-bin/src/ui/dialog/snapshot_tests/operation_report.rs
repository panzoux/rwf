//! Snapshots for `DialogContent::OperationReportView`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::{
    Location, OperationRecord, OperationReport, ReversalAction, UndoAvailability,
};

fn sample_report() -> OperationReport {
    OperationReport {
        id: 1,
        operation_name: "Copy".to_string(),
        records: vec![
            OperationRecord {
                source: Some(Location::Local("a.txt".into())),
                destination: Some(Location::Local("b.txt".into())),
                succeeded: true,
                failure_reason: None,
                undo: UndoAvailability::Available(ReversalAction::Delete {
                    target: Location::Local("b.txt".into()),
                    recreate: None,
                }),
            },
            OperationRecord {
                source: Some(Location::Local("c.txt".into())),
                destination: Some(Location::Local("d.txt".into())),
                succeeded: false,
                failure_reason: Some("Access denied".to_string()),
                undo: UndoAvailability::NotApplicable,
            },
        ],
        finished_at: std::time::SystemTime::UNIX_EPOCH,
        is_undo: false,
    }
}

#[test]
fn operation_report_mixed_success_and_failure() {
    let state = test_state();
    let dialog = Dialog::operation_report_view(sample_report());
    snapshot_dialog(
        "operation_report_mixed_success_and_failure",
        &dialog,
        &state,
    );
}

#[test]
fn operation_report_undo_report_shows_redo_column() {
    let state = test_state();
    let mut report = sample_report();
    report.is_undo = true;
    let dialog = Dialog::operation_report_view(report);
    snapshot_dialog(
        "operation_report_undo_report_shows_redo_column",
        &dialog,
        &state,
    );
}
