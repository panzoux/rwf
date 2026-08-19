//! Operation Report dialog state (Phase 7.6).

/// Which report in `AppState.operation_reports` is being viewed, the cursor
/// row within it, and per-row checkbox selection for "Undo/Redo Selected".
#[derive(Debug, Clone, PartialEq)]
pub struct OperationReportDialogContent {
    /// Snapshot of the focused report — always `history[history_cursor]`,
    /// kept as its own field (rather than re-derived on every access)
    /// since it's what the row-list/detail panes and
    /// `selected_reversal_actions()` read from.
    pub report: crate::model::OperationReport,
    pub cursor: usize,
    /// One entry per `report.records` row; `true` = selected for the next
    /// Undo/Redo trigger. Starts all-`true` (design doc's "全件戻す" default
    /// — an explicit deselect is needed to exclude a row, not the reverse).
    pub selected: Vec<bool>,
    /// The canonical, browsable Undo/Redo history for the sidebar: one
    /// entry per *original* action, newest first — never the raw flat
    /// `AppState.operation_reports` audit log, which would show a
    /// just-undone report *and* the fresh "undo of it" record as two
    /// separate rows (confusing: "why is the thing I just undid still
    /// listed as undo-able?"). Built by
    /// `AppState::operation_history_slots()`, which interleaves
    /// `redo_stack` (newest-undone first) and `undo_stack` (newest-applied
    /// first, reversed) into this single newest-to-oldest list. Defaults
    /// to `[report]` via `new()` for callers with no navigation context.
    pub history: Vec<crate::model::OperationReport>,
    /// Parallel to `history`: `true` at index *i* iff `history[i]` is the
    /// live Undo *or* Redo target (its id matched `AppState.undo_stack.last()`
    /// or `redo_stack.last()`). Two entries can be `true` at once — the
    /// nearest-redo and nearest-undo sit right next to each other at the
    /// stack boundary. Drives the sidebar's `*` marker, independent of
    /// which entry is merely scrolled into focus. Defaults to `[true]` via
    /// `new()`.
    pub history_actionable: Vec<bool>,
    /// Index into `history`/`history_actionable` of the focused report —
    /// `history[history_cursor] == report`. Defaults to 0 via `new()`.
    pub history_cursor: usize,
    /// True iff `report` (i.e. `history[history_cursor]`) may be acted on
    /// directly — always equal to `history_actionable[history_cursor]`,
    /// kept as its own field for the many call sites that only care about
    /// the focused entry. Older/newer entries reached by browsing
    /// `history` that aren't a live stack top are view-only: reaching
    /// their state requires undoing/redoing through everything between
    /// here and there first, matching how every mainstream undo system
    /// works.
    pub actionable: bool,
}

impl OperationReportDialogContent {
    pub fn new(report: crate::model::OperationReport) -> Self {
        let selected = vec![true; report.records.len()];
        Self {
            history: vec![report.clone()],
            history_actionable: vec![true],
            history_cursor: 0,
            report,
            cursor: 0,
            selected,
            actionable: true,
        }
    }

    /// Like `new`, but for the true "nothing recorded yet" empty state:
    /// `report` (a placeholder, never a real operation) is still shown in
    /// the row-list/detail panes for its own empty-state message, but
    /// `history` is genuinely empty — so the sidebar renders as an empty
    /// box instead of a fake browsable entry for a report that never
    /// happened. The sidebar always renders regardless of history size
    /// (see `render_operation_report_dialog`), so this is what keeps it
    /// honest in the one case `new()`'s `history: vec![report]` would
    /// otherwise mislead.
    pub fn empty(report: crate::model::OperationReport) -> Self {
        let selected = vec![true; report.records.len()];
        Self {
            report,
            cursor: 0,
            selected,
            history: Vec::new(),
            history_actionable: Vec::new(),
            history_cursor: 0,
            actionable: true,
        }
    }

    /// Like `new`, but with explicit sidebar/stack context — used when the
    /// dialog is opened or navigated with real knowledge of
    /// `AppState.undo_stack`/`redo_stack`. `history_cursor` selects the
    /// focused entry within `history`/`history_actionable`; panics if out
    /// of bounds or if the two Vecs' lengths differ (both are always built
    /// together, in lockstep, by
    /// `AppState::operation_report_dialog_for_current_state`/
    /// `NavigateOperationReportHistory`, which guarantee this invariant).
    pub fn with_history(
        history: Vec<crate::model::OperationReport>,
        history_actionable: Vec<bool>,
        history_cursor: usize,
    ) -> Self {
        debug_assert_eq!(history.len(), history_actionable.len());
        let report = history[history_cursor].clone();
        let actionable = history_actionable[history_cursor];
        let selected = vec![true; report.records.len()];
        Self {
            report,
            cursor: 0,
            selected,
            history,
            history_actionable,
            history_cursor,
            actionable,
        }
    }

    /// True iff Undo/Redo may act on `report` directly. See `actionable`'s
    /// doc comment.
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
    fn new_defaults_to_a_single_actionable_history_entry() {
        let content =
            OperationReportDialogContent::new(report_with(UndoAvailability::NotApplicable));
        assert_eq!(content.history.len(), 1);
        assert_eq!(content.history_actionable, vec![true]);
        assert_eq!(content.history_cursor, 0);
        assert!(content.is_actionable());
    }

    #[test]
    fn empty_shows_the_placeholder_report_but_keeps_history_empty() {
        let placeholder = report_with(UndoAvailability::NotApplicable);
        let content = OperationReportDialogContent::empty(placeholder.clone());
        assert_eq!(content.report, placeholder);
        assert!(
            content.history.is_empty(),
            "the sidebar must not show a fake browsable entry for a report that never happened"
        );
        assert!(content.history_actionable.is_empty());
        assert_eq!(content.history_cursor, 0);
    }

    #[test]
    fn with_history_carries_the_given_actionable_flags_and_focuses_the_cursor_entry() {
        let a = report_with(UndoAvailability::NotApplicable);
        let mut b = a.clone();
        b.id = 2;
        b.operation_name = "Move".to_string();
        let history = vec![b.clone(), a];

        let view_only =
            OperationReportDialogContent::with_history(history.clone(), vec![true, false], 1);
        assert!(!view_only.is_actionable());
        assert_eq!(view_only.report.operation_name, "Copy");

        let stack_top = OperationReportDialogContent::with_history(history, vec![true, false], 0);
        assert!(stack_top.is_actionable());
        assert_eq!(stack_top.report.operation_name, "Move");
    }
}
