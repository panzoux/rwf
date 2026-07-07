use crate::job::JobSpec;
use crate::state::{AppState, StateUpdateResult, Transition};

impl AppState {
    pub(crate) fn handle_tab_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::CreateTab => {
                let now = std::time::Instant::now();
                if let Some(last) = self.last_tab_created {
                    if now.duration_since(last) < std::time::Duration::from_millis(300) {
                        return Some(StateUpdateResult::none());
                    }
                }
                self.last_tab_created = Some(now);

                let new_index = self.tabs.create_tab();
                // Get the stable ID of the new tab
                let tab_id = self.tabs.tabs[new_index].id;

                // Fetch locations and set loading state
                let left_loc = self.tabs.tabs[new_index].left_pane.current_location.clone();
                let right_loc = self.tabs.tabs[new_index]
                    .right_pane
                    .current_location
                    .clone();

                let job_left =
                    JobSpec::new(crate::job::JobKind::ReadDirectory { location: left_loc })
                        .with_requesting_pane(tab_id, crate::model::ActivePane::Left);

                let job_right = JobSpec::new(crate::job::JobKind::ReadDirectory {
                    location: right_loc,
                })
                .with_requesting_pane(tab_id, crate::model::ActivePane::Right);

                self.tabs.tabs[new_index].left_pane.is_loading = true;
                self.tabs.tabs[new_index].right_pane.is_loading = true;
                self.tabs.tabs[new_index].left_pane.active_job_id = Some(job_left.id);
                self.tabs.tabs[new_index].right_pane.active_job_id = Some(job_right.id);

                tracing::info!("[CreateTab] Created tab index={}, id={}", new_index, tab_id);

                self.tabs.active_index = new_index;

                let mut result = StateUpdateResult::with_ui_change();
                result.jobs_to_start.push(job_left);
                result.jobs_to_start.push(job_right);
                Some(result)
            }
            Transition::CloseTab { index } => {
                if *index >= self.tabs.tabs.len() {
                    return Some(StateUpdateResult::none());
                }

                let tab_id = self.tabs.tabs[*index].id;
                let is_active = *index == self.tabs.active_index;

                // Cancel any viewer job saved in the tab being closed.
                if is_active {
                    // Drop live viewer state (cancel job, reset fields).
                    if let Some(job_id) = self.viewer_job_id.take() {
                        self.jobs.request_cancel(job_id);
                    }
                    if let Some(job_id) = self.viewer_search_job_id.take() {
                        self.jobs.request_cancel(job_id);
                    }
                    self.viewer = None;
                    self.ui.layout.viewer_layout = crate::model::ViewerLayout::FullScreen;
                    if matches!(
                        self.ui.mode,
                        crate::model::UIMode::Viewer
                            | crate::model::UIMode::ViewerSearch
                            | crate::model::UIMode::ViewerCommand
                    ) {
                        self.ui.mode = crate::model::UIMode::Normal;
                    }
                } else if let Some(job_id) = self.tabs.tabs[*index].tab_viewer.viewer_job_id {
                    self.jobs.request_cancel(job_id);
                }

                // Collect and cancel all active jobs for this tab
                let active_jobs: Vec<crate::job::JobId> = self
                    .background_jobs
                    .get_active_jobs()
                    .filter(|j| j.tab_id == tab_id)
                    .map(|j| j.id.uuid)
                    .collect();

                for id in &active_jobs {
                    self.background_jobs.cancel_job(*id);
                }

                if self.tabs.close_tab(*index) {
                    if self.tabs.active_index >= self.tabs.tabs.len() {
                        self.tabs.active_index = self.tabs.tabs.len().saturating_sub(1);
                    }
                    if is_active {
                        self.restore_viewer_from_tab();
                    }
                    let mut result = StateUpdateResult::with_ui_change();
                    result.jobs_to_cancel = active_jobs;
                    Some(result)
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            Transition::NextTab => {
                self.save_viewer_to_current_tab();
                self.tabs.switch_to_next();
                self.restore_viewer_from_tab();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::PrevTab => {
                self.save_viewer_to_current_tab();
                self.tabs.switch_to_prev();
                self.restore_viewer_from_tab();
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::SwitchTab { index } => {
                if *index < self.tabs.tabs.len() {
                    self.save_viewer_to_current_tab();
                    self.tabs.active_index = *index;
                    self.restore_viewer_from_tab();
                    Some(StateUpdateResult::with_ui_change())
                } else {
                    Some(StateUpdateResult::none())
                }
            }
            _ => None,
        }
    }
}
