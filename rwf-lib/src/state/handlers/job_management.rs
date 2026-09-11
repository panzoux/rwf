use crate::state::{AppState, StateUpdateResult, Transition};
use tracing::debug;

impl AppState {
    /// The 1-based position of the tab with this id — the number the user sees.
    ///
    /// Background jobs and `JobSpec::requesting_pane` key tabs by **id**, which stays
    /// put when a tab to the left closes; a position does not. Convert only at the
    /// point of display.
    pub fn tab_number(&self, tab_id: usize) -> Option<usize> {
        self.tabs
            .tabs
            .iter()
            .position(|t| t.id == tab_id)
            .map(|p| p + 1)
    }

    /// [`tab_number`](Self::tab_number) for a log line; `?` once the tab has closed.
    pub(crate) fn tab_label(&self, tab_id: usize) -> String {
        self.tab_number(tab_id)
            .map_or_else(|| "?".to_string(), |n| n.to_string())
    }

    /// Register a pane's `ReadDirectory` as a quiet background job, so its tab shows a
    /// spinner and the job manager lists it while it runs (Phase 7.22 §2).
    ///
    /// Reads never registered before, which is why a tab stuck on a dead share showed
    /// no spinner and the task panel said "No active tasks" with two reads running.
    /// Called from `App::submit_job`, the one path every job takes to the pool.
    pub fn track_directory_read(&mut self, spec: &crate::job::JobSpec) {
        let crate::job::JobKind::ReadDirectory { location } = &spec.kind else {
            return;
        };
        if self.background_jobs.get_job(spec.id).is_some() {
            return;
        }
        // A read with no requesting pane refreshes every pane on that path; the active
        // tab is where the user is looking.
        let tab_id = spec
            .requesting_pane
            .map_or_else(|| self.current_tab().id, |(tab_id, _)| tab_id);
        let name = format!("Read {}", location.display_path());
        self.background_jobs.start_quiet_job(
            name.clone(),
            name,
            tab_id,
            String::new(),
            spec.clone(),
        );
    }

    /// Quiet jobs that have been running for `QUIET_JOB_ANNOUNCE_AFTER` without being
    /// announced. Timed from `Job::started_at` — when a worker actually picked the job
    /// up — so a read waiting for a free worker is not called slow. Cheap enough for
    /// every loop iteration.
    pub fn quiet_jobs_due(&self) -> Vec<crate::job::JobId> {
        let now = std::time::SystemTime::now();
        self.background_jobs
            .get_active_jobs()
            .filter(|j| j.quiet && !j.announced)
            .filter(|j| {
                self.jobs
                    .active
                    .get(&j.id.uuid)
                    .and_then(|job| job.started_at)
                    .and_then(|started| now.duration_since(started).ok())
                    .is_some_and(|ran| {
                        ran >= crate::job::background_job_manager::QUIET_JOB_ANNOUNCE_AFTER
                    })
            })
            .map(|j| j.id.uuid)
            .collect()
    }

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
                let tab_id = self.current_tab().id;

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
                let tab_id = self.current_tab().id;

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
