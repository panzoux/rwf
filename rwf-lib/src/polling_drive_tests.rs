//! Phase 7.5 T3: per-drive polling (D10a–d) — adaptive interval, automatic switch-off,
//! config reload. Timings are synthetic: a poll's `started_at` is set in the past.

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::polling::StopReason;
    use crate::model::{ActivePane, Location};
    use crate::state::{update_state, AppState, StateUpdateResult, Transition};
    use crate::test_utils::AppStateBuilder;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    const C: &str = r"C:\";
    const D: &str = r"D:\";

    /// Left pane on drive C:, right pane on drive D:.
    fn two_drives() -> AppState {
        let mut state = AppStateBuilder::new()
            .left_path(r"C:\work")
            .right_path(r"D:\data")
            .build();
        state.config.polling_interval_ms = 1000;
        state.config.polling_disable_after_ms = 30_000;
        state
    }

    fn tick_at(state: &mut AppState, now: Instant) -> StateUpdateResult {
        let result = update_state(state, Transition::PollTick { now });
        for job in &result.jobs_to_start {
            state.jobs.start_job(job.clone());
        }
        result
    }

    fn poll_for(result: &StateUpdateResult, side: ActivePane) -> Option<JobSpec> {
        result
            .jobs_to_start
            .iter()
            .find(|j| j.requesting_pane.map(|(_, s)| s) == Some(side))
            .cloned()
    }

    /// Pretend a worker picked `job` up `ago` before now.
    fn started(state: &mut AppState, job: &JobSpec, ago: Duration) {
        update_state(state, Transition::JobStarted { job_id: job.id });
        let key = state.polling.owns(job.id).expect("poll in flight");
        let poll = state.polling.in_flight.get_mut(&key).expect("in flight");
        // `Instant` counts from boot on Windows: a freshly started CI runner cannot
        // represent a time long ago, so keep `ago` small.
        poll.started_at = Some(
            Instant::now()
                .checked_sub(ago)
                .expect("`ago` fits in the machine's uptime"),
        );
    }

    fn complete(state: &mut AppState, job: &JobSpec, result: OpResult) -> StateUpdateResult {
        update_state(
            state,
            Transition::CompleteJob {
                job_id: job.id,
                result,
            },
        )
    }

    fn empty_listing() -> OpResult {
        OpResult::Success(SuccessData::DirectoryRead(Vec::new()))
    }

    fn interval(state: &AppState, drive: &str) -> Duration {
        state.polling.drives[drive].interval
    }

    /// One left-pane poll that took `took`, completed with `result`.
    fn left_poll_taking(state: &mut AppState, now: Instant, took: Duration, result: OpResult) {
        let ticked = tick_at(state, now);
        let job = poll_for(&ticked, ActivePane::Left).expect("left poll");
        started(state, &job, took);
        complete(state, &job, result);
    }

    #[test]
    fn a_worker_start_is_recorded_for_a_poll() {
        let mut state = two_drives();
        let job = poll_for(&tick_at(&mut state, Instant::now()), ActivePane::Left).expect("poll");

        update_state(&mut state, Transition::JobStarted { job_id: job.id });

        let key = state.polling.owns(job.id).expect("in flight");
        assert!(state.polling.in_flight[&key].started_at.is_some());
    }

    #[test]
    fn a_slow_poll_backs_off_only_its_own_drive() {
        let mut state = two_drives();

        left_poll_taking(
            &mut state,
            Instant::now(),
            Duration::from_millis(1500),
            empty_listing(),
        );

        assert_eq!(interval(&state, C), Duration::from_millis(2000));
        assert!(state
            .polling
            .drives
            .get(D)
            .is_none_or(|d| d.interval == Duration::from_millis(1000)));
    }

    #[test]
    fn the_next_poll_is_scheduled_from_the_backed_off_interval() {
        let mut state = two_drives();
        left_poll_taking(
            &mut state,
            Instant::now(),
            Duration::from_millis(1500),
            empty_listing(),
        );

        let soon = tick_at(&mut state, Instant::now() + Duration::from_millis(1100));
        assert!(
            poll_for(&soon, ActivePane::Left).is_none(),
            "still backing off"
        );
        let later = tick_at(&mut state, Instant::now() + Duration::from_millis(2100));
        assert!(poll_for(&later, ActivePane::Left).is_some());
    }

    #[test]
    fn a_failed_poll_backs_off_and_marks_the_drive_failing_until_a_success() {
        let mut state = two_drives();
        left_poll_taking(
            &mut state,
            Instant::now(),
            Duration::from_millis(5),
            OpResult::Failed("unreachable".to_string()),
        );
        assert_eq!(interval(&state, C), Duration::from_millis(2000));
        assert!(state.polling.drives[C].failing);

        left_poll_taking(
            &mut state,
            Instant::now() + Duration::from_secs(10),
            Duration::from_millis(5),
            empty_listing(),
        );
        assert!(!state.polling.drives[C].failing);
        assert_eq!(interval(&state, C), Duration::from_millis(1000));
    }

    #[test]
    fn a_hung_poll_switches_its_drive_off_on_the_next_tick() {
        let mut state = two_drives();
        state.config.polling_disable_after_ms = 2000;
        let ticked = tick_at(&mut state, Instant::now());
        let left = poll_for(&ticked, ActivePane::Left).expect("left");
        started(&mut state, &left, Duration::from_secs(3));

        let result = tick_at(&mut state, Instant::now());

        assert_eq!(state.polling.drives[C].stopped, Some(StopReason::Auto));
        let warnings: Vec<_> = result
            .task_panel_logs
            .iter()
            .filter(|l| l.contains("[WARN] Polling stopped for C:\\"))
            .collect();
        assert_eq!(warnings.len(), 1, "got {:?}", result.task_panel_logs);
        assert!(warnings[0].contains("StartPolling"));
        assert!(!state.polling.is_stopped(D), "only the slow drive stops");
    }

    #[test]
    fn a_switched_off_drive_is_not_polled_and_says_so_only_once() {
        let mut state = two_drives();
        state.config.polling_disable_after_ms = 2000;
        let ticked = tick_at(&mut state, Instant::now());
        let left = poll_for(&ticked, ActivePane::Left).expect("left");
        let right = poll_for(&ticked, ActivePane::Right).expect("right");
        started(&mut state, &left, Duration::from_secs(3));
        tick_at(&mut state, Instant::now());
        complete(&mut state, &left, empty_listing());
        complete(&mut state, &right, empty_listing());

        let later = tick_at(&mut state, Instant::now() + Duration::from_secs(60));

        assert!(poll_for(&later, ActivePane::Left).is_none());
        assert!(poll_for(&later, ActivePane::Right).is_some());
        assert!(later.task_panel_logs.is_empty());
    }

    #[test]
    fn a_poll_still_waiting_for_a_worker_does_not_trip_the_switch() {
        let mut state = two_drives();
        state.config.polling_disable_after_ms = 2000;
        tick_at(&mut state, Instant::now());

        tick_at(&mut state, Instant::now() + Duration::from_secs(10));

        assert!(!state.polling.is_stopped(C));
    }

    #[test]
    fn a_disable_after_of_zero_never_switches_off() {
        let mut state = two_drives();
        state.config.polling_disable_after_ms = 0;
        let ticked = tick_at(&mut state, Instant::now());
        let left = poll_for(&ticked, ActivePane::Left).expect("left");
        // Well past any limit a non-zero setting would impose in these tests (2 s).
        started(&mut state, &left, Duration::from_secs(5));

        tick_at(&mut state, Instant::now());
        complete(&mut state, &left, empty_listing());

        assert!(!state.polling.is_stopped(C));
    }

    #[test]
    fn a_poll_that_completes_past_the_limit_switches_off_too() {
        let mut state = two_drives();
        state.config.polling_disable_after_ms = 2000;

        left_poll_taking(
            &mut state,
            Instant::now(),
            Duration::from_secs(3),
            empty_listing(),
        );

        assert_eq!(state.polling.drives[C].stopped, Some(StopReason::Auto));
    }

    #[test]
    fn a_slow_user_read_does_not_switch_a_drive_off() {
        let mut state = two_drives();
        state.config.polling_disable_after_ms = 2000;
        let read = update_state(
            &mut state,
            Transition::Refresh {
                pane: ActivePane::Left,
            },
        )
        .jobs_to_start
        .remove(0);
        state.jobs.start_job(read.clone());
        update_state(&mut state, Transition::JobStarted { job_id: read.id });
        state
            .jobs
            .active
            .get_mut(&read.id)
            .expect("active")
            .started_at = Some(std::time::SystemTime::now() - Duration::from_secs(60));

        tick_at(&mut state, Instant::now());
        complete(&mut state, &read, empty_listing());

        assert!(!state.polling.is_stopped(C));
    }

    #[test]
    fn the_loop_is_woken_for_a_hung_poll_even_when_the_pool_is_full() {
        let mut state = two_drives();
        state.config.polling_disable_after_ms = 2000;
        let ticked = tick_at(&mut state, Instant::now());
        for side in [ActivePane::Left, ActivePane::Right] {
            let job = poll_for(&ticked, side).expect("poll");
            started(&mut state, &job, Duration::from_millis(500));
        }

        let due = state
            .poll_due_in(Instant::now())
            .expect("a deadline to wake for");

        assert!(
            due > Duration::ZERO && due <= Duration::from_millis(1500),
            "{due:?}"
        );
    }

    #[test]
    fn a_config_reload_resets_intervals_and_keeps_stopped_drives_stopped() {
        let mut state = two_drives();
        let base = Duration::from_millis(1000);
        state.polling.drive_mut(C, base).interval = Duration::from_millis(4000);
        state.polling.drive_mut(D, base).stopped = Some(StopReason::Auto);

        update_state(&mut state, Transition::ReloadConfig);

        // ReloadConfig reads the live config; compare against whatever base it loaded.
        if let Some(new_base) =
            crate::model::polling::PollingState::interval(state.config.polling_interval_ms)
        {
            assert_eq!(interval(&state, C), new_base);
        }
        assert_eq!(state.polling.drives[D].stopped, Some(StopReason::Auto));
    }

    #[test]
    fn the_mount_table_job_result_keys_unix_drives() {
        let mut state = two_drives();
        let job = JobSpec::new(JobKind::LoadMountTable);
        state.jobs.start_job(job.clone());

        complete(
            &mut state,
            &job,
            OpResult::Success(SuccessData::MountTable(vec![
                PathBuf::from("/"),
                PathBuf::from("/mnt/usb"),
            ])),
        );

        assert_eq!(
            state
                .polling
                .drive_of(&Location::Local(PathBuf::from("/mnt/usb/photos")))
                .as_deref(),
            Some("/mnt/usb")
        );
    }
}
