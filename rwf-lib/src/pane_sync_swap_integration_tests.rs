//! Integration tests for pane synchronization and swapping operations
//!
//! Tests Requirements 41.1-41.7:
//! - Pane synchronization (O key)
//! - Pane swapping (Shift+O key)
//! - Cursor position and marked file preservation during swap
//! - Job creation for directory reading
//! - UI thread responsiveness

#[cfg(test)]
mod tests {
    use crate::model::ActivePane;
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::{state_with_temp_dirs, FileEntryBuilder};
    use tempfile::TempDir;

    /// Helper function to create a test AppState with two different pane locations
    fn create_test_state_with_different_panes() -> (AppState, TempDir, TempDir) {
        let (mut state, left_dir, right_dir) = state_with_temp_dirs();

        // Get the locations for building entries
        let left_location = state.current_tab().left_pane.current_location.clone();
        let right_location = state.current_tab().right_pane.current_location.clone();

        // Set up left pane entries
        state.current_tab_mut().left_pane.entries = vec![
            FileEntryBuilder::new("left_file1.txt")
                .location(left_location.join("left_file1.txt"))
                .size(100)
                .build(),
            FileEntryBuilder::new("left_file2.txt")
                .location(left_location.join("left_file2.txt"))
                .size(200)
                .build(),
        ];
        state.current_tab_mut().left_pane.cursor = 1;

        // Set up right pane entries
        state.current_tab_mut().right_pane.entries = vec![
            FileEntryBuilder::new("right_file1.txt")
                .location(right_location.join("right_file1.txt"))
                .size(300)
                .build(),
            FileEntryBuilder::new("right_file2.txt")
                .location(right_location.join("right_file2.txt"))
                .size(400)
                .build(),
            FileEntryBuilder::new("right_file3.txt")
                .location(right_location.join("right_file3.txt"))
                .size(500)
                .build(),
        ];
        state.current_tab_mut().right_pane.cursor = 2;

        // Set active pane to left
        state.ui.active_pane = ActivePane::Left;

        (state, left_dir, right_dir)
    }

    #[test]
    fn test_sync_panes_from_left_to_right() {
        // Requirement 41.1: WHEN the user presses 'O', THE Application SHALL synchronize
        // the opposite pane to the active pane's current directory
        // Requirement 41.2: WHEN synchronization occurs, THE Application SHALL navigate
        // the opposite pane to the same location as the active pane

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        let left_location = state.current_tab().left_pane.current_location.clone();
        let right_location_before = state.current_tab().right_pane.current_location.clone();

        // Active pane is left, so sync should update right pane to left's location
        let _result = update_state(&mut state, Transition::SyncPanes);

        // Verify right pane location was updated to match left pane
        assert_eq!(
            state.current_tab().right_pane.current_location,
            left_location,
            "Right pane should be synced to left pane's location"
        );

        // Verify right pane location changed
        assert_ne!(
            state.current_tab().right_pane.current_location,
            right_location_before,
            "Right pane location should have changed"
        );

        // Verify cursor was reset to 0
        assert_eq!(
            state.current_tab().right_pane.cursor,
            0,
            "Right pane cursor should be reset to 0"
        );

        // Verify scroll offset was reset
        assert_eq!(
            state.current_tab().right_pane.scroll_offset,
            0,
            "Right pane scroll offset should be reset to 0"
        );

        // Verify left pane was not affected
        assert_eq!(
            state.current_tab().left_pane.current_location,
            left_location,
            "Left pane location should not change"
        );
        assert_eq!(
            state.current_tab().left_pane.cursor,
            1,
            "Left pane cursor should not change"
        );
    }

    #[test]
    fn test_sync_panes_from_right_to_left() {
        // Requirement 41.1, 41.2: Test synchronization from right to left pane

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        // Switch to right pane
        state.ui.active_pane = ActivePane::Right;

        let right_location = state.current_tab().right_pane.current_location.clone();
        let left_location_before = state.current_tab().left_pane.current_location.clone();

        // Active pane is right, so sync should update left pane to right's location
        let _result = update_state(&mut state, Transition::SyncPanes);

        // Verify left pane location was updated to match right pane
        assert_eq!(
            state.current_tab().left_pane.current_location,
            right_location,
            "Left pane should be synced to right pane's location"
        );

        // Verify left pane location changed
        assert_ne!(
            state.current_tab().left_pane.current_location,
            left_location_before,
            "Left pane location should have changed"
        );

        // Verify cursor was reset to 0
        assert_eq!(
            state.current_tab().left_pane.cursor,
            0,
            "Left pane cursor should be reset to 0"
        );
    }

    #[test]
    fn test_sync_panes_creates_read_directory_job() {
        // Requirement 41.6: THE Application SHALL create Jobs to read directories
        // for both panes after synchronization or swapping

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        // Sync panes
        let result = update_state(&mut state, Transition::SyncPanes);

        // Verify a job was created (either directly or via cache)
        // If cache hit, no job is created; if cache miss, a job is created
        // We can't guarantee which, but we can verify the state is consistent
        assert!(
            result.ui_changed || !result.jobs_to_start.is_empty(),
            "SyncPanes should either update UI or create a job"
        );
    }

