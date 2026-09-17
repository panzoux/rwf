//! Phase 7.5: poll duration measurement — how fast each drive lists a directory.
//!
//! Timed by the worker (`JobEvent::Elapsed`), because the App loop sees a fast poll's start
//! and completion in the same batch and would measure it as 0 ms. The per-drive figures feed
//! the back-off (D10b), the F12 snapshot, and a later user-facing display.

#[cfg(test)]
mod tests {
    use crate::job::{JobOrigin, JobSpec, OpResult, SuccessData};
    use crate::model::{ActivePane, Location};
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::AppStateBuilder;
    use crate::worker_pool::JobEvent;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    const C: &str = r"C:\";

    fn two_drives() -> AppState {
        let mut state = AppStateBuilder::new()
            .left_path(r"C:\work")
            .right_path(r"D:\data")
            .active_pane(ActivePane::Left)
            .build();
        state.config.polling_interval_ms = 1000;
        state
    }

    fn left_poll(state: &mut AppState, now: Instant) -> JobSpec {
        let jobs = update_state(state, Transition::PollTick { now }).jobs_to_start;
        for job in &jobs {
            state.jobs.start_job(job.clone());
        }
        jobs.into_iter()
            .find(|j| j.requesting_pane.map(|(_, s)| s) == Some(ActivePane::Left))
            .expect("left poll")
    }

    /// A left-pane poll whose worker reports it took `elapsed`, finishing with `result`.
    fn timed_poll(state: &mut AppState, now: Instant, elapsed: Duration, result: OpResult) {
        let job = left_poll(state, now);
        update_state(state, Transition::JobStarted { job_id: job.id });
        update_state(
            state,
            Transition::JobElapsed {
                job_id: job.id,
                elapsed,
            },
        );
        update_state(
            state,
            Transition::CompleteJob {
                job_id: job.id,
                result,
            },
        );
    }

    fn listing() -> OpResult {
        OpResult::Success(SuccessData::DirectoryRead(Vec::new()))
    }

    fn later(secs: u64) -> Instant {
        Instant::now() + Duration::from_secs(secs)
    }

    #[test]
    fn the_worker_measured_duration_drives_the_back_off() {
        let mut state = two_drives();

        // Processed back to back, as the loop would for a batch: loop timing would say ~0.
        timed_poll(
            &mut state,
            Instant::now(),
            Duration::from_millis(1500),
            listing(),
        );

        assert_eq!(
            state.polling.drives[C].interval,
            Duration::from_millis(2000)
        );
    }

    #[test]
    fn each_drive_keeps_last_average_and_slowest_listing_time() {
        let mut state = two_drives();

        timed_poll(&mut state, later(0), Duration::from_millis(100), listing());
        timed_poll(&mut state, later(10), Duration::from_millis(300), listing());
        timed_poll(&mut state, later(20), Duration::from_millis(200), listing());

        let timing = state.polling.drives[C].timing.expect("timed");
        assert_eq!(timing.count, 3);
        assert_eq!(timing.last, Duration::from_millis(200));
        assert_eq!(timing.slowest, Duration::from_millis(300));
        // Exponentially weighted, 1/4 per sample: 100 → 150 → 162.5 ms.
        assert_eq!(timing.average, Duration::from_micros(162_500));
    }

    #[test]
    fn a_failed_poll_is_not_a_listing_time() {
        let mut state = two_drives();

        timed_poll(
            &mut state,
            later(0),
            Duration::from_secs(20),
            OpResult::Failed("unreachable".to_string()),
        );

        assert!(state.polling.drives[C].timing.is_none());
        assert_eq!(
            state.polling.drives[C].interval,
            Duration::from_millis(2000)
        );
    }

    #[test]
    fn the_timing_of_a_visible_panes_drive_is_available_for_display() {
        let mut state = two_drives();
        assert!(state.poll_timing(ActivePane::Left).is_none());

        timed_poll(&mut state, later(0), Duration::from_millis(42), listing());

        assert_eq!(
            state.poll_timing(ActivePane::Left).map(|t| t.last),
            Some(Duration::from_millis(42))
        );
        assert!(state.poll_timing(ActivePane::Right).is_none());
    }

    #[test]
    fn a_cancelled_poll_frees_its_slot() {
        let mut state = two_drives();
        let job = left_poll(&mut state, Instant::now());

        update_state(&mut state, Transition::AcknowledgeCancel { job_id: job.id });

        assert!(state.polling.owns(job.id).is_none());
    }

    #[test]
    fn the_diagnostic_snapshot_includes_listing_times() {
        let mut state = two_drives();
        timed_poll(&mut state, later(0), Duration::from_millis(120), listing());
        timed_poll(&mut state, later(10), Duration::from_millis(80), listing());

        let snapshot = crate::diagnostics::DiagnosticStateSnapshot::capture(&state, 1, "manual");
        let json = serde_json::to_value(&snapshot).expect("serializes");
        let drive = json["polling"]
            .as_array()
            .and_then(|d| d.iter().find(|d| d["drive"] == C))
            .expect("drive C");

        assert_eq!(drive["polls"], 2);
        assert_eq!(drive["last_poll_duration_ms"], 80);
        assert_eq!(drive["average_poll_duration_ms"], 110);
        assert_eq!(drive["slowest_poll_duration_ms"], 120);
    }

    #[tokio::test]
    async fn the_worker_reports_elapsed_for_polls_only() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let backend = std::sync::Arc::new(crate::backend::LocalFilesystemBackend::new());
        let archive = std::sync::Arc::new(crate::backend::MockArchiveHandler);
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let executor = crate::job::JobExecutor::new(backend, archive, event_tx);
        let read = || {
            JobSpec::new(crate::job::JobKind::ReadDirectory {
                location: Location::Local(PathBuf::from(dir.path())),
            })
        };

        let mut kinds = Vec::new();
        for spec in [read().with_origin(JobOrigin::Poll), read()] {
            executor.execute(spec).await;
            let mut events = Vec::new();
            while let Ok(event) = event_rx.try_recv() {
                events.push(match event {
                    JobEvent::Started(_) => "Started",
                    JobEvent::Elapsed(_, _) => "Elapsed",
                    JobEvent::Completed(_, _) => "Completed",
                    _ => "other",
                });
            }
            kinds.push(events);
        }

        assert_eq!(kinds[0], vec!["Started", "Elapsed", "Completed"]);
        assert_eq!(kinds[1], vec!["Started", "Completed"]);
    }
}
