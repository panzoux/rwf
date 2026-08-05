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

                self.background_jobs.start_job(
                    name.clone(),
                    description.clone(),
                    tab_id,
                    tab_name,
                    spec.clone(),
                );

                self.jobs.start_job(spec.clone());

                // "Started" is logged solely by Transition::JobStarted, fired
                // when the worker pool actually picks the job up — the one
                // event path common to every dispatch route (including the
                // queued-job path). Don't duplicate it here.
                Some(StateUpdateResult {
                    jobs_to_start: vec![spec.clone()],
                    jobs_to_cancel: Vec::new(),
                    completed_jobs: Vec::new(),
                    failed_jobs: Vec::new(),
                    cancelled_jobs: Vec::new(),
                    started_jobs: Vec::new(),
                    task_panel_logs: Vec::new(),
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

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec};
    use crate::state::{update_state, AppConfig, AppState, Transition};

    #[test]
    fn create_and_start_file_job_does_not_duplicate_started_log() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        let spec = JobSpec::new(JobKind::CountDown {
            duration_secs: 1,
            start_value: 1,
        });
        let job_id = spec.id;

        let dispatch_result = update_state(
            &mut state,
            Transition::CreateAndStartFileJob {
                spec,
                name: "Test Job".to_string(),
                description: "desc".to_string(),
            },
        );

        // Dispatching the job must not itself log "Started" — that is the sole
        // responsibility of Transition::JobStarted, fired once the worker pool
        // actually picks the job up (the only event path common to every
        // dispatch route, including the queued-job path).
        assert!(
            dispatch_result
                .task_panel_logs
                .iter()
                .all(|l| !l.contains("Started")),
            "dispatch must not log Started, got: {:?}",
            dispatch_result.task_panel_logs
        );

        let started_result = update_state(&mut state, Transition::JobStarted { job_id });
        let started_count = started_result
            .task_panel_logs
            .iter()
            .filter(|l| l.contains("Started"))
            .count();
        assert_eq!(
            started_count, 1,
            "expected exactly one Started log across both transitions, got: dispatch={:?} started={:?}",
            dispatch_result.task_panel_logs, started_result.task_panel_logs
        );
    }
}
