use crate::state::{AppState, StateUpdateResult, Transition};

impl AppState {
    pub(crate) fn handle_marking_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::ToggleMark { location } => {
                self.active_pane_mut().marking.toggle(location.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkAll => {
                let pane = self.active_pane_mut();
                pane.marking.mark_all(&pane.entries);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UnmarkAll => {
                self.active_pane_mut().marking.unmark_all();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkPattern { pattern } => {
                let pane = self.active_pane_mut();
                pane.marking.mark_pattern(&pane.entries, pattern);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkRange { start, end } => {
                let pane = self.active_pane_mut();
                pane.marking.mark_range(&pane.entries, *start, *end);
                self.ui.range_marking_start = None;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::InvertMarks => {
                let pane = self.active_pane_mut();
                pane.marking.invert_marks(&pane.entries);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::EnterRangeMarkingMode => {
                let cursor = self.active_pane().cursor;
                self.ui.range_marking_start = Some(cursor);
                Some(StateUpdateResult::with_ui_change())
            }
            _ => None,
        }
    }
}
