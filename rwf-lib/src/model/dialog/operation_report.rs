//! Operation Report dialog state (Phase 7.6).

/// Which report in `AppState.operation_reports` is being viewed, the cursor
/// row within it, and per-row checkbox selection for "Undo/Redo Selected".
#[derive(Debug, Clone, PartialEq)]
pub struct OperationReportDialogContent {
    /// Snapshot of the report being displayed — dialogs don't borrow
    /// `AppState` (see `Dialog`/`DialogStack`), so this is a clone taken when
    /// the dialog was opened. Re-opening after a further Undo/Redo shows the
    /// newest report again (that's the point — Undo/Redo push a fresh
    /// report, and the dialog re-reads history each time it's opened).
    pub report: crate::model::OperationReport,
    pub cursor: usize,
    /// One entry per `report.records` row; `true` = selected for the next
    /// Undo/Redo trigger. Starts all-`true` (design doc's "全件戻す" default
    /// — an explicit deselect is needed to exclude a row, not the reverse).
    pub selected: Vec<bool>,
    /// 0-based position of `report` within `AppState.operation_reports` at
    /// the time this content was built (0 = oldest). Purely for the
    /// browsable position indicator/title and Left/Right navigation — see
    /// `actionable` for whether Undo/Redo can actually run on this report.
    /// Defaults to 0 via `new()` for callers with no navigation context.
    pub history_position: usize,
    /// Total report count at the time this content was built. See
    /// `history_position`.
    pub history_total: usize,
    /// True iff this report is the current Undo *or* Redo target — its id
    /// matched `AppState.undo_stack.last()` or `redo_stack.last()` when
    /// this content was built. This, not `history_position`/
    /// `history_total`, is what gates Enter/marking: after any Undo/Redo
    /// the two no longer coincide (the stack top is rarely
    /// `operation_reports.back()` once anything's been undone), which is
    /// the whole reason `AppState` tracks the stacks separately from the
    /// flat, browsable history — see `AppState::undo_stack`'s doc comment.
    /// Defaults to `true` via `new()`, matching every isolated-report
    /// test/call site that doesn't care about stack context.
    pub actionable: bool,
}

impl OperationReportDialogContent {
    pub fn new(report: crate::model::OperationReport) -> Self {
        let selected = vec![true; report.records.len()];
        Self {
            report,
            cursor: 0,
            selected,
            history_position: 0,
            history_total: 1,
            actionable: true,
        }
    }

    /// Like `new`, but with explicit history-navigation and stack context —
    /// used when the dialog is opened or navigated with real knowledge of
    /// where this report sits in `AppState.operation_reports` and whether
    /// it's the live Undo/Redo target.
    pub fn with_history_position(
        report: crate::model::OperationReport,
        history_position: usize,
        history_total: usize,
        actionable: bool,
    ) -> Self {
        let selected = vec![true; report.records.len()];
        Self {
            report,
            cursor: 0,
            selected,
            history_position,
            history_total,
            actionable,
        }
    }

    /// True iff Undo/Redo may act on this report directly (see `actionable`'s
    /// doc comment). Older/newer reports reached by browsing that aren't the
    /// live stack top are view-only — reaching their state requires
    /// undoing/redoing through everything between here and there first,
    /// matching how every mainstream undo system works.
    pub fn is_actionable(&self) -> bool {
        self.actionable
    }

    /// The `ReversalAction`s for every currently-selected row whose `undo` is
    /// `Available` — rows that are selected but `Unavailable`/`NotApplicable`
    /// are silently skipped (nothing to run), not an error.
    pub fn selected_reversal_actions(&self) -> Vec<crate::model::ReversalAction> {
        self.report
            .records
            .iter()
            .zip(self.selected.iter())
            .filter(|(_, sel)| **sel)
            .filter_map(|(record, _)| match &record.undo {
                crate::model::UndoAvailability::Available(action) => Some(action.clone()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Location, OperationRecord, ReversalAction, UndoAvailability};

    fn report_with(undo: UndoAvailability) -> crate::model::OperationReport {
        crate::model::OperationReport {
            id: 1,
            operation_name: "Copy".to_string(),
            records: vec![OperationRecord {
                source: Some(Location::Local("a.txt".into())),
                destination: Some(Location::Local("b.txt".into())),
                succeeded: true,
                failure_reason: None,
                undo,
            }],
            finished_at: std::time::SystemTime::now(),
            is_undo: false,
        }
    }

    #[test]
    fn starts_with_all_rows_selected() {
        let content = OperationReportDialogContent::new(report_with(UndoAvailability::Available(
            ReversalAction::Delete {
                target: Location::Local("b.txt".into()),
                recreate: None,
            },
        )));
        assert_eq!(content.selected, vec![true]);
        assert_eq!(content.selected_reversal_actions().len(), 1);
    }

    #[test]
    fn deselected_or_unavailable_rows_are_excluded_from_actions() {
        let mut content =
            OperationReportDialogContent::new(report_with(UndoAvailability::NotApplicable));
        assert!(content.selected_reversal_actions().is_empty());

        content.selected[0] = false;
        assert!(content.selected_reversal_actions().is_empty());
    }

    #[test]
    fn deselecting_an_available_row_excludes_it() {
        // Unlike the NotApplicable case above, this row IS actionable
        // (`Available`) — so this is the one scenario that actually exercises
        // the `selected` filter rather than the `undo` filter. Deleting the
        // `.filter(|(_, sel)| **sel)` step in `selected_reversal_actions`
        // would leave this test failing where the others would still pass.
        let mut content = OperationReportDialogContent::new(report_with(
            UndoAvailability::Available(ReversalAction::Delete {
                target: Location::Local("b.txt".into()),
                recreate: None,
            }),
        ));
        assert_eq!(content.selected_reversal_actions().len(), 1);

        content.selected[0] = false;
        assert!(content.selected_reversal_actions().is_empty());
    }

    #[test]
    fn new_defaults_to_actionable() {
        let content =
            OperationReportDialogContent::new(report_with(UndoAvailability::NotApplicable));
        assert_eq!(content.history_position, 0);
        assert_eq!(content.history_total, 1);
        assert!(content.is_actionable());
    }

    #[test]
    fn with_history_position_carries_the_given_actionable_flag() {
        let report = report_with(UndoAvailability::NotApplicable);
        let view_only =
            OperationReportDialogContent::with_history_position(report.clone(), 0, 3, false);
        assert!(!view_only.is_actionable());
        let stack_top = OperationReportDialogContent::with_history_position(report, 2, 3, true);
        assert!(stack_top.is_actionable());
    }
}
