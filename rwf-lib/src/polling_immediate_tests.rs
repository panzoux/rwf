//! Phase 7.5 T5: immediate polls (D17) — terminal focus gained, tab switch, and a pane
//! reappearing from behind a viewer are polled at once, bypassing the interval, under
//! the same skip rules as any tick.

#[cfg(test)]
mod tests {
    use crate::job::{JobSpec, OpResult, SuccessData};
    use crate::model::polling::StopReason;
    use crate::model::{ActivePane, Location, ViewerLayout, ViewerState};
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::AppStateBuilder;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn two_drives() -> AppState {
        let mut state = AppStateBuilder::new()
            .left_path(r"C:\work")
            .right_path(r"D:\data")
            .active_pane(ActivePane::Left)
            .build();
        state.config.polling_interval_ms = 1000;
        state
    }

    fn tick(state: &mut AppState) -> Vec<JobSpec> {
        let jobs = update_state(
            state,
            Transition::PollTick {
                now: Instant::now(),
            },
        )
        .jobs_to_start;
        for job in &jobs {
            state.jobs.start_job(job.clone());
        }
        jobs
    }

    /// Poll whatever is due and complete it, leaving every visible pane scheduled a full
    /// interval ahead.
    fn poll_and_settle(state: &mut AppState) -> Vec<JobSpec> {
        let jobs = tick(state);
        for job in &jobs {
            update_state(
                state,
                Transition::CompleteJob {
                    job_id: job.id,
                    result: OpResult::Success(SuccessData::DirectoryRead(Vec::new())),
                },
            );
        }
        jobs
    }

    fn sides(jobs: &[JobSpec]) -> Vec<ActivePane> {
        let mut sides: Vec<_> = jobs
            .iter()
            .filter_map(|j| j.requesting_pane.map(|(_, side)| side))
            .collect();
        sides.sort_by_key(|s| *s == ActivePane::Right);
        sides
    }

    #[test]
    fn nothing_is_due_again_before_the_interval_without_a_trigger() {
        let mut state = two_drives();
        poll_and_settle(&mut state);

        assert_ne!(state.poll_due_in(Instant::now()), Some(Duration::ZERO));
        assert!(tick(&mut state).is_empty());
    }

    #[test]
    fn focus_gained_polls_the_visible_panes_at_once() {
        let mut state = two_drives();
        poll_and_settle(&mut state);

        update_state(&mut state, Transition::TerminalFocusGained);

        assert_eq!(state.poll_due_in(Instant::now()), Some(Duration::ZERO));
        assert_eq!(
            sides(&tick(&mut state)),
            vec![ActivePane::Left, ActivePane::Right]
        );
    }

    #[test]
    fn focus_gained_keeps_the_skip_rules() {
        let mut state = two_drives();
        poll_and_settle(&mut state);
        state.current_tab_mut().left_pane.active_job_id = Some(crate::job::JobId::new());
        state
            .polling
            .drive_mut(r"D:\", Duration::from_millis(1000))
            .stopped = Some(StopReason::Manual);

        update_state(&mut state, Transition::TerminalFocusGained);

        assert!(tick(&mut state).is_empty());
        assert_eq!(
            state.poll_due_in(Instant::now()),
            None,
            "no busy loop over panes that cannot be polled"
        );
    }

    #[test]
    fn focus_gained_does_nothing_when_polling_is_off() {
        let mut state = two_drives();
        state.config.polling_interval_ms = 0;

        update_state(&mut state, Transition::TerminalFocusGained);

        assert!(tick(&mut state).is_empty());
        assert_eq!(state.poll_due_in(Instant::now()), None);
    }

    #[test]
    fn switching_back_to_a_tab_polls_its_panes_at_once() {
        let mut state = two_drives();
        poll_and_settle(&mut state);
        state.tabs.create_tab();
        state.tabs.switch_to_next();
        {
            let tab = state.current_tab_mut();
            tab.left_pane.current_location = Location::Local(PathBuf::from(r"E:\x"));
            tab.right_pane.current_location = Location::Local(PathBuf::from(r"F:\y"));
        }
        poll_and_settle(&mut state);

        state.tabs.switch_to_prev();

        assert_eq!(state.poll_due_in(Instant::now()), Some(Duration::ZERO));
        assert_eq!(
            sides(&tick(&mut state)),
            vec![ActivePane::Left, ActivePane::Right]
        );
    }

    /// A pane that appears while both poll workers are busy is polled as soon as one is
    /// free — and the loop does not spin while it waits.
    #[test]
    fn a_pane_appearing_while_the_poll_pool_is_full_waits_without_spinning() {
        let mut state = two_drives();
        poll_and_settle(&mut state);
        state.tabs.create_tab();
        state.tabs.switch_to_next();
        {
            let tab = state.current_tab_mut();
            tab.left_pane.current_location = Location::Local(PathBuf::from(r"E:\x"));
            tab.right_pane.current_location = Location::Local(PathBuf::from(r"F:\y"));
        }
        let busy = tick(&mut state);
        assert_eq!(busy.len(), 2);

        state.tabs.switch_to_prev();
        assert!(tick(&mut state).is_empty(), "pool full");
        assert_ne!(
            state.poll_due_in(Instant::now()),
            Some(Duration::ZERO),
            "no busy loop while the pool is full"
        );

        for job in &busy {
            update_state(
                &mut state,
                Transition::CompleteJob {
                    job_id: job.id,
                    result: OpResult::Cancelled,
                },
            );
        }
        assert_eq!(state.poll_due_in(Instant::now()), Some(Duration::ZERO));
        assert_eq!(
            sides(&tick(&mut state)),
            vec![ActivePane::Left, ActivePane::Right]
        );
    }

    #[test]
    fn closing_a_fullscreen_viewer_polls_both_panes_at_once() {
        let mut state = two_drives();
        poll_and_settle(&mut state);
        state.viewer = Some(ViewerState::new(Location::Local(PathBuf::from(
            r"C:\work\a",
        ))));
        state.ui.layout.viewer_layout = ViewerLayout::FullScreen;
        // The loop is woken once to notice the panes went out of sight, then sleeps.
        poll_and_settle(&mut state);
        assert_eq!(state.poll_due_in(Instant::now()), None);

        state.viewer = None;

        assert_eq!(
            sides(&tick(&mut state)),
            vec![ActivePane::Left, ActivePane::Right]
        );
    }

    #[test]
    fn the_pane_hidden_by_a_side_by_side_viewer_is_polled_when_it_reappears() {
        let mut state = two_drives();
        poll_and_settle(&mut state);
        state.viewer = Some(ViewerState::new(Location::Local(PathBuf::from(
            r"C:\work\a",
        ))));
        state.ui.layout.viewer_layout = ViewerLayout::SideBySide;
        state.ui.layout.viewer_anchor_pane = ActivePane::Left;
        poll_and_settle(&mut state);

        state.viewer = None;

        assert_eq!(sides(&tick(&mut state)), vec![ActivePane::Right]);
    }
}