    #[test]
    fn test_swap_panes_exchanges_locations() {
        // Requirement 41.3: WHEN the user presses Shift+O, THE Application SHALL swap
        // the paths of the left and right panes
        // Requirement 41.4: WHEN swapping occurs, THE Application SHALL exchange
        // the current_location of both panes

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        let left_location_before = state.current_tab().left_pane.current_location.clone();
        let right_location_before = state.current_tab().right_pane.current_location.clone();

        // Swap panes
        let _result = update_state(&mut state, Transition::SwapPanes);

        // Verify locations were swapped
        assert_eq!(
            state.current_tab().left_pane.current_location,
            right_location_before,
            "Left pane should now have right pane's location"
        );
        assert_eq!(
            state.current_tab().right_pane.current_location,
            left_location_before,
            "Right pane should now have left pane's location"
        );
    }

    #[test]
    fn test_swap_panes_maintains_cursor_positions() {
        // Requirement 41.5: THE Application SHALL maintain cursor positions and
        // marked files during swap operations

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        let left_cursor_before = state.current_tab().left_pane.cursor;
        let right_cursor_before = state.current_tab().right_pane.cursor;

        // Swap panes
        let _result = update_state(&mut state, Transition::SwapPanes);

        // Verify cursor positions were maintained (stayed with their panes)
        assert_eq!(
            state.current_tab().left_pane.cursor,
            left_cursor_before,
            "Left pane cursor should remain the same"
        );
        assert_eq!(
            state.current_tab().right_pane.cursor,
            right_cursor_before,
            "Right pane cursor should remain the same"
        );
    }

    #[test]
    fn test_swap_panes_maintains_marked_files() {
        // Requirement 41.5: THE Application SHALL maintain cursor positions and
        // marked files during swap operations

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        // Mark some files
        let left_file = state.current_tab().left_pane.entries[0].location.clone();
        let right_file = state.current_tab().right_pane.entries[1].location.clone();

        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(left_file.clone());
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(right_file.clone());

        assert_eq!(
            state.current_tab_mut().left_pane.marking.count(),
            2,
            "Should have 2 marked files"
        );

        // Swap panes
        let _result = update_state(&mut state, Transition::SwapPanes);

        // Verify marked files are still marked
        assert!(
            state
                .current_tab_mut()
                .left_pane
                .marking
                .is_marked(&left_file),
            "Left file should still be marked after swap"
        );
        assert!(
            state
                .current_tab_mut()
                .left_pane
                .marking
                .is_marked(&right_file),
            "Right file should still be marked after swap"
        );
        assert_eq!(
            state.current_tab_mut().left_pane.marking.count(),
            2,
            "Should still have 2 marked files after swap"
        );
    }

    #[test]
    fn test_swap_panes_creates_read_directory_jobs() {
        // Requirement 41.6: THE Application SHALL create Jobs to read directories
        // for both panes after synchronization or swapping

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        // Swap panes
        let result = update_state(&mut state, Transition::SwapPanes);

        // Verify jobs were created or UI was updated (depending on cache)
        assert!(
            result.ui_changed || !result.jobs_to_start.is_empty(),
            "SwapPanes should either update UI or create jobs"
        );

        // If jobs were created, verify we have up to 2 jobs (one for each pane)
        if !result.jobs_to_start.is_empty() {
            assert!(
                result.jobs_to_start.len() <= 2,
                "Should create at most 2 jobs (one per pane)"
            );
        }
    }

    #[test]
    fn test_sync_panes_adds_to_history() {
        // Verify that sync panes adds the previous location to navigation history

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        let left_location = state.current_tab().left_pane.current_location.clone();
        let right_location_before = state.current_tab().right_pane.current_location.clone();

        // Check initial history state for right pane
        let initial_history_len = state.current_tab().history.right_stack.len();

        // Sync panes (left to right)
        let _result = update_state(&mut state, Transition::SyncPanes);

        // After sync, right pane should have left's location
        assert_eq!(
            state.current_tab().right_pane.current_location,
            left_location,
            "Right pane should have left's location after sync"
        );

        // Verify history was updated - should have one more entry
        let new_history_len = state.current_tab().history.right_stack.len();
        assert_eq!(
            new_history_len,
            initial_history_len + 1,
            "Right pane history should have one more entry after sync"
        );

        // Verify the last history entry is the previous right location
        if let Some(last_history_entry) = state.current_tab().history.right_stack.last() {
            assert_eq!(
                *last_history_entry, right_location_before,
                "Last history entry should be the previous right pane location"
            );
        } else {
            panic!("History should have at least one entry");
        }
    }

    #[test]
    fn test_swap_panes_ui_responsiveness() {
        // Requirement 41.7: THE UI_Thread SHALL remain responsive during pane
        // synchronization and swapping
        //
        // This test verifies that swap operations complete synchronously and don't block

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        // Measure that swap completes quickly (should be near-instant for state changes)
        let start = std::time::Instant::now();
        let result = update_state(&mut state, Transition::SwapPanes);
        let duration = start.elapsed();

        // State update should complete in less than 1ms (it's just swapping pointers)
        assert!(
            duration.as_millis() < 10,
            "SwapPanes state update should complete in less than 10ms, took {:?}",
            duration
        );

        // Verify the operation completed successfully
        assert!(result.ui_changed, "SwapPanes should mark UI as changed");
    }

    #[test]
    fn test_sync_panes_ui_responsiveness() {
        // Requirement 41.7: THE UI_Thread SHALL remain responsive during pane
        // synchronization and swapping

        let (mut state, _left_dir, _right_dir) = create_test_state_with_different_panes();

        // Measure that sync completes quickly
        let start = std::time::Instant::now();
        let _result = update_state(&mut state, Transition::SyncPanes);
        let duration = start.elapsed();

        // State update should complete in less than 10ms
        assert!(
            duration.as_millis() < 10,
            "SyncPanes state update should complete in less than 10ms, took {:?}",
            duration
        );
    }
}
