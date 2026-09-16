//! Phase 7.5 T1: refresh correctness (D7, D8a–b, D12).
//!
//! Background polling turns every refresh into a routine event, so a refresh must not
//! move the cursor off its file, wipe calculated directory sizes, or revert an
//! in-memory rename. These hold for F5 and post-operation refreshes as much as polls.

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action};
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::{ActivePane, FileEntry, Location, OperationRecord, UndoAvailability};
    use crate::state::{update_state, AppState, StateUpdateResult, Transition};
    use crate::test_utils::{AppStateBuilder, FileEntryBuilder};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn file(name: &str) -> FileEntry {
        FileEntryBuilder::new(name)
            .modified(SystemTime::UNIX_EPOCH)
            .build()
    }

    fn dir(name: &str) -> FileEntry {
        FileEntryBuilder::new(name)
            .dir(true)
            .modified(SystemTime::UNIX_EPOCH)
            .build()
    }

    fn files(names: &[&str]) -> Vec<FileEntry> {
        names.iter().map(|n| file(n)).collect()
    }

    /// Left pane at `/test`, showing `listing` with the cursor on `cursor`.
    fn state_showing(listing: Vec<FileEntry>, cursor: usize) -> AppState {
        let mut state = AppStateBuilder::new()
            .left_path("/test")
            .left_entries(listing.clone())
            .left_cursor(cursor)
            .active_pane(ActivePane::Left)
            .build();
        state.current_tab_mut().left_pane.raw_entries = listing;
        state
    }

    fn left(state: &AppState) -> &crate::model::PaneModel {
        &state.current_tab().left_pane
    }

    /// F5 on the left pane, and the read returns `listing`.
    fn refresh_returns(state: &mut AppState, listing: Vec<FileEntry>) -> StateUpdateResult {
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
        state.track_directory_read(&spec);
        state.jobs.start_job(spec.clone());
        update_state(
            state,
            Transition::CompleteJob {
                job_id: spec.id,
                result: OpResult::Success(SuccessData::DirectoryRead(listing)),
            },
        )
    }

    /// Alt+s on the cursor entry, then the worker reports `size`.
    fn calculate_size(state: &mut AppState, size: u64) -> Vec<String> {
        let mut logs = Vec::new();
        let mut spec = None;
        for transition in action_to_transitions(state, &Action::CalculateDirectorySize) {
            let update = update_state(state, transition);
            spec = spec.or_else(|| {
                update
                    .jobs_to_start
                    .into_iter()
                    .find(|j| matches!(j.kind, JobKind::CalculateSize { .. }))
            });
        }
        let spec = spec.expect("Alt+s starts a size job instead of only queueing it");
        state.jobs.start_job(spec.clone());
        let update = update_state(
            state,
            Transition::CompleteJob {
                job_id: spec.id,
                result: OpResult::Success(SuccessData::SizeCalculated(size)),
            },
        );
        logs.extend(update.task_panel_logs);
        logs
    }

    // --- D7: the cursor stays on its file -------------------------------------------

    #[test]
    fn the_cursor_follows_its_file_when_one_appears_above_it() {
        let mut state = state_showing(files(&["b", "c", "d"]), 1);

        refresh_returns(&mut state, files(&["a", "b", "c", "d"]));

        let pane = left(&state);
        assert_eq!(pane.current_entry().map(|e| e.name.as_str()), Some("c"));
    }

    #[test]
    fn the_cursor_keeps_its_screen_row_when_the_list_shifts() {
        let names: Vec<String> = (0..40).map(|i| format!("f{i:02}")).collect();
        let listing: Vec<FileEntry> = names.iter().map(|n| file(n)).collect();
        let mut state = state_showing(listing.clone(), 15);
        state.current_tab_mut().left_pane.scroll_offset = 10;

        let mut grown = files(&["a0", "a1"]);
        grown.extend(listing);
        refresh_returns(&mut state, grown);

        let pane = left(&state);
        assert_eq!(pane.current_entry().map(|e| e.name.as_str()), Some("f15"));
        assert_eq!(pane.cursor - pane.scroll_offset, 5, "same screen row");
    }

    #[test]
    fn the_cursor_keeps_its_index_when_its_file_is_gone() {
        let mut state = state_showing(files(&["a", "b", "c", "d"]), 2);

        refresh_returns(&mut state, files(&["a", "b", "d"]));

        assert_eq!(left(&state).cursor, 2);
    }

    #[test]
    fn the_cursor_index_is_clamped_when_the_list_shrinks_past_it() {
        let mut state = state_showing(files(&["a", "b", "c", "d"]), 3);

        refresh_returns(&mut state, files(&["a", "b"]));

        assert_eq!(left(&state).cursor, 1);
    }

    #[test]
    fn a_pending_cursor_name_still_wins() {
        let mut state = state_showing(files(&["b", "c", "d"]), 1);
        state.current_tab_mut().left_pane.pending_cursor_name = Some("d".to_string());

        refresh_returns(&mut state, files(&["a", "b", "c", "d"]));

        assert_eq!(
            left(&state).current_entry().map(|e| e.name.as_str()),
            Some("d")
        );
    }

    /// A same-named file in a *different* directory is not "its file".
    #[test]
    fn the_cursor_is_not_restored_by_name_across_directories() {
        let mut state = state_showing(files(&["a", "b", "c"]), 2);
        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/other"));
        let other: Vec<FileEntry> = ["c", "x", "y"]
            .iter()
            .map(|n| {
                FileEntryBuilder::new(n)
                    .path(&format!("/other/{n}"))
                    .modified(SystemTime::UNIX_EPOCH)
                    .build()
            })
            .collect();

        refresh_returns(&mut state, other);

        assert_eq!(left(&state).cursor, 2);
    }

    // --- D12: pane change counter ---------------------------------------------------

    #[test]
    fn a_listing_change_bumps_the_counter_and_an_identical_read_does_not() {
        let mut state = state_showing(files(&["a", "b"]), 0);
        let before = left(&state).listing_generation;

        refresh_returns(&mut state, files(&["a", "b"]));
        assert_eq!(left(&state).listing_generation, before, "unchanged read");

        refresh_returns(&mut state, files(&["a", "b", "c"]));
        assert!(left(&state).listing_generation > before, "changed read");
    }

    #[test]
    fn an_in_memory_rename_bumps_the_counter() {
        let mut state = state_showing(files(&["a", "b"]), 0);
        let before = left(&state).listing_generation;
        let from = file("a").location;
        let to = Location::Local(PathBuf::from("/test/z"));
        let spec = JobSpec::new(JobKind::Rename {
            from: from.clone(),
            to: to.clone(),
        });
        state.jobs.start_job(spec.clone());

        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: spec.id,
                result: OpResult::Success(SuccessData::OperationRecords(vec![OperationRecord {
                    source: Some(from),
                    destination: Some(to),
                    succeeded: true,
                    failure_reason: None,
                    undo: UndoAvailability::NotApplicable,
                }])),
            },
        );

        assert!(left(&state).listing_generation > before);
    }

    #[test]
    fn an_in_memory_delete_bumps_the_counter() {
        let mut state = state_showing(files(&["a", "b"]), 0);
        let before = left(&state).listing_generation;
        let target = file("a").location;
        let spec = JobSpec::new(JobKind::Delete {
            targets: vec![target.clone()],
        });
        state.jobs.start_job(spec.clone());

        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: spec.id,
                result: OpResult::Success(SuccessData::OperationRecords(vec![OperationRecord {
                    source: Some(target),
                    destination: None,
                    succeeded: true,
                    failure_reason: None,
                    undo: UndoAvailability::NotApplicable,
                }])),
            },
        );

        assert!(left(&state).listing_generation > before);
    }

    /// Re-entering the directory already shown clears `entries` but not `raw_entries`;
    /// an identical read must still fill the pane (and is not a listing change).
    #[test]
    fn an_identical_read_refills_a_pane_cleared_by_navigation() {
        let mut state = state_showing(files(&["a", "b"]), 0);
        let before = left(&state).listing_generation;
        state.current_tab_mut().left_pane.entries.clear();

        refresh_returns(&mut state, files(&["a", "b"]));

        assert_eq!(left(&state).entries.len(), 2);
        assert_eq!(left(&state).listing_generation, before);
    }

    // --- D8a: size result is logged with name and size --------------------------------

    #[test]
    fn a_size_calculation_logs_the_directory_name_and_its_size() {
        let mut state = state_showing(vec![dir("photos"), file("x")], 0);

        let logs = calculate_size(&mut state, 13_314_398_618);

        let expected = FileEntryBuilder::new("photos")
            .calculated_size(Some(13_314_398_618))
            .build()
            .formatted_size();
        assert!(
            logs.iter()
                .any(|l| l.contains(&format!("Calculate size: photos — {expected} [OK]"))),
            "got {logs:?}"
        );
    }

    // --- D8b: calculated sizes survive refreshes and can go stale ---------------------

    #[test]
    fn a_calculated_size_survives_filter_reapplication() {
        let mut state = state_showing(vec![dir("photos"), file("x")], 0);
        calculate_size(&mut state, 5000);

        state.current_tab_mut().left_pane.apply_current_filter();

        assert_eq!(left(&state).entries[0].calculated_size, Some(5000));
    }

    #[test]
    fn a_fresh_size_is_not_stale() {
        let mut state = state_showing(vec![dir("photos"), file("x")], 0);
        calculate_size(&mut state, 5000);

        let pane = left(&state);
        assert!(!pane.is_size_stale(&pane.entries[0]));
    }

    #[test]
    fn an_identical_read_keeps_the_size_fresh() {
        let mut state = state_showing(vec![dir("photos"), file("x")], 0);
        calculate_size(&mut state, 5000);

        refresh_returns(&mut state, vec![dir("photos"), file("x")]);

        let pane = left(&state);
        assert_eq!(pane.entries[0].calculated_size, Some(5000));
        assert!(!pane.is_size_stale(&pane.entries[0]));
    }

    #[test]
    fn a_changed_listing_carries_the_size_over_as_stale() {
        let mut state = state_showing(vec![dir("photos"), file("x")], 0);
        calculate_size(&mut state, 5000);

        refresh_returns(&mut state, vec![dir("photos"), file("x"), file("y")]);

        let pane = left(&state);
        let photos = pane
            .entries
            .iter()
            .find(|e| e.name == "photos")
            .expect("photos");
        assert_eq!(photos.calculated_size, Some(5000));
        assert!(pane.is_size_stale(photos));
    }

    #[test]
    fn a_recalculated_size_is_fresh_again() {
        let mut state = state_showing(vec![dir("photos"), file("x")], 0);
        calculate_size(&mut state, 5000);
        refresh_returns(&mut state, vec![dir("photos"), file("x"), file("y")]);

        calculate_size(&mut state, 6000);

        let pane = left(&state);
        assert_eq!(pane.entries[0].calculated_size, Some(6000));
        assert!(!pane.is_size_stale(&pane.entries[0]));
    }
}
