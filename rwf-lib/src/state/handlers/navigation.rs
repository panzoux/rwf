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
                    pane_model.raw_entries = entries.clone();
                    pane_model.entries = entries;
                    pane_model.is_loading = false;
                    pane_model.apply_sort();
                    pane_model.apply_current_filter();
                    if !pane_model.entries.is_empty() {
                        pane_model.cursor = pane_model.cursor.min(pane_model.entries.len() - 1);
                    } else {
                        pane_model.cursor = 0;
                        pane_model.scroll_offset = 0;
                    }
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    // Clear current entries and set loading state. cursor/scroll_offset
                    // were already set above (restored from navigation_cache, or 0 for a
                    // fresh location) — keep that value; CompleteJob's ReadDirectory
                    // handler clamps it via update_scroll() once entries arrive, instead
                    // of resetting it here and losing the restored position.
                    pane_model.entries.clear();
                    pane_model.is_loading = true;

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
                    crate::model::ActivePane::Left => tab.left_pane.current_location.clone(),
                    crate::model::ActivePane::Right => tab.right_pane.current_location.clone(),
                };

                if let Some(parent) = current_location.parent() {
                    // No navigation_cache entry for `parent` means we've never left it
                    // this session (e.g. it's the app's startup directory) — restored_position
                    // will be None, so ChangeLocation's cursor defaults to 0. Fall back to
                    // selecting the child we're coming from by name, same lookup CompleteJob
                    // already does for `pending_cursor_name` (JumpToFile).
                    let had_cached_position = self.navigation_cache.restore(&parent).is_some();
                    let child_name = current_location.file_name();

                    let result = self.handle_navigation_transition(&Transition::ChangeLocation {
                        pane: *pane,
                        location: parent,
                    });

                    if !had_cached_position {
                        if let Some(name) = child_name {
                            let visible_height = self.ui.layout.pane_height;
                            let scroll_margin = self.config.ui.scroll_offset;
                            let tab_mut = self.current_tab_mut();
                            let pane_model = match pane {
                                crate::model::ActivePane::Left => &mut tab_mut.left_pane,
                                crate::model::ActivePane::Right => &mut tab_mut.right_pane,
                            };
                            if let Some(pos) =
                                pane_model.entries.iter().position(|e| e.name == name)
                            {
                                pane_model.cursor = pos;
                                pane_model.update_scroll(visible_height, scroll_margin);
                            } else {
                                pane_model.pending_cursor_name = Some(name);
                            }
                        }
                    }
                    result
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
                        pane_model.raw_entries = entries.clone();
                        pane_model.entries = entries;
                        pane_model.is_loading = false;
                        pane_model.apply_sort();
                        pane_model.apply_current_filter();
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
