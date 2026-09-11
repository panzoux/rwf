//! Phase 7.22 §2: a pane's directory read is visible while it runs.
//!
//! Diagnostic bundle `20260910-203646` caught two reads running on a dead share with
//! `jobs.background = []`: no tab spinner, and a task panel reading "No active tasks".
//! Reads now register as *quiet* background jobs — they spin their tab and list in the
//! job manager at once, but reach the task panel only when slow or failed.

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::{ActivePane, Location};
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::test_state;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn read_for(tab_id: usize) -> JobSpec {
        JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/mnt/share")),
        })
        .with_requesting_pane(tab_id, ActivePane::Right)
    }

    /// What `App::submit_job` does for every job bound for the pool.
    fn submit(state: &mut AppState, spec: &JobSpec) {
        state.track_directory_read(spec);
        state.jobs.start_job(spec.clone());
    }

    /// A worker picked the job up this long ago.
    fn started_ago(state: &mut AppState, spec: &JobSpec, ago: Duration) {
        update_state(state, Transition::JobStarted { job_id: spec.id });
        state
            .jobs
            .active
            .get_mut(&spec.id)
            .expect("active job")
            .started_at = Some(SystemTime::now() - ago);
    }

    #[test]
    fn a_pane_read_spins_the_tab_that_asked_not_the_active_one() {
        let mut state = test_state();
        state.tabs.create_tab();
        state.tabs.create_tab();
        let third = state.tabs.tabs[2].id;

        submit(&mut state, &read_for(third));

        assert_eq!(state.background_jobs.get_active_job_count(third), 1);
        assert_eq!(
            state
                .background_jobs
                .get_active_job_count(state.tabs.tabs[0].id),
            0
        );
    }

    /// Keyed by id, the spinner follows its tab when positions shift — keyed by
    /// index it would jump to whichever tab slid into the old slot (the bug class of
    /// `498c710`).
    #[test]
    fn the_spinner_stays_with_its_tab_when_a_tab_to_the_left_closes() {
        let mut state = test_state();
        state.tabs.create_tab();
        state.tabs.create_tab();
        let last = state.tabs.tabs[2].id;
        submit(&mut state, &read_for(last));

        update_state(&mut state, Transition::CloseTab { index: 0 });

        assert_eq!(state.background_jobs.get_active_job_count(last), 1);
        assert_eq!(state.tab_number(last), Some(2));
    }

    #[test]
    fn a_fast_read_writes_nothing_to_the_task_panel() {
        let mut state = test_state();
        let spec = read_for(state.tabs.tabs[0].id);
        submit(&mut state, &spec);

        let started = update_state(&mut state, Transition::JobStarted { job_id: spec.id });
        assert!(
            started.task_panel_logs.is_empty(),
            "{:?}",
            started.task_panel_logs
        );

        let done = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: spec.id,
                result: OpResult::Success(SuccessData::DirectoryRead(Vec::new())),
            },
        );
        assert!(
            done.task_panel_logs.is_empty(),
            "{:?}",
            done.task_panel_logs
        );
        assert!(state.background_jobs.get_active_jobs().next().is_none());
    }

    #[test]
    fn a_failed_read_is_always_logged() {
        let mut state = test_state();
        let spec = read_for(state.tabs.tabs[0].id);
        submit(&mut state, &spec);
        update_state(&mut state, Transition::JobStarted { job_id: spec.id });

        let done = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: spec.id,
                result: OpResult::Failed("network name not resolved".to_string()),
            },
        );

        assert!(
            done.task_panel_logs.iter().any(|l| l.contains("[FAIL]")
                && l.contains("Read /mnt/share")
                && l.contains("[Tab 1]")),
            "{:?}",
            done.task_panel_logs
        );
    }

    #[test]
    fn a_slow_read_is_announced_once_and_then_logs_its_outcome() {
        let mut state = test_state();
        let spec = read_for(state.tabs.tabs[0].id);
        submit(&mut state, &spec);
        started_ago(&mut state, &spec, Duration::from_secs(2));

        let due = state.quiet_jobs_due();
        assert_eq!(due, vec![spec.id]);
        let announced = update_state(&mut state, Transition::AnnounceQuietJobs { job_ids: due });
        assert_eq!(announced.task_panel_logs.len(), 1);
        assert!(announced.task_panel_logs[0].contains("Read /mnt/share"));
        assert!(
            state.quiet_jobs_due().is_empty(),
            "announced once, not on every loop iteration"
        );

        let done = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: spec.id,
                result: OpResult::Success(SuccessData::DirectoryRead(Vec::new())),
            },
        );
        assert!(
            done.task_panel_logs.iter().any(|l| l.contains("[OK]")),
            "an announced read reports how it ended: {:?}",
            done.task_panel_logs
        );
    }

    #[test]
    fn a_read_inside_the_quiet_period_is_not_due() {
        let mut state = test_state();
        let spec = read_for(state.tabs.tabs[0].id);
        submit(&mut state, &spec);
        started_ago(&mut state, &spec, Duration::from_millis(10));
        assert!(state.quiet_jobs_due().is_empty());
    }

    /// Timed from when a worker picked it up: a read still waiting for a free worker
    /// has not been slow yet.
    #[test]
    fn a_read_no_worker_has_picked_up_is_not_due() {
        let mut state = test_state();
        let spec = read_for(state.tabs.tabs[0].id);
        submit(&mut state, &spec);
        assert!(state.quiet_jobs_due().is_empty());
    }

    /// Navigation cancels the read it supersedes. Without this, every such read would
    /// spin its tab forever — the background list never heard about the cancel.
    #[test]
    fn a_cancelled_read_stops_spinning_its_tab() {
        let mut state = test_state();
        let tab = state.tabs.tabs[0].id;
        let spec = read_for(tab);
        submit(&mut state, &spec);
        update_state(&mut state, Transition::JobStarted { job_id: spec.id });

        update_state(
            &mut state,
            Transition::AcknowledgeCancel { job_id: spec.id },
        );

        assert_eq!(state.background_jobs.get_active_job_count(tab), 0);
    }

    #[test]
    fn registering_the_same_read_twice_is_harmless() {
        let mut state = test_state();
        let tab = state.tabs.tabs[0].id;
        let spec = read_for(tab);
        state.track_directory_read(&spec);
        state.track_directory_read(&spec);
        assert_eq!(state.background_jobs.get_active_job_count(tab), 1);
    }

    #[test]
    fn only_directory_reads_are_tracked() {
        let mut state = test_state();
        let spec = JobSpec::new(JobKind::Delete {
            targets: vec![Location::Local(PathBuf::from("/tmp/x"))],
        });
        state.track_directory_read(&spec);
        assert!(state.background_jobs.get_job(spec.id).is_none());
    }
}
