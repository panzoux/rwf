use crate::state::{AppState, StateUpdateResult, Transition};

impl AppState {
    pub(crate) fn handle_search_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::StartSearch { query } => {
                self.search.start_search(query.clone());
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdateSearchQuery { query } => {
                self.search.query = query.clone();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::UpdateSearchResults { results } => {
                self.search.results = results.clone();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::NextSearchResult => {
                self.search.next_result();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::PrevSearchResult => {
                self.search.prev_result();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ClearSearch => {
                self.search.clear();
                Some(StateUpdateResult::with_ui_change())
            }
            _ => None,
        }
    }
}
