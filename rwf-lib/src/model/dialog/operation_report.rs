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
}

impl OperationReportDialogContent {
    pub fn new(report: crate::model::OperationReport) -> Self {
        let selected = vec![true; report.records.len()];
        Self {
            report,
            cursor: 0,
            selected,
        }
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
}
