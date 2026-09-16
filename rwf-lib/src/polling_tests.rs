//! Phase 7.5 T2: background polling core (D1–D5, D9, D11, D12, D14, D15).
//!
//! The App loop only says "time has passed" (`Transition::PollTick`); everything about
//! which pane is read, and what happens to the result, is decided here in pure state.

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobOrigin, JobSpec, OpResult, SuccessData};
    use crate::model::polling::POLL_POOL_WORKERS;
    use crate::model::{ActivePane, FileEntry, Location, ViewerLayout, ViewerState};
    use crate::state::{update_state, AppState, StateUpdateResult, Transition};
    use crate::test_utils::{AppStateBuilder, FileEntryBuilder};
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime};

    fn local(path: &str) -> Location {
        Location::Local(PathBuf::from(path))
    }

    fn file_in(dir: &str, name: &str) -> FileEntry {
        FileEntryBuilder::new(name)
            .path(&format!("{dir}/{name}"))
            .modified(SystemTime::UNIX_EPOCH)
            .build()
    }

    /// Left pane at `/l` listing `a`, right pane at `/r` listing `b`.
    fn two_local_panes() -> AppState {
        let mut state = AppStateBuilder::new()
            .left_path("/l")
            .right_path("/r")
            .left_entries(vec![file_in("/l", "a")])
            .right_entries(vec![file_in("/r", "b")])
            .build();
        let tab = state.current_tab_mut();
        tab.left_pane.raw_entries = tab.left_pane.entries.clone();
        tab.right_pane.raw_entries = tab.right_pane.entries.clone();
        state.config.polling_interval_ms = 1000;
        state
    }

    fn pane(state: &AppState, side: ActivePane) -> &crate::model::PaneModel {
        let tab = state.current_tab();
        match side {
            ActivePane::Left => &tab.left_pane,
            ActivePane::Right => &tab.right_pane,
        }
    }

    /// A tick, with the resulting jobs registered as `App::submit_job` would.
    fn tick_at(state: &mut AppState, now: Instant) -> Vec<JobSpec> {
        let jobs = update_state(state, Transition::PollTick { now }).jobs_to_start;
        for job in &jobs {
            state.track_directory_read(job);
            state.jobs.start_job(job.clone());
        }
        jobs
    }

    fn tick(state: &mut AppState) -> Vec<JobSpec> {
        tick_at(state, Instant::now())
    }

    fn poll_for(jobs: &[JobSpec], side: ActivePane) -> JobSpec {
        jobs.iter()
            .find(|j| j.requesting_pane.map(|(_, s)| s) == Some(side))
            .cloned()
            .unwrap_or_else(|| panic!("no poll for {side:?} in {jobs:?}"))
    }

    fn complete(state: &mut AppState, job: &JobSpec, result: OpResult) -> StateUpdateResult {
        state.jobs.start_job(job.clone());
        update_state(
            state,
            Transition::CompleteJob {
                job_id: job.id,
                result,
            },
        )
    }

    fn listing(entries: Vec<FileEntry>) -> OpResult {
        OpResult::Success(SuccessData::DirectoryRead(entries))
    }

    // --- Which panes a tick reads ----------------------------------------------------

    #[test]
    fn a_tick_polls_both_visible_local_panes_without_claiming_them() {
        let mut state = two_local_panes();

        let jobs = tick(&mut state);

        assert_eq!(jobs.len(), 2);
        for side in [ActivePane::Left, ActivePane::Right] {
            let job = poll_for(&jobs, side);
            assert_eq!(job.origin, JobOrigin::Poll);
            assert!(matches!(job.kind, JobKind::ReadDirectory { .. }));
            assert_eq!(
                pane(&state, side).active_job_id,
                None,
                "a poll never owns a pane"
            );
            assert!(!pane(&state, side).is_loading);
        }
    }

    #[test]
    fn an_interval_of_zero_disables_polling() {
        let mut state = two_local_panes();
        state.config.polling_interval_ms = 0;

        assert!(tick(&mut state).is_empty());
        assert_eq!(state.poll_due_in(Instant::now()), None);
    }

    #[test]
    fn only_local_locations_are_polled() {
        let mut state = two_local_panes();
        state.current_tab_mut().right_pane.current_location = Location::Archive {
            archive_path: Box::new(local("/r/x.zip")),
            inner_path: PathBuf::new(),
        };

        let jobs = tick(&mut state);

        assert_eq!(jobs.len(), 1);
        poll_for(&jobs, ActivePane::Left);
    }

    #[test]
    fn a_pane_with_a_read_in_flight_is_skipped() {
        let mut state = two_local_panes();
        state.current_tab_mut().left_pane.active_job_id = Some(crate::job::JobId::new());

        let jobs = tick(&mut state);

        assert_eq!(jobs.len(), 1);
        poll_for(&jobs, ActivePane::Right);
    }

    #[test]
    fn a_pane_with_a_poll_in_flight_is_not_polled_again() {
        let mut state = two_local_panes();
        tick(&mut state);

        assert!(tick(&mut state).is_empty());
    }

    #[test]
    fn the_next_poll_waits_an_interval_from_the_previous_completion() {
        let mut state = two_local_panes();
        let jobs = tick(&mut state);
        for job in &jobs {
            let unchanged = pane(&state, job.requesting_pane.expect("pane").1)
                .raw_entries
                .clone();
            complete(&mut state, job, listing(unchanged));
        }

        assert!(tick(&mut state).is_empty(), "not due yet");
        let later = Instant::now() + Duration::from_millis(1001);
        assert_eq!(tick_at(&mut state, later).len(), 2);
    }

    #[test]
    fn an_interval_below_250_ms_is_raised_to_250() {
        let mut state = two_local_panes();
        state.config.polling_interval_ms = 10;
        let jobs = tick(&mut state);
        for job in &jobs {
            complete(&mut state, job, listing(Vec::new()));
        }

        assert!(tick_at(&mut state, Instant::now() + Duration::from_millis(100)).is_empty());
        assert_eq!(
            tick_at(&mut state, Instant::now() + Duration::from_millis(260)).len(),
            2
        );
    }

    #[test]
    fn a_fullscreen_viewer_pauses_polling() {
        let mut state = two_local_panes();
        state.viewer = Some(ViewerState::new(local("/l/a")));
        state.ui.layout.viewer_layout = ViewerLayout::FullScreen;

        assert!(tick(&mut state).is_empty());
    }

    #[test]
    fn a_side_by_side_viewer_polls_only_its_anchor_pane() {
        let mut state = two_local_panes();
        state.viewer = Some(ViewerState::new(local("/l/a")));
        state.ui.layout.viewer_layout = ViewerLayout::SideBySide;
        state.ui.layout.viewer_anchor_pane = ActivePane::Right;

        let jobs = tick(&mut state);

        assert_eq!(jobs.len(), 1);
        poll_for(&jobs, ActivePane::Right);
    }

    #[test]
    fn the_active_pane_in_range_marking_is_not_polled() {
        let mut state = two_local_panes();
        state.ui.active_pane = ActivePane::Left;
        state.ui.range_marking_start = Some(0);

        let jobs = tick(&mut state);

        assert_eq!(jobs.len(), 1);
        poll_for(&jobs, ActivePane::Right);
    }

    #[test]
    fn the_active_pane_in_leap_mode_is_not_polled() {
        let mut state = two_local_panes();
        state.ui.active_pane = ActivePane::Right;
        state.leap = Some(crate::model::LeapState::new(PathBuf::from("/r"), 0));

        let jobs = tick(&mut state);

        assert_eq!(jobs.len(), 1);
        poll_for(&jobs, ActivePane::Left);
    }

    #[test]
    fn background_tabs_are_not_polled() {
        let mut state = two_local_panes();
        let first = state.current_tab().id;
        state.tabs.create_tab();
        state.tabs.switch_to_next();
        let second = state.current_tab().id;
        assert_ne!(first, second);

        let jobs = tick(&mut state);

        assert!(jobs
            .iter()
            .all(|j| j.requesting_pane.map(|(t, _)| t) == Some(second)));
    }

    #[test]
    fn a_full_poll_pool_skips_the_tick() {
        let mut state = two_local_panes();
        let hung = tick(&mut state);
        assert_eq!(hung.len(), POLL_POOL_WORKERS);
        state.tabs.create_tab();
        state.tabs.switch_to_next();
        state.current_tab_mut().left_pane.current_location = local("/other");

        assert!(tick(&mut state).is_empty());
    }

    #[test]
    fn polls_are_not_background_jobs() {
        let mut state = two_local_panes();
        let tab_id = state.current_tab().id;

        tick(&mut state);

        assert_eq!(state.background_jobs.get_active_job_count(tab_id), 0);
    }

    #[test]
    fn poll_due_in_reports_now_for_an_unpolled_pane_and_nothing_while_all_are_in_flight() {
        let mut state = two_local_panes();
        let now = Instant::now();
        assert_eq!(state.poll_due_in(now), Some(Duration::ZERO));

        tick_at(&mut state, now);

        assert_eq!(state.poll_due_in(now), None);
    }

    // --- Completion ------------------------------------------------------------------

    #[test]
    fn a_poll_that_finds_a_new_file_updates_the_pane() {
        let mut state = two_local_panes();
        let job = poll_for(&tick(&mut state), ActivePane::Left);

        let result = complete(
            &mut state,
            &job,
            listing(vec![file_in("/l", "a"), file_in("/l", "new")]),
        );

        assert!(result.ui_changed);
        assert_eq!(pane(&state, ActivePane::Left).entries.len(), 2);
        assert!(result.task_panel_logs.is_empty(), "polls are silent");
    }

    #[test]
    fn a_poll_read_before_an_in_memory_change_is_discarded() {
        let mut state = two_local_panes();
        let job = poll_for(&tick(&mut state), ActivePane::Left);
        state.current_tab_mut().left_pane.mark_listing_changed();

        complete(&mut state, &job, listing(Vec::new()));

        assert_eq!(pane(&state, ActivePane::Left).entries.len(), 1);
    }

    #[test]
    fn a_poll_is_discarded_when_a_user_read_started_meanwhile() {
        let mut state = two_local_panes();
        let job = poll_for(&tick(&mut state), ActivePane::Left);
        state.current_tab_mut().left_pane.active_job_id = Some(crate::job::JobId::new());

        complete(&mut state, &job, listing(Vec::new()));

        assert_eq!(pane(&state, ActivePane::Left).entries.len(), 1);
    }

    #[test]
    fn a_poll_is_discarded_when_the_pane_moved_meanwhile() {
        let mut state = two_local_panes();
        let job = poll_for(&tick(&mut state), ActivePane::Left);
        state.current_tab_mut().left_pane.current_location = local("/elsewhere");

        complete(&mut state, &job, listing(Vec::new()));

        assert_eq!(pane(&state, ActivePane::Left).entries.len(), 1);
    }

    // --- Failure (D9) ----------------------------------------------------------------

    /// Fail the left poll and return the fallback search it asks for, if any.
    fn fail_left_poll(state: &mut AppState, now: Instant) -> (StateUpdateResult, Option<JobSpec>) {
        let jobs = tick_at(state, now);
        let job = poll_for(&jobs, ActivePane::Left);
        let result = complete(state, &job, OpResult::Failed("unreachable".to_string()));
        let fallback = result
            .jobs_to_start
            .iter()
            .find(|j| matches!(j.kind, JobKind::ResolveFallbackPath { .. }))
            .cloned();
        (result, fallback)
    }

    fn resolve(
        state: &mut AppState,
        fallback: &JobSpec,
        found: Option<Location>,
    ) -> StateUpdateResult {
        complete(
            state,
            fallback,
            OpResult::Success(SuccessData::FallbackPath(found)),
        )
    }

    #[test]
    fn a_deleted_directory_with_a_readable_parent_navigates_to_the_parent() {
        let mut state = two_local_panes();
        state.current_tab_mut().left_pane.current_location = local("/l/sub");

        let (failed, fallback) = fail_left_poll(&mut state, Instant::now());
        let fallback = fallback.expect("a failed poll looks for the directory");
        assert_eq!(fallback.origin, JobOrigin::Poll);
        assert!(state.dialogs.current().is_none());
        assert!(failed.task_panel_logs.is_empty());

        resolve(&mut state, &fallback, Some(local("/l")));

        assert_eq!(pane(&state, ActivePane::Left).current_location, local("/l"));
        assert!(state.dialogs.current().is_none());
    }

    #[test]
    fn an_unreachable_directory_stays_put_with_one_warning_and_one_recovery_line() {
        let mut state = two_local_panes();

        let (_, fallback) = fail_left_poll(&mut state, Instant::now());
        let resolved = resolve(&mut state, &fallback.expect("fallback"), None);
        assert_eq!(pane(&state, ActivePane::Left).current_location, local("/l"));
        assert!(state.dialogs.current().is_none(), "no modal for a poll");
        assert_eq!(resolved.task_panel_logs.len(), 1);
        assert!(resolved.task_panel_logs[0].contains("[WARN]"));

        // Still failing: no second search, no second line.
        let later = Instant::now() + Duration::from_secs(60);
        let (again, fallback) = fail_left_poll(&mut state, later);
        assert!(fallback.is_none());
        assert!(again.task_panel_logs.is_empty());

        // Recovery.
        let later = Instant::now() + Duration::from_secs(120);
        let job = poll_for(&tick_at(&mut state, later), ActivePane::Left);
        let recovered = complete(&mut state, &job, listing(vec![file_in("/l", "a")]));
        assert_eq!(recovered.task_panel_logs.len(), 1);
        assert!(recovered.task_panel_logs[0].contains("[OK]"));
    }

    #[test]
    fn a_surviving_grandparent_is_not_a_readable_parent() {
        let mut state = two_local_panes();
        state.current_tab_mut().left_pane.current_location = local("/l/sub/deeper");

        let (_, fallback) = fail_left_poll(&mut state, Instant::now());
        resolve(&mut state, &fallback.expect("fallback"), Some(local("/l")));

        assert_eq!(
            pane(&state, ActivePane::Left).current_location,
            local("/l/sub/deeper")
        );
        assert!(state.dialogs.current().is_none());
    }

    // --- End to end with a real directory ---------------------------------------------

    #[tokio::test]
    async fn a_file_created_outside_rwf_appears_after_a_tick() {
        let (mut state, left_dir, _right_dir) = crate::test_utils::state_with_temp_dirs();
        state.config.polling_interval_ms = 1000;
        let backend = std::sync::Arc::new(crate::backend::LocalFilesystemBackend::new());
        let archive = std::sync::Arc::new(crate::backend::MockArchiveHandler);
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let executor = crate::job::JobExecutor::new(backend, archive, event_tx);

        async fn run_left_poll<B, A>(
            executor: &crate::job::JobExecutor<B, A>,
            rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::worker_pool::JobEvent>,
            state: &mut AppState,
            now: Instant,
        ) where
            B: crate::backend::FilesystemBackend,
            A: crate::backend::ArchiveHandler,
        {
            let jobs = tick_at(state, now);
            let job = poll_for(&jobs, ActivePane::Left);
            executor.execute(job.clone()).await;
            let result = loop {
                match rx.recv().await.expect("event") {
                    crate::worker_pool::JobEvent::Completed(_, data) => {
                        break OpResult::Success(data)
                    }
                    crate::worker_pool::JobEvent::Failed(_, e) => break OpResult::Failed(e),
                    _ => continue,
                }
            };
            update_state(
                state,
                Transition::CompleteJob {
                    job_id: job.id,
                    result,
                },
            );
            // The right pane's poll is irrelevant here; drop it so it doesn't block.
            if let Some(right) = jobs
                .iter()
                .find(|j| j.requesting_pane.map(|(_, s)| s) == Some(ActivePane::Right))
            {
                update_state(
                    state,
                    Transition::CompleteJob {
                        job_id: right.id,
                        result: OpResult::Cancelled,
                    },
                );
            }
        }

        run_left_poll(&executor, &mut event_rx, &mut state, Instant::now()).await;
        assert!(pane(&state, ActivePane::Left).entries.is_empty());

        std::fs::write(left_dir.path().join("external.txt"), b"x").expect("write");
        let later = Instant::now() + Duration::from_millis(1001);
        run_left_poll(&executor, &mut event_rx, &mut state, later).await;

        let names: Vec<_> = pane(&state, ActivePane::Left)
            .entries
            .iter()
            .map(|e| e.name.clone())
            .collect();
        assert_eq!(names, vec!["external.txt".to_string()]);
    }
}
