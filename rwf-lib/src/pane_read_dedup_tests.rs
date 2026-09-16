//! Phase 7.5 T0 (D13): a second read of the same pane while the first is still running.
//!
//! `JobManager::start_job` used to drop a ReadDirectory whose kind and pane matched an
//! active one, while `App::submit_job` still handed it to the pool and the transition
//! had already moved `active_job_id` to it. Its completion then found no spec and never
//! cleared the pane, and the first read's completion was discarded as stale — the pane
//! stayed `is_loading` with an `active_job_id` nothing would ever clear. Background
//! polling skips panes with a read in flight, so this would silently stop polling.

#[cfg(test)]
mod tests {
    use crate::job::{JobSpec, OpResult, SuccessData};
    use crate::model::ActivePane;
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::test_state;

    /// What `App::submit_job` does for every job bound for the pool.
    fn submit(state: &mut AppState, spec: &JobSpec) {
        state.track_directory_read(spec);
        state.jobs.start_job(spec.clone());
    }

    fn refresh_left(state: &mut AppState) -> JobSpec {
        let update = update_state(
            state,
            Transition::Refresh {
                pane: ActivePane::Left,
            },
        );
        let spec = update
            .jobs_to_start
            .into_iter()
            .next()
            .expect("refresh starts a read");
        submit(state, &spec);
        spec
    }

    fn complete_empty(state: &mut AppState, spec: &JobSpec) {
        update_state(
            state,
            Transition::CompleteJob {
                job_id: spec.id,
                result: OpResult::Success(SuccessData::DirectoryRead(Vec::new())),
            },
        );
    }

    fn assert_pane_settled(state: &AppState) {
        let pane = &state.current_tab().left_pane;
        assert_eq!(pane.active_job_id, None, "no read left in flight");
        assert!(!pane.is_loading, "pane is not stuck loading");
    }

    #[test]
    fn a_refresh_during_a_read_settles_when_both_finish_in_order() {
        let mut state = test_state();
        let first = refresh_left(&mut state);
        let second = refresh_left(&mut state);

        complete_empty(&mut state, &first);
        complete_empty(&mut state, &second);

        assert_pane_settled(&state);
    }

    #[test]
    fn a_refresh_during_a_read_settles_when_the_second_finishes_first() {
        let mut state = test_state();
        let first = refresh_left(&mut state);
        let second = refresh_left(&mut state);

        complete_empty(&mut state, &second);
        complete_empty(&mut state, &first);

        assert_pane_settled(&state);
    }

    #[test]
    fn the_superseding_read_is_tracked_by_the_job_manager() {
        let mut state = test_state();
        let _first = refresh_left(&mut state);
        let second = refresh_left(&mut state);

        assert!(state.jobs.active.contains_key(&second.id));
    }
}
