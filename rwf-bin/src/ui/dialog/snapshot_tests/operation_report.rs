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

/// CJK filenames are double-width per character — proves the Source/
/// Destination columns are padded and truncated by display width
/// (`pad_to_width`/`smart_truncate`), not by Rust `char` count
/// (`{:<28}`-style formatting), so a long CJK name doesn't push the
/// Result/Undo columns out of alignment.
#[test]
fn operation_report_cjk_filenames_stay_aligned() {
    let state = test_state();
    let report = OperationReport {
        id: 1,
        operation_name: "Copy".to_string(),
        records: vec![OperationRecord {
            source: Some(Location::Local("日本語ファイル名前.txt".into())),
            destination: Some(Location::Local("コピー先フォルダ\\日本語.txt".into())),
            succeeded: true,
            failure_reason: None,
            undo: UndoAvailability::Available(ReversalAction::Delete {
                target: Location::Local("コピー先フォルダ\\日本語.txt".into()),
                recreate: None,
            }),
        }],
        finished_at: std::time::SystemTime::UNIX_EPOCH,
        is_undo: false,
    };
    let dialog = Dialog::operation_report_view(report);
    snapshot_dialog(
        "operation_report_cjk_filenames_stay_aligned",
        &dialog,
        &state,
    );
}

#[test]
fn operation_report_viewing_an_older_report_shows_position_and_view_only_hint() {
    let state = test_state();
    let report = sample_report();
    let dialog = Dialog::operation_report_view_at(report, 1, 5, false); // displays as [4 of 5], not actionable
    snapshot_dialog(
        "operation_report_viewing_an_older_report_shows_position_and_view_only_hint",
        &dialog,
        &state,
    );
}

/// The other half of the `history_total > 1` branch: viewing the
/// actionable (stack-top) report of a multi-report history shows the
/// position indicator (no "(view only)" suffix) and the full, still-active
/// hint with "history" appended — as opposed to the non-actionable case
/// covered by the test above.
#[test]
fn operation_report_viewing_the_latest_of_several_reports_shows_position_with_active_hint() {
    let state = test_state();
    let report = sample_report();
    let dialog = Dialog::operation_report_view_at(report, 4, 5, true); // displays as [1 of 5], actionable
    snapshot_dialog(
        "operation_report_viewing_the_latest_of_several_reports_shows_position_with_active_hint",
        &dialog,
        &state,
    );
}

/// `Alt+o` with no operations recorded yet still opens this dialog (rather
/// than only logging to the task panel) — it shows its own empty state:
/// no rows, no `[X of Y]` position indicator, and a bare "Esc: close" hint
/// (no "Space: toggle"/"a: all/none"/undo hint, since there's nothing to
/// select or run).
#[test]
fn operation_report_empty_history_shows_no_operations_message() {
    let state = test_state();
    let report = OperationReport {
        id: 0,
        operation_name: "Operations".to_string(),
        records: vec![],
        finished_at: std::time::SystemTime::UNIX_EPOCH,
        is_undo: false,
    };
    let dialog = Dialog::operation_report_view(report);
    snapshot_dialog(
        "operation_report_empty_history_shows_no_operations_message",
        &dialog,
        &state,
    );
}
