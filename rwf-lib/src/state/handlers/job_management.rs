use crate::state::{AppState, StateUpdateResult, Transition};
use tracing::debug;

impl AppState {
    pub(crate) fn handle_job_management_transition(
        &mut self,
        transition: &Transition,
    ) -> Option<StateUpdateResult> {
        match transition {
            Transition::CreateBackgroundJob {
                spec,
                name,
                description,
            } => {
                let tab = self.current_tab();
                let tab_name = format!(
                    "{}|{}",
                    tab.left_pane.current_location.display_path(),
                    tab.right_pane.current_location.display_path()
                );
                let tab_id = self.tabs.active_index;

                self.background_jobs.start_job(
                    name.clone(),
                    description.clone(),
                    tab_id,
                    tab_name,
                    spec.clone(),
                );
                Some(StateUpdateResult::with_ui_change())
            }
            Transition::CreateAndStartCountDownJob {
                spec,
                name,
                description,
            }
            | Transition::CreateAndStartFileJob {
                spec,
                name,
                description,
            } => {
                let tab = self.current_tab();
                let tab_name = format!(
                    "{}|{}",
                    tab.left_pane.current_location.display_path(),
                    tab.right_pane.current_location.display_path()
                );
                let tab_id = self.tabs.active_index;

                let bg_job_id = self.background_jobs.start_job(
                    name.clone(),
                    description.clone(),
                    tab_id,
                    tab_name,
                    spec.clone(),
                );

                self.jobs.start_job(spec.clone());

                let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                let log_msg = format!(
                    "{} [Job {}] [Tab {}] {}: Started",
                    timestamp,
                    bg_job_id.short_id,
                    tab_id + 1,
                    name
                );

                Some(StateUpdateResult {
                    jobs_to_start: vec![spec.clone()],
                    jobs_to_cancel: Vec::new(),
                    completed_jobs: Vec::new(),
                    failed_jobs: Vec::new(),
                    cancelled_jobs: Vec::new(),
                    started_jobs: Vec::new(),
                    task_panel_logs: vec![log_msg],
                    panes_to_refresh: Vec::new(),
                    ui_changed: true,
                    reload_keybindings: false,
                })
            }
            Transition::CreatePendingFileJob {
                spec,
                name,
                description: _,
            } => {
                // Create job spec WITHOUT starting it yet
                // Job will be started after conflict detection (or after dialog confirmation)
                debug!(
                    "CreatePendingFileJob: {:?} (will start after conflict check)",
                    spec.kind
                );

                let timestamp = chrono::Local::now().format("[%H:%M:%S]");
                let log_msg = format!(
                    "{} [Pending] {}: Waiting for conflict check",
                    timestamp, name
                );

                Some(StateUpdateResult {
                    jobs_to_start: vec![spec.clone()],
                    jobs_to_cancel: Vec::new(),
                    completed_jobs: Vec::new(),
                    failed_jobs: Vec::new(),
                    cancelled_jobs: Vec::new(),
                    started_jobs: Vec::new(),
                    task_panel_logs: vec![log_msg],
                    panes_to_refresh: Vec::new(),
                    ui_changed: true,
                    reload_keybindings: false,
                })
            }
            _ => None,
        }
    }
}
