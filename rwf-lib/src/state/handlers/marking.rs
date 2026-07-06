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
                let entries = self.active_pane().entries.clone();
                self.active_pane_mut().marking.mark_all(&entries);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UnmarkAll => {
                self.active_pane_mut().marking.unmark_all();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkPattern { pattern } => {
                let entries = self.active_pane().entries.clone();
                self.active_pane_mut()
                    .marking
                    .mark_pattern(&entries, pattern);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::MarkRange { start, end } => {
                let entries = self.active_pane().entries.clone();
                self.active_pane_mut()
                    .marking
                    .mark_range(&entries, *start, *end);
                self.ui.range_marking_start = None;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::InvertMarks => {
                let entries = self.active_pane().entries.clone();
                self.active_pane_mut().marking.invert_marks(&entries);
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
