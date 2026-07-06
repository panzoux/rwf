use crate::job::JobSpec;
use crate::state::{AppState, StateUpdateResult, Transition};

impl AppState {
    pub(crate) fn handle_view_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::ChangeSortMode { pane, mode } => {
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.sort_mode = *mode;
                pane_model.apply_sort();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ChangeSortOrder { pane, order } => {
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.sort_order = *order;
                pane_model.apply_sort();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ChangeDisplayMode { pane, mode } => {
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.display_mode = *mode;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::SetFileMask { pane, mask } => {
                let pane_height = self.ui.layout.pane_height;
                let scroll_offset = self.config.ui.scroll_offset;
                let tab = self.current_tab_mut();
                let pane_model = match pane {
                    crate::model::ActivePane::Left => &mut tab.left_pane,
                    crate::model::ActivePane::Right => &mut tab.right_pane,
                };
                pane_model.file_mask = mask.clone();
                pane_model.apply_current_filter();
                pane_model.update_scroll(pane_height, scroll_offset);
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::ToggleHidden => {
                self.ui.show_hidden = !self.ui.show_hidden;
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::Refresh { pane }
            | Transition::RefreshAndClearMarks { pane }
            | Transition::RefreshNoClearMarks { pane } => {
                if let Transition::RefreshAndClearMarks { .. } = transition {
                    let cleared_pane = *pane;
                    let tab = self.current_tab_mut();
                    match cleared_pane {
                        crate::model::ActivePane::Left => tab.left_pane.marking.unmark_all(),
                        crate::model::ActivePane::Right => tab.right_pane.marking.unmark_all(),
                    }
                }
                let tab = self.current_tab_mut();
                let tab_id = tab.id;
                let (location, pane_model) = match pane {
                    crate::model::ActivePane::Left => {
                        (tab.left_pane.current_location.clone(), &mut tab.left_pane)
                    }
                    crate::model::ActivePane::Right => {
                        (tab.right_pane.current_location.clone(), &mut tab.right_pane)
                    }
                };
                let job_spec = JobSpec::new(crate::job::JobKind::ReadDirectory { location })
                    .with_requesting_pane(tab_id, *pane);
                pane_model.is_loading = true;
                pane_model.active_job_id = Some(job_spec.id);
                Some(StateUpdateResult::with_job(job_spec))
            }
            _ => None,
        }
    }
}
