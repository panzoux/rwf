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

/// Five distinct reports sharing `sample_report()`'s single-row shape, ids
/// 1..=5 — a stand-in "canonical Undo/Redo history"
/// (`AppState::operation_history_slots()`'s real output) for exercising the
/// sidebar and title across more than one entry.
fn sample_history() -> Vec<OperationReport> {
    (1..=5)
        .map(|id| {
            let mut r = sample_report();
            r.id = id;
            r
        })
        .collect()
}

#[test]
fn operation_report_viewing_a_non_actionable_history_entry_shows_view_only_hint() {
    let state = test_state();
    let history = sample_history();
    let actionable = vec![false, false, false, false, true]; // only the newest (id 5) is live
    let dialog = Dialog::operation_report_view_at(history, actionable, 2); // browsed to a non-actionable entry
    snapshot_dialog(
        "operation_report_viewing_a_non_actionable_history_entry_shows_view_only_hint",
        &dialog,
        &state,
    );
}

/// The other half: viewing the actionable (live stack-top) entry of a
/// multi-report history shows the full, still-active hint with "history"
/// appended and no "(view only)" title suffix — as opposed to the
/// non-actionable case covered by the test above.
#[test]
fn operation_report_viewing_the_actionable_history_entry_shows_active_hint() {
    let state = test_state();
    let history = sample_history();
    let actionable = vec![false, false, false, false, true];
    let dialog = Dialog::operation_report_view_at(history, actionable, 4); // focused on the live target
    snapshot_dialog(
        "operation_report_viewing_the_actionable_history_entry_shows_active_hint",
        &dialog,
        &state,
    );
}

/// The sidebar itself: `*` marks both the nearest-redo and nearest-undo
/// entries (the two live targets, adjacent at the stack boundary), `r:`/`u:`
/// prefixes reflect each entry's own direction (`OperationReport::is_undo`),
/// and the focused entry (cursor) is highlighted independently of whether
/// it's one of the starred ones.
#[test]
fn operation_report_sidebar_marks_both_stack_boundaries() {
    let state = test_state();
    let history: Vec<OperationReport> = [
        ("Rename", true),
        ("Create Directory", true),
        ("Create Directory", false),
        ("Copy", false),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (name, is_undo))| {
        let mut r = sample_report();
        r.id = i as u64 + 1;
        r.operation_name = name.to_string();
        r.is_undo = is_undo;
        r
    })
    .collect();
    // Boundary sits between index 1 (nearest redo) and index 2 (nearest
    // undo) — both starred; focus on the undo side, matching what a fresh
    // Alt+o open would show.
    let actionable = vec![false, true, true, false];
    let dialog = Dialog::operation_report_view_at(history, actionable, 2);
    snapshot_dialog(
        "operation_report_sidebar_marks_both_stack_boundaries",
        &dialog,
        &state,
    );
}

/// `Alt+o` with no operations recorded yet still opens this dialog (rather
/// than only logging to the task panel) — it shows its own empty state: no
/// rows, a bare "Esc: close" hint (no "Space: toggle"/"a: all/none"/undo
/// hint, since there's nothing to select or run), and — since the sidebar
/// always renders (see `render_operation_report_dialog`) — an empty
/// sidebar box, not a fake browsable entry for a report that never
/// happened (`Dialog::operation_report_empty`, not `operation_report_view`
/// with a placeholder report).
#[test]
fn operation_report_empty_history_shows_no_operations_message() {
    let state = test_state();
    let dialog = Dialog::operation_report_empty();
    snapshot_dialog(
        "operation_report_empty_history_shows_no_operations_message",
        &dialog,
        &state,
    );
}
