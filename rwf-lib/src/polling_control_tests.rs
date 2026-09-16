//! Phase 7.5 T4: user control and visibility of polling (D10e, D18, D21, D22).

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action};
    use crate::job::{JobSpec, OpResult, SuccessData};
    use crate::model::polling::StopReason;
    use crate::model::{ActivePane, Location};
    use crate::state::{update_state, AppState, StateUpdateResult, Transition};
    use crate::test_utils::{AppStateBuilder, FileEntryBuilder};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    const C: &str = r"C:\";

    /// Active left pane on drive C:, right pane on drive D:.
    fn two_drives() -> AppState {
        let mut state = AppStateBuilder::new()
            .left_path(r"C:\work")
            .right_path(r"D:\data")
            .active_pane(ActivePane::Left)
            .build();
        state.config.polling_interval_ms = 1000;
        state
    }

    fn act(state: &mut AppState, action: Action) -> StateUpdateResult {
        let mut result = StateUpdateResult::none();
        for transition in action_to_transitions(state, &action) {
            result.absorb(update_state(state, transition));
        }
        result
    }

    fn tick(state: &mut AppState, now: Instant) -> Vec<JobSpec> {
        let jobs = update_state(state, Transition::PollTick { now }).jobs_to_start;
        for job in &jobs {
            state.jobs.start_job(job.clone());
        }
        jobs
    }

    fn left_poll(jobs: &[JobSpec]) -> Option<&JobSpec> {
        jobs.iter()
            .find(|j| j.requesting_pane.map(|(_, s)| s) == Some(ActivePane::Left))
    }

    // --- Actions ---------------------------------------------------------------------

    #[test]
    fn stop_polling_stops_the_active_panes_drive_and_drops_its_running_poll() {
        let mut state = two_drives();
        let jobs = tick(&mut state, Instant::now());
        let poll = left_poll(&jobs).expect("left poll").clone();

        let result = act(&mut state, Action::StopPolling);

        assert_eq!(state.polling.drives[C].stopped, Some(StopReason::Manual));
        assert!(
            state.jobs.active[&poll.id].cancel_requested,
            "the poll is cancelled"
        );
        assert!(result
            .task_panel_logs
            .iter()
            .any(|l| l.contains("Polling stopped for C:\\")));

        // Its result, arriving anyway, is discarded.
        let late = FileEntryBuilder::new("late").path(r"C:\work\late").build();
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: poll.id,
                result: OpResult::Success(SuccessData::DirectoryRead(vec![late])),
            },
        );
        assert!(state.current_tab().left_pane.entries.is_empty());

        let later = tick(&mut state, Instant::now() + Duration::from_secs(60));
        assert!(left_poll(&later).is_none());
    }

    #[test]
    fn start_polling_clears_any_stop_resets_the_interval_and_polls_at_once() {
        let mut state = two_drives();
        let base = Duration::from_millis(1000);
        let drive = state.polling.drive_mut(C, base);
        drive.stopped = Some(StopReason::Auto);
        drive.interval = Duration::from_millis(4000);
        let key = (state.current_tab().id, ActivePane::Left);
        state
            .polling
            .next_due
            .insert(key, Instant::now() + Duration::from_secs(3600));

        let result = act(&mut state, Action::StartPolling);

        assert_eq!(state.polling.drives[C].stopped, None);
        assert_eq!(state.polling.drives[C].interval, base);
        assert!(result
            .task_panel_logs
            .iter()
            .any(|l| l.contains("Polling resumed for C:\\")));
        assert!(left_poll(&tick(&mut state, Instant::now())).is_some());
    }

    #[test]
    fn toggle_polling_stops_then_starts() {
        let mut state = two_drives();

        act(&mut state, Action::TogglePolling);
        assert_eq!(state.polling.drives[C].stopped, Some(StopReason::Manual));

        act(&mut state, Action::TogglePolling);
        assert_eq!(state.polling.drives[C].stopped, None);
    }

    #[test]
    fn the_actions_change_nothing_on_a_location_that_is_never_polled() {
        let mut state = two_drives();
        state.current_tab_mut().left_pane.current_location = Location::Archive {
            archive_path: Box::new(Location::Local(PathBuf::from(r"C:\a.zip"))),
            inner_path: PathBuf::new(),
        };

        for action in [
            Action::StopPolling,
            Action::StartPolling,
            Action::TogglePolling,
        ] {
            let result = act(&mut state, action);
            assert!(state.polling.drives.values().all(|d| d.stopped.is_none()));
            assert!(
                result
                    .task_panel_logs
                    .iter()
                    .any(|l| l.contains("Polling not available for this location")),
                "got {:?}",
                result.task_panel_logs
            );
        }
    }

    #[test]
    fn the_action_names_parse_from_keybindings_and_menus() {
        for (name, action) in [
            ("StopPolling", Action::StopPolling),
            ("StartPolling", Action::StartPolling),
            ("TogglePolling", Action::TogglePolling),
        ] {
            let parsed: Action =
                serde_json::from_value(serde_json::Value::String(name.to_string()))
                    .expect("parses");
            assert_eq!(parsed, action);
        }
    }

    /// D18: the sample custom functions ship a "Polling" submenu naming the three actions.
    #[test]
    fn the_sample_custom_functions_offer_a_polling_submenu() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../sample/custom_functions.json");
        let functions = crate::model::dialog::load_custom_functions(&path).expect("sample parses");
        let polling = functions
            .iter()
            .find(|f| f.name == "Polling")
            .expect("a Polling entry");
        let Some(crate::model::dialog::MenuContent::Items(items)) = &polling.menu else {
            panic!("Polling is an inline submenu, got {:?}", polling.menu);
        };
        let actions: Vec<&str> = items.iter().map(|i| i.action.as_str()).collect();

        assert_eq!(
            actions,
            vec!["StopPolling", "StartPolling", "TogglePolling"]
        );
    }

    // --- Indicators (D21) ------------------------------------------------------------

    #[test]
    fn a_stopped_drive_shows_no_poll_and_a_failing_one_shows_offline() {
        let mut state = two_drives();
        assert_eq!(state.poll_indicator(ActivePane::Left), None);

        state
            .polling
            .drive_mut(C, Duration::from_millis(1000))
            .failing = true;
        assert_eq!(state.poll_indicator(ActivePane::Left), Some("[offline]"));

        state
            .polling
            .drive_mut(C, Duration::from_millis(1000))
            .stopped = Some(StopReason::Manual);
        assert_eq!(state.poll_indicator(ActivePane::Left), Some("[no poll]"));
        assert_eq!(state.poll_indicator(ActivePane::Right), None);
    }

    #[test]
    fn no_indicator_when_polling_is_off_globally() {
        let mut state = two_drives();
        state
            .polling
            .drive_mut(C, Duration::from_millis(1000))
            .stopped = Some(StopReason::Auto);
        state.config.polling_interval_ms = 0;

        assert_eq!(state.poll_indicator(ActivePane::Left), None);
    }

    // --- Diagnostics (D22) -----------------------------------------------------------

    #[test]
    fn the_diagnostic_snapshot_records_each_drives_polling_state() {
        let mut state = two_drives();
        let base = Duration::from_millis(1000);
        state.polling.drive_mut(C, base).stopped = Some(StopReason::Auto);
        state.polling.drive_mut(r"D:\", base).failing = true;
        state.polling.drive_mut(r"D:\", base).interval = Duration::from_millis(2000);
        state.polling.drive_mut(r"D:\", base).last_poll = Some(Instant::now());
        state.polling.drive_mut(r"\\srv\share", base).stopped = Some(StopReason::Manual);
        state.polling.drive_mut("/", base);

        let snapshot = crate::diagnostics::DiagnosticStateSnapshot::capture(&state, 1, "manual");
        let json = serde_json::to_value(&snapshot).expect("serializes");
        let drives = json["polling"].as_array().expect("polling array");
        let find = |name: &str| {
            drives
                .iter()
                .find(|d| d["drive"] == name)
                .unwrap_or_else(|| panic!("{name} missing from {drives:?}"))
        };

        assert_eq!(find(C)["state"], "stopped-auto");
        assert_eq!(find(r"\\srv\share")["state"], "stopped-manual");
        assert_eq!(find(r"D:\")["state"], "failing");
        assert_eq!(find(r"D:\")["interval_ms"], 2000);
        assert!(find(r"D:\")["last_poll_ms_ago"].is_u64());
        assert_eq!(find("/")["state"], "active");
        assert!(find("/")["last_poll_ms_ago"].is_null());
    }
}
