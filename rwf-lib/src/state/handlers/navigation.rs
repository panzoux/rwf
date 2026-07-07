use crate::job::JobSpec;
use crate::state::{AppState, HistoryDirection, StateUpdateResult, Transition};
use tracing::debug;

impl AppState {
    pub(crate) fn handle_navigation_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::SwitchPane => {
                let old_pane = self.ui.active_pane;
                self.ui.active_pane = self.ui.active_pane.opposite();
                debug!(
                    "SwitchPane transition: {:?} -> {:?}",
                    old_pane, self.ui.active_pane
                );
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CursorMove { pane, delta } => {
                let visible_height = self.ui.layout.pane_height;
                let scroll_margin = self.config.ui.scroll_offset;

                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };

                if !pane_model.entries.is_empty() {
                    let new_cursor = (pane_model.cursor as isize + *delta)
                        .max(0)
                        .min(pane_model.entries.len() as isize - 1)
                        as usize;
                    pane_model.cursor = new_cursor;
                    pane_model.update_scroll(visible_height, scroll_margin);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CursorJump { pane, position } => {
                let visible_height = self.ui.layout.pane_height;
                let scroll_margin = self.config.ui.scroll_offset;

                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };

                if !pane_model.entries.is_empty() {
                    pane_model.cursor = (*position).min(pane_model.entries.len().saturating_sub(1));
                    pane_model.update_scroll(visible_height, scroll_margin);
                }
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ChangeLocation { pane, location } => {
                debug!(
                    "ChangeLocation: pane = {:?}, location = {}",
                    pane,
                    location.display_path()
                );

                let cached_entries = self.cache.get(location);

                let tab = self.current_tab();
                let tab_id = tab.id;
                let (current_loc, current_cursor, current_scroll) = match pane {
                    crate::model::ActivePane::Left => (
                        tab.left_pane.current_location.clone(),
                        tab.left_pane.cursor,
                        tab.left_pane.scroll_offset,
                    ),
                    crate::model::ActivePane::Right => (
                        tab.right_pane.current_location.clone(),
                        tab.right_pane.cursor,
                        tab.right_pane.scroll_offset,
                    ),
                };

                self.navigation_cache
                    .save(current_loc.clone(), current_cursor, current_scroll);
                let restored_position = self.navigation_cache.restore(location);

                let tab_mut = self.current_tab_mut();
                tab_mut.history.push(*pane, current_loc);

                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab_mut.left_pane,
                    crate::model::ActivePane::Right => &mut tab_mut.right_pane,
                };
                pane_model.current_location = location.clone();

                if let Some((cached_cursor, cached_scroll)) = restored_position {
                    pane_model.cursor = cached_cursor;
                    pane_model.scroll_offset = cached_scroll;
                } else {
                    pane_model.cursor = 0;
                    pane_model.scroll_offset = 0;
                }

                if let Some(entries) = cached_entries {
                    pane_model.entries = entries;
                    pane_model.is_loading = false;
                    pane_model.apply_sort();
                    if !pane_model.entries.is_empty() {
                        pane_model.cursor = pane_model.cursor.min(pane_model.entries.len() - 1);
                    } else {
                        pane_model.cursor = 0;
                        pane_model.scroll_offset = 0;
                    }
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    // Clear current entries and set loading state
                    pane_model.entries.clear();
                    pane_model.is_loading = true;
                    pane_model.cursor = 0;
                    pane_model.scroll_offset = 0;

                    let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory {
                        location: location.clone(),
                    })
                    .with_requesting_pane(tab_id, *pane);
                    pane_model.active_job_id = Some(job_spec.id);
                    Some(StateUpdateResult::with_job(job_spec))
                }
            }

            Transition::NavigateUp { pane } => {
                let tab = self.current_tab();
                let current_location = match pane {
                    crate::model::ActivePane::Left => &tab.left_pane.current_location,
                    crate::model::ActivePane::Right => &tab.right_pane.current_location,
                };

                if let Some(parent) = current_location.parent() {
                    self.handle_navigation_transition(&Transition::ChangeLocation {
                        pane: *pane,
                        location: parent,
                    })
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::NavigateHistory { pane, direction } => {
                let location = {
                    let tab = self.current_tab_mut();
                    match direction {
                        HistoryDirection::Back => tab.history.go_back(*pane),
                        HistoryDirection::Forward => tab.history.go_forward(*pane),
                    }
                };

                if let Some(location) = location {
                    let cached_entries = self.cache.get(&location);
                    let tab = self.current_tab_mut();
                    let pane_model = match pane {
                        crate::model::ActivePane::Left => &mut tab.left_pane,
                        crate::model::ActivePane::Right => &mut tab.right_pane,
                    };
                    pane_model.current_location = location.clone();
                    pane_model.cursor = 0;
                    pane_model.scroll_offset = 0;

                    if let Some(entries) = cached_entries {
                        pane_model.entries = entries;
                        pane_model.is_loading = false;
                        pane_model.apply_sort();
                        Some(StateUpdateResult::with_ui_change())
                    } else {
                        pane_model.entries.clear();
                        pane_model.is_loading = true;
                        let tab_id = self.current_tab().id;
                        let job_spec =
                            JobSpec::new(crate::job::JobKind::ReadDirectory { location })
                                .with_requesting_pane(tab_id, *pane);
                        Some(StateUpdateResult::with_job(job_spec))
                    }
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            _ => None,
        }
    }
}
