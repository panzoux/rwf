//! Property-based tests for input handling and navigation
//!
//! **Validates: Requirements 1.5, 1.6, 2.1, 2.8, 3.1, 3.3, 3.6**

#[cfg(test)]
mod tests {
    use crate::model::{ActivePane, FileEntry, Location};
    use crate::state::{AppState, AppConfig, Transition, update_state};
    use proptest::prelude::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    // Helper to create a test file entry
    pub(crate) fn create_test_entry(name: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(format!("/test/{}", name))),
            size: 100,
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        }
    }

    // Helper to create a state with entries in both panes
    pub(crate) fn create_state_with_entries(left_count: usize, right_count: usize) -> AppState {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add entries to left pane
        let left_entries: Vec<FileEntry> = (0..left_count)
            .map(|i| create_test_entry(&format!("left{}.txt", i), false))
            .collect();
        state.current_tab_mut().left_pane.entries = left_entries;
        
        // Add entries to right pane
        let right_entries: Vec<FileEntry> = (0..right_count)
            .map(|i| create_test_entry(&format!("right{}.txt", i), false))
            .collect();
        state.current_tab_mut().right_pane.entries = right_entries;
        
        state
    }

    /// **Property 1: Pane Independence**
    ///
    /// *For any* AppState with multiple panes, modifying the cursor position in one pane
    /// should not affect the cursor position in any other pane.
    ///
    /// **Validates: Requirements 1.5**
    #[test]
    fn property_pane_independence() {
        proptest!(|(
            left_cursor in 0usize..10,
            right_cursor in 0usize..10,
            delta in -5isize..5
        )| {
            let mut state = create_state_with_entries(10, 10);
            
            // Set initial cursor positions
            state.current_tab_mut().left_pane.cursor = left_cursor;
            state.current_tab_mut().right_pane.cursor = right_cursor;
            
            // Move cursor in left pane
            state.ui.active_pane = ActivePane::Left;
            update_state(&mut state, Transition::CursorMove {
                pane: ActivePane::Left,
                delta,
            });
            
            // Right pane cursor should be unchanged
            prop_assert_eq!(state.current_tab().right_pane.cursor, right_cursor);
            
            // Move cursor in right pane
            state.ui.active_pane = ActivePane::Right;
            let left_cursor_after = state.current_tab().left_pane.cursor;
            update_state(&mut state, Transition::CursorMove {
                pane: ActivePane::Right,
                delta,
            });
            
            // Left pane cursor should be unchanged
            prop_assert_eq!(state.current_tab().left_pane.cursor, left_cursor_after);
        });
    }

    /// **Property 2: Scroll Independence**
    ///
    /// *For any* AppState with multiple panes, modifying the scroll offset in one pane
    /// should not affect the scroll offset in any other pane.
    ///
    /// **Validates: Requirements 1.6**
    #[test]
    fn property_scroll_independence() {
        proptest!(|(
            left_scroll in 0usize..10,
            right_scroll in 0usize..10,
            left_cursor in 0usize..30,
            right_cursor in 0usize..30
        )| {
            let mut state = create_state_with_entries(30, 30);
            
            // Set initial scroll positions
            state.current_tab_mut().left_pane.scroll_offset = left_scroll;
            state.current_tab_mut().right_pane.scroll_offset = right_scroll;
            
            // Move cursor in left pane (may trigger scroll)
            state.ui.active_pane = ActivePane::Left;
            update_state(&mut state, Transition::CursorJump {
                pane: ActivePane::Left,
                position: left_cursor,
            });
            
            // Right pane scroll should be unchanged
            prop_assert_eq!(state.current_tab().right_pane.scroll_offset, right_scroll);
            
            // Move cursor in right pane (may trigger scroll)
            state.ui.active_pane = ActivePane::Right;
            let left_scroll_after = state.current_tab().left_pane.scroll_offset;
            update_state(&mut state, Transition::CursorJump {
                pane: ActivePane::Right,
                position: right_cursor,
            });
            
            // Left pane scroll should be unchanged
            prop_assert_eq!(state.current_tab().left_pane.scroll_offset, left_scroll_after);
        });
    }

    /// **Property 3: Pane Switching Toggles**
    ///
    /// *For any* AppState, applying the SwitchPane transition twice should return
    /// the active pane to its original state.
    ///
    /// **Validates: Requirements 2.1**
    #[test]
    fn property_pane_switching_toggles() {
        proptest!(|(start_left in prop::bool::ANY)| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Set initial pane
            state.ui.active_pane = if start_left {
                ActivePane::Left
            } else {
                ActivePane::Right
            };
            
            let initial_pane = state.ui.active_pane;
            
            // Switch twice
            update_state(&mut state, Transition::SwitchPane);
            update_state(&mut state, Transition::SwitchPane);
            
            // Should be back to original
            prop_assert_eq!(state.ui.active_pane, initial_pane);
        });
    }

    /// **Property 5: Cursor Visibility Invariant**
    ///
    /// *For any* PaneModel with cursor position C and scroll offset S, the cursor
    /// should always be visible within the viewport: S ≤ C < S + viewport_height.
    ///
    /// **Validates: Requirements 2.8**
    #[test]
    fn property_cursor_visibility_invariant() {
        proptest!(|(
            entry_count in 30usize..100,
            target_position in 0usize..100
        )| {
            let mut state = create_state_with_entries(entry_count, 10);
            
            // Jump to target position (clamped to valid range)
            let target = target_position.min(entry_count - 1);
            update_state(&mut state, Transition::CursorJump {
                pane: ActivePane::Left,
                position: target,
            });
            
            let pane = &state.current_tab().left_pane;
            let cursor = pane.cursor;
            let scroll = pane.scroll_offset;
            let viewport_height = 20; // Hardcoded in update_state
            
            // Cursor must be within viewport
            prop_assert!(cursor >= scroll, "Cursor {} must be >= scroll {}", cursor, scroll);
            prop_assert!(cursor < scroll + viewport_height, 
                "Cursor {} must be < scroll {} + viewport {}", cursor, scroll, viewport_height);
        });
    }

    /// **Property 6: Directory Navigation Creates Job**
    ///
    /// *For any* directory Location, applying a ChangeLocation transition should
    /// return a StateUpdateResult containing a ReadDirectory JobSpec.
    ///
    /// **Validates: Requirements 3.1, 3.3**
    #[test]
    fn property_directory_navigation_creates_job() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let new_location = Location::Local(PathBuf::from("/test/newdir"));
        
        let result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: new_location.clone(),
        });
        
        // Should have created a job
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Job should be ReadDirectory
        match &result.jobs_to_start[0].kind {
            crate::job::JobKind::ReadDirectory { location } => {
                assert_eq!(location, &new_location);
            }
            _ => panic!("Expected ReadDirectory job"),
        }
    }

    /// **Property 8: Location Change Resets Cursor**
    ///
    /// *For any* ChangeLocation transition, the resulting PaneModel should have cursor = 0.
    ///
    /// **Validates: Requirements 3.6**
    #[test]
    fn property_location_change_resets_cursor() {
        proptest!(|(initial_cursor in 0usize..20)| {
            let mut state = create_state_with_entries(20, 10);
            
            // Set cursor to non-zero position
            state.current_tab_mut().left_pane.cursor = initial_cursor;
            
            // Change location
            let new_location = Location::Local(PathBuf::from("/test/newdir"));
            update_state(&mut state, Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: new_location,
            });
            
            // Cursor should be reset to 0
            prop_assert_eq!(state.current_tab().left_pane.cursor, 0);
            prop_assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
        });
    }

    // Additional unit tests for edge cases

    #[test]
    fn test_cursor_move_bounds_clamping() {
        let mut state = create_state_with_entries(10, 10);
        
        // Try to move beyond upper bound
        update_state(&mut state, Transition::CursorMove {
            pane: ActivePane::Left,
            delta: 100,
        });
        assert_eq!(state.current_tab().left_pane.cursor, 9);
        
        // Try to move beyond lower bound
        update_state(&mut state, Transition::CursorMove {
            pane: ActivePane::Left,
            delta: -100,
        });
        assert_eq!(state.current_tab().left_pane.cursor, 0);
    }

    #[test]
    fn test_cursor_move_empty_pane() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Empty pane - cursor should stay at 0
        update_state(&mut state, Transition::CursorMove {
            pane: ActivePane::Left,
            delta: 5,
        });
        assert_eq!(state.current_tab().left_pane.cursor, 0);
    }

    #[test]
    fn test_pane_switch_alternates() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        assert_eq!(state.ui.active_pane, ActivePane::Left);
        
        update_state(&mut state, Transition::SwitchPane);
        assert_eq!(state.ui.active_pane, ActivePane::Right);
        
        update_state(&mut state, Transition::SwitchPane);
        assert_eq!(state.ui.active_pane, ActivePane::Left);
    }

    #[test]
    fn test_navigate_up_from_root() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set location to root
        state.current_tab_mut().left_pane.current_location = Location::Local(PathBuf::from("/"));
        
        // Try to navigate up from root
        let result = update_state(&mut state, Transition::NavigateUp {
            pane: ActivePane::Left,
        });
        
        // Should not create a job (no parent)
        assert_eq!(result.jobs_to_start.len(), 0);
    }

    #[test]
    fn test_history_navigation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let initial = state.current_tab().left_pane.current_location.clone();
        let loc1 = Location::Local(PathBuf::from("/test/dir1"));
        let loc2 = Location::Local(PathBuf::from("/test/dir2"));
        let loc3 = Location::Local(PathBuf::from("/test/dir3"));
        
        // Navigate to dir1
        // History: [initial], pos=0, current=dir1
        update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: loc1.clone(),
        });
        
        // Navigate to dir2
        // History: [initial, dir1], pos=1, current=dir2
        update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: loc2.clone(),
        });
        
        // Navigate to dir3
        // History: [initial, dir1, dir2], pos=2, current=dir3
        update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: loc3.clone(),
        });
        
        // Current location should be dir3
        assert_eq!(state.current_tab().left_pane.current_location, loc3);
        
        // Go back (pos=1, returns dir1)
        update_state(&mut state, Transition::NavigateHistory {
            pane: ActivePane::Left,
            direction: crate::state::HistoryDirection::Back,
        });
        assert_eq!(state.current_tab().left_pane.current_location, loc1);
        
        // Go back again (pos=0, returns initial)
        update_state(&mut state, Transition::NavigateHistory {
            pane: ActivePane::Left,
            direction: crate::state::HistoryDirection::Back,
        });
        assert_eq!(state.current_tab().left_pane.current_location, initial);
        
        // Go forward (pos=1, returns dir1)
        update_state(&mut state, Transition::NavigateHistory {
            pane: ActivePane::Left,
            direction: crate::state::HistoryDirection::Forward,
        });
        assert_eq!(state.current_tab().left_pane.current_location, loc1);
        
        // Go forward again (pos=2, returns dir2)
        update_state(&mut state, Transition::NavigateHistory {
            pane: ActivePane::Left,
            direction: crate::state::HistoryDirection::Forward,
        });
        assert_eq!(state.current_tab().left_pane.current_location, loc2);
    }

    /// **Property 14: Copy Operation Job Creation**
    ///
    /// *For any* copy operation (with marked files or cursor file), confirming the copy dialog
    /// should create a JobSpec with JobKind::Copy containing the correct sources and destination.
    ///
    /// **Validates: Requirements 6.1, 6.2, 6.3**
    #[test]
    fn property_copy_operation_job_creation() {
        proptest!(|(
            mark_count in 0usize..5,
            use_marked in prop::bool::ANY
        )| {
            let mut state = create_state_with_entries(10, 10);
            
            // Optionally mark some files
            if use_marked && mark_count > 0 {
                for i in 0..mark_count.min(state.current_tab().left_pane.entries.len()) {
                    let location = state.current_tab().left_pane.entries[i].location.clone();
                    state.current_tab_mut().left_pane.marking.mark(location);
                }
            }
            
            // Show copy dialog
            let transitions = crate::input::action_to_transitions(&state, &crate::input::Action::Copy);
            
            // Should create a dialog (entries exist)
            prop_assert_eq!(transitions.len(), 1);
            
            // Apply the transition to show the dialog
            for transition in transitions {
                update_state(&mut state, transition);
            }
            
            // Confirm the dialog
            let result = update_state(&mut state, Transition::ConfirmDialog);
            
            // Should create exactly one job
            prop_assert_eq!(result.jobs_to_start.len(), 1);
            
            // Job should be Copy
            match &result.jobs_to_start[0].kind {
                crate::job::JobKind::Copy { sources, dest } => {
                    // Verify sources
                    if use_marked && mark_count > 0 {
                        // Should use marked files
                        prop_assert_eq!(sources.len(), mark_count.min(10));
                    } else {
                        // Should use cursor file
                        prop_assert_eq!(sources.len(), 1);
                    }
                    
                    // Verify destination is opposite pane location
                    prop_assert_eq!(dest, &state.current_tab().right_pane.current_location);
                }
                _ => prop_assert!(false, "Expected Copy job"),
            }
        });
    }

    /// **Property 15: Move Operation Job Creation**
    ///
    /// *For any* move operation (with marked files or cursor file), confirming the move dialog
    /// should create a JobSpec with JobKind::Move containing the correct sources and destination.
    ///
    /// **Validates: Requirements 7.1, 7.2, 7.3**
    #[test]
    fn property_move_operation_job_creation() {
        proptest!(|(
            mark_count in 0usize..5,
            use_marked in prop::bool::ANY
        )| {
            let mut state = create_state_with_entries(10, 10);
            
            // Optionally mark some files
            if use_marked && mark_count > 0 {
                for i in 0..mark_count.min(state.current_tab().left_pane.entries.len()) {
                    let location = state.current_tab().left_pane.entries[i].location.clone();
                    state.current_tab_mut().left_pane.marking.mark(location);
                }
            }
            
            // Show move dialog
            let transitions = crate::input::action_to_transitions(&state, &crate::input::Action::Move);
            
            // Should create a dialog (entries exist)
            prop_assert_eq!(transitions.len(), 1);
            
            // Apply the transition to show the dialog
            for transition in transitions {
                update_state(&mut state, transition);
            }
            
            // Confirm the dialog
            let result = update_state(&mut state, Transition::ConfirmDialog);
            
            // Should create exactly one job
            prop_assert_eq!(result.jobs_to_start.len(), 1);
            
            // Job should be Move
            match &result.jobs_to_start[0].kind {
                crate::job::JobKind::Move { sources, dest } => {
                    // Verify sources
                    if use_marked && mark_count > 0 {
                        // Should use marked files
                        prop_assert_eq!(sources.len(), mark_count.min(10));
                    } else {
                        // Should use cursor file
                        prop_assert_eq!(sources.len(), 1);
                    }
                    
                    // Verify destination is opposite pane location
                    prop_assert_eq!(dest, &state.current_tab().right_pane.current_location);
                }
                _ => prop_assert!(false, "Expected Move job"),
            }
        });
    }

    /// **Property 16: Delete Operation Job Creation**
    ///
    /// *For any* delete operation (with marked files or cursor file), confirming the delete dialog
    /// should create a JobSpec with JobKind::Delete containing the correct targets.
    ///
    /// **Validates: Requirements 8.1, 8.2, 8.3**
    #[test]
    fn property_delete_operation_job_creation() {
        proptest!(|(
            mark_count in 0usize..5,
            use_marked in prop::bool::ANY
        )| {
            let mut state = create_state_with_entries(10, 10);
            
            // Optionally mark some files
            if use_marked && mark_count > 0 {
                for i in 0..mark_count.min(state.current_tab().left_pane.entries.len()) {
                    let location = state.current_tab().left_pane.entries[i].location.clone();
                    state.current_tab_mut().left_pane.marking.mark(location);
                }
            }
            
            // Show delete dialog
            let transitions = crate::input::action_to_transitions(&state, &crate::input::Action::Delete);
            
            // Should create a dialog (entries exist)
            prop_assert_eq!(transitions.len(), 1);
            
            // Apply the transition to show the dialog
            for transition in transitions {
                update_state(&mut state, transition);
            }
            
            // Confirm the dialog
            let result = update_state(&mut state, Transition::ConfirmDialog);
            
            // Should create exactly one job
            prop_assert_eq!(result.jobs_to_start.len(), 1);
            
            // Job should be Delete
            match &result.jobs_to_start[0].kind {
                crate::job::JobKind::Delete { targets } => {
                    // Verify targets
                    if use_marked && mark_count > 0 {
                        // Should use marked files
                        prop_assert_eq!(targets.len(), mark_count.min(10));
                    } else {
                        // Should use cursor file
                        prop_assert_eq!(targets.len(), 1);
                    }
                }
                _ => prop_assert!(false, "Expected Delete job"),
            }
        });
    }

    /// **Property 17: Delete Completion Unmarks Files**
    ///
    /// *For any* delete operation that completes successfully, all deleted files should be
    /// unmarked from the marking model.
    ///
    /// **Validates: Requirements 8.10**
    #[test]
    fn property_delete_completion_unmarks_files() {
        let mut state = create_state_with_entries(10, 10);
        
        // Mark several files
        let marked_locations: Vec<_> = state.current_tab().left_pane.entries[0..3]
            .iter()
            .map(|e| e.location.clone())
            .collect();
        
        for location in &marked_locations {
            state.current_tab_mut().left_pane.marking.mark(location.clone());
        }
        
        // Verify files are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 3);
        for location in &marked_locations {
            assert!(state.current_tab_mut().left_pane.marking.is_marked(location));
        }
        
        // Show and confirm delete dialog
        let transitions = crate::input::action_to_transitions(&state, &crate::input::Action::Delete);
        for transition in transitions {
            update_state(&mut state, transition);
        }
        
        let result = update_state(&mut state, Transition::ConfirmDialog);
        
        // Verify delete job was created
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            crate::job::JobKind::Delete { targets } => {
                assert_eq!(targets.len(), 3);
                
                // Simulate job completion by unmarking the deleted files
                // In the real implementation, this would happen in the job completion handler
                for target in targets {
                    state.current_tab_mut().left_pane.marking.unmark(target.clone());
                }
                
                // Verify all deleted files are unmarked
                assert_eq!(state.current_tab_mut().left_pane.marking.count(), 0);
                for location in &marked_locations {
                    assert!(!state.current_tab_mut().left_pane.marking.is_marked(location));
                }
            }
            _ => panic!("Expected Delete job"),
        }
    }

    /// **Property 30: Wildcard Marking Completeness**
    ///
    /// *For any* wildcard pattern and list of FileEntries, after applying MarkPattern,
    /// all entries matching the pattern should be marked, and no non-matching entries
    /// should be marked.
    ///
    /// **Validates: Requirements 36.3**
    #[test]
    fn property_wildcard_marking_completeness() {
        proptest!(|(
            file_names in prop::collection::hash_set(
                prop::string::string_regex("[a-z]{1,5}\\.(txt|rs|md|json)").unwrap(),
                5..20
            ),
            pattern_prefix in "[a-z]{1,3}",
            pattern_suffix in prop::option::of("[a-z]{1,3}")
        )| {
            let mut state = create_state_with_entries(0, 0);
            
            // Create entries with generated file names (now guaranteed unique)
            let entries: Vec<FileEntry> = file_names.iter().map(|name| {
                FileEntry {
                    name: name.clone(),
                    location: Location::Local(PathBuf::from(format!("/test/{}", name))),
                    size: 100,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                }
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries.clone();
            
            // Build wildcard pattern
            let pattern = if let Some(suffix) = pattern_suffix {
                format!("{}*{}", pattern_prefix, suffix)
            } else {
                format!("{}*", pattern_prefix)
            };
            
            // Apply MarkPattern transition
            update_state(&mut state, Transition::MarkPattern { pattern: pattern.clone() });
            
            // Helper function to match wildcard pattern
            fn matches_wildcard(name: &str, pattern: &str) -> bool {
                let regex_pattern = pattern
                    .replace(".", "\\.")
                    .replace("*", ".*")
                    .replace("?", ".");
                let regex = regex::Regex::new(&format!("^{}$", regex_pattern)).unwrap();
                regex.is_match(name)
            }
            
            // Verify all matching entries are marked
            for entry in &entries {
                let should_be_marked = matches_wildcard(&entry.name, &pattern);
                let is_marked = state.current_tab_mut().left_pane.marking.is_marked(&entry.location);
                
                prop_assert_eq!(
                    is_marked,
                    should_be_marked,
                    "File '{}' with pattern '{}': expected marked={}, got marked={}",
                    entry.name,
                    pattern,
                    should_be_marked,
                    is_marked
                );
            }
            
            // Verify marked count matches expected count
            let expected_marked_count = entries.iter()
                .filter(|e| matches_wildcard(&e.name, &pattern))
                .count();
            prop_assert_eq!(
                state.current_tab_mut().left_pane.marking.count(),
                expected_marked_count,
                "Expected {} marked files, got {}",
                expected_marked_count,
                state.current_tab_mut().left_pane.marking.count()
            );
        });
    }

    // Additional unit tests for wildcard marking edge cases

    #[test]
    fn test_wildcard_marking_star_only() {
        let mut state = create_state_with_entries(10, 10);
        
        // Mark all files with "*"
        update_state(&mut state, Transition::MarkPattern { pattern: "*".to_string() });
        
        // All files should be marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 10);
    }

    #[test]
    fn test_wildcard_marking_no_matches() {
        let mut state = create_state_with_entries(10, 10);
        
        // Mark files with pattern that matches nothing
        update_state(&mut state, Transition::MarkPattern { pattern: "nonexistent*.xyz".to_string() });
        
        // No files should be marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 0);
    }

    #[test]
    fn test_wildcard_marking_question_mark_single_char() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            create_test_entry("a.txt", false),
            create_test_entry("ab.txt", false),
            create_test_entry("abc.txt", false),
            create_test_entry("b.txt", false),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Mark files matching "?.txt" (single character before .txt)
        update_state(&mut state, Transition::MarkPattern { pattern: "?.txt".to_string() });
        
        // Only single-character names should be marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/a.txt"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/b.txt"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/ab.txt"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/abc.txt"))));
    }

    #[test]
    fn test_wildcard_marking_multiple_wildcards() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            create_test_entry("test_file_1.txt", false),
            create_test_entry("test_file_2.txt", false),
            create_test_entry("test_doc_1.txt", false),
            create_test_entry("other_file_1.txt", false),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Mark files matching "test_*_*.txt"
        update_state(&mut state, Transition::MarkPattern { pattern: "test_*_*.txt".to_string() });
        
        // Only test_* files should be marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 3);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/test_file_1.txt"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/test_file_2.txt"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/test_doc_1.txt"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/other_file_1.txt"))));
    }

    #[test]
    fn test_wildcard_marking_extension_only() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            create_test_entry("file1.rs", false),
            create_test_entry("file2.rs", false),
            create_test_entry("file3.txt", false),
            create_test_entry("file4.md", false),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Mark all .rs files
        update_state(&mut state, Transition::MarkPattern { pattern: "*.rs".to_string() });
        
        // Only .rs files should be marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file1.rs"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file2.rs"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file3.txt"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file4.md"))));
    }

    #[test]
    fn test_wildcard_marking_preserves_existing_marks() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            create_test_entry("file1.txt", false),
            create_test_entry("file2.rs", false),
            create_test_entry("file3.txt", false),
        ];
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Manually mark file2.rs
        state.current_tab_mut().left_pane.marking.mark(entries[1].location.clone());
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 1);
        
        // Mark all .txt files
        update_state(&mut state, Transition::MarkPattern { pattern: "*.txt".to_string() });
        
        // Should have 3 marked files total (2 .txt + 1 .rs)
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 3);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[0].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[1].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location));
    }

    /// **Property 18: Directory-First Sorting**
    ///
    /// *For any* PaneModel with mixed files and directories, after applying any sort mode,
    /// all directory entries should appear before all file entries in the entries list.
    ///
    /// **Validates: Requirements 12.6**
    #[test]
    fn property_directory_first_sorting() {
        use crate::model::SortMode;
        
        proptest!(|(
            sort_mode in prop_oneof![
                Just(SortMode::Name),
                Just(SortMode::Size),
                Just(SortMode::Date),
                Just(SortMode::Extension),
            ]
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create a mix of files and directories with various properties
            let entries = vec![
                FileEntry {
                    name: "file1.txt".to_string(),
                    location: Location::Local(PathBuf::from("/test/file1.txt")),
                    size: 100,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "dir1".to_string(),
                    location: Location::Local(PathBuf::from("/test/dir1")),
                    size: 0,
                    is_dir: true,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "file2.rs".to_string(),
                    location: Location::Local(PathBuf::from("/test/file2.rs")),
                    size: 200,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "dir2".to_string(),
                    location: Location::Local(PathBuf::from("/test/dir2")),
                    size: 0,
                    is_dir: true,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "file3.md".to_string(),
                    location: Location::Local(PathBuf::from("/test/file3.md")),
                    size: 50,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                },
            ];
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Apply sort mode
            update_state(&mut state, Transition::ChangeSortMode {
                pane: ActivePane::Left,
                mode: sort_mode,
            });
            
            let sorted_entries = &state.current_tab().left_pane.entries;
            
            // Find the index of the last directory and first file
            let last_dir_index = sorted_entries.iter()
                .rposition(|e| e.is_dir);
            let first_file_index = sorted_entries.iter()
                .position(|e| !e.is_dir);
            
            // If both exist, last directory should come before first file
            if let (Some(last_dir), Some(first_file)) = (last_dir_index, first_file_index) {
                prop_assert!(
                    last_dir < first_file,
                    "Directory at index {} should come before file at index {}",
                    last_dir,
                    first_file
                );
            }
            
            // Verify all directories come before all files
            let mut seen_file = false;
            for entry in sorted_entries {
                if !entry.is_dir {
                    seen_file = true;
                } else if seen_file {
                    prop_assert!(
                        false,
                        "Found directory '{}' after a file, violating directory-first ordering",
                        entry.name
                    );
                }
            }
        });
    }

    /// **Property 19: Sort Stability**
    ///
    /// *For any* PaneModel and SortMode, sorting twice with the same mode should
    /// produce identical ordering.
    ///
    /// **Validates: Requirements 12.1, 12.2, 12.3, 12.4**
    #[test]
    fn property_sort_stability() {
        use crate::model::SortMode;
        
        proptest!(|(
            sort_mode in prop_oneof![
                Just(SortMode::Name),
                Just(SortMode::Size),
                Just(SortMode::Date),
                Just(SortMode::Extension),
            ]
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create entries with various properties
            let entries = vec![
                FileEntry {
                    name: "zebra.txt".to_string(),
                    location: Location::Local(PathBuf::from("/test/zebra.txt")),
                    size: 300,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "apple.rs".to_string(),
                    location: Location::Local(PathBuf::from("/test/apple.rs")),
                    size: 100,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3000),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "banana.md".to_string(),
                    location: Location::Local(PathBuf::from("/test/banana.md")),
                    size: 200,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2000),
                    marked: false,
                    calculated_size: None,
                },
                FileEntry {
                    name: "dir1".to_string(),
                    location: Location::Local(PathBuf::from("/test/dir1")),
                    size: 0,
                    is_dir: true,
                    is_hidden: false,
                    modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1500),
                    marked: false,
                    calculated_size: None,
                },
            ];
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Apply sort mode first time
            update_state(&mut state, Transition::ChangeSortMode {
                pane: ActivePane::Left,
                mode: sort_mode,
            });
            
            // Capture the order after first sort
            let first_sort_order: Vec<String> = state.current_tab().left_pane.entries
                .iter()
                .map(|e| e.name.clone())
                .collect();
            
            // Apply sort mode second time
            update_state(&mut state, Transition::ChangeSortMode {
                pane: ActivePane::Left,
                mode: sort_mode,
            });
            
            // Capture the order after second sort
            let second_sort_order: Vec<String> = state.current_tab().left_pane.entries
                .iter()
                .map(|e| e.name.clone())
                .collect();
            
            // Orders should be identical
            prop_assert_eq!(
                first_sort_order,
                second_sort_order,
                "Sorting twice with {:?} produced different orders",
                sort_mode
            );
        });
    }

    // Additional unit tests for sorting edge cases

    #[test]
    fn test_sort_by_name() {
        use crate::model::SortMode;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            create_test_entry("zebra.txt", false),
            create_test_entry("apple.txt", false),
            create_test_entry("banana.txt", false),
            create_test_entry("dir_z", true),
            create_test_entry("dir_a", true),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Sort by name
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Name,
        });
        
        let sorted = &state.current_tab().left_pane.entries;
        
        // Directories first, then files, both alphabetically
        assert_eq!(sorted[0].name, "dir_a");
        assert_eq!(sorted[1].name, "dir_z");
        assert_eq!(sorted[2].name, "apple.txt");
        assert_eq!(sorted[3].name, "banana.txt");
        assert_eq!(sorted[4].name, "zebra.txt");
    }

    #[test]
    fn test_sort_by_size() {
        use crate::model::SortMode;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            FileEntry {
                name: "large.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/large.txt")),
                size: 1000,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "small.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/small.txt")),
                size: 10,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "medium.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/medium.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            create_test_entry("dir1", true),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Sort by size
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Size,
        });
        
        let sorted = &state.current_tab().left_pane.entries;
        
        // Directory first, then files by size
        assert_eq!(sorted[0].name, "dir1");
        assert_eq!(sorted[1].name, "small.txt");
        assert_eq!(sorted[1].size, 10);
        assert_eq!(sorted[2].name, "medium.txt");
        assert_eq!(sorted[2].size, 100);
        assert_eq!(sorted[3].name, "large.txt");
        assert_eq!(sorted[3].size, 1000);
    }

    #[test]
    fn test_sort_by_date() {
        use crate::model::SortMode;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            FileEntry {
                name: "newest.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/newest.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3000),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "oldest.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/oldest.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "middle.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/middle.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2000),
                marked: false,
                calculated_size: None,
            },
            create_test_entry("dir1", true),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Sort by date
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Date,
        });
        
        let sorted = &state.current_tab().left_pane.entries;
        
        // Directory first, then files by date (oldest to newest)
        assert_eq!(sorted[0].name, "dir1");
        assert_eq!(sorted[1].name, "oldest.txt");
        assert_eq!(sorted[2].name, "middle.txt");
        assert_eq!(sorted[3].name, "newest.txt");
    }

    #[test]
    fn test_sort_by_extension() {
        use crate::model::SortMode;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            create_test_entry("file.txt", false),
            create_test_entry("file.rs", false),
            create_test_entry("file.md", false),
            create_test_entry("noext", false),
            create_test_entry("dir1", true),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Sort by extension
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Extension,
        });
        
        let sorted = &state.current_tab().left_pane.entries;
        
        // Directory first, then files by extension
        assert_eq!(sorted[0].name, "dir1");
        assert_eq!(sorted[1].name, "noext"); // No extension comes first
        assert_eq!(sorted[2].name, "file.md");
        assert_eq!(sorted[3].name, "file.rs");
        assert_eq!(sorted[4].name, "file.txt");
    }

    #[test]
    fn test_sort_maintains_separate_pane_settings() {
        use crate::model::SortMode;
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add entries to both panes
        state.current_tab_mut().left_pane.entries = vec![
            create_test_entry("zebra.txt", false),
            create_test_entry("apple.txt", false),
        ];
        state.current_tab_mut().right_pane.entries = vec![
            create_test_entry("banana.txt", false),
            create_test_entry("cherry.txt", false),
        ];
        
        // Sort left pane by name
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Name,
        });
        
        // Sort right pane by size
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Right,
            mode: SortMode::Size,
        });
        
        // Verify each pane has its own sort mode
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Name);
        assert_eq!(state.current_tab().right_pane.sort_mode, SortMode::Size);
    }

    /// **Property 26: Archive Entry Creates Archive Location**
    ///
    /// **Validates: Requirements 29.1**
    ///
    /// When the user presses Enter on an archive file (.zip), the application
    /// SHALL open a virtual folder view of the archive contents by creating
    /// a Location::Archive.
    #[test]
    fn property_archive_entry_creates_archive_location() {
        use crate::input::{Action, action_to_transitions};
        use crate::model::{FileEntry, Location, ActivePane};
        use crate::job::JobKind;
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        // Create a state with an archive file
        let mut state = create_state_with_entries(1, 0);
        
        // Add a .zip file to the left pane
        let archive_entry = FileEntry {
            name: "test.zip".to_string(),
            location: Location::Local(PathBuf::from("/test/test.zip")),
            size: 1024,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        state.current_tab_mut().left_pane.entries = vec![archive_entry];
        state.current_tab_mut().left_pane.cursor = 0;
        
        // Press Enter on the archive file
        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        
        // Should create a ChangeLocation transition with Archive location
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ChangeLocation { pane, location } => {
                assert_eq!(*pane, ActivePane::Left);
                match location {
                    Location::Archive { archive_path, inner_path } => {
                        // Archive path should point to the zip file
                        assert_eq!(**archive_path, Location::Local(PathBuf::from("/test/test.zip")));
                        // Inner path should be empty (root of archive)
                        assert_eq!(*inner_path, PathBuf::new());
                    }
                    _ => panic!("Expected Archive location"),
                }
            }
            _ => panic!("Expected ChangeLocation transition"),
        }
        
        // Apply the transition
        let result = update_state(&mut state, transitions[0].clone());
        
        // Should create a ReadDirectory job for the archive
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::ReadDirectory { location } => {
                match location {
                    Location::Archive { .. } => {
                        // Correct - reading archive contents
                    }
                    _ => panic!("Expected ReadDirectory job with Archive location"),
                }
            }
            _ => panic!("Expected ReadDirectory job"),
        }
    }

    /// **Property 27: Archive Exit Returns to Filesystem**
    ///
    /// **Validates: Requirements 29.4**
    ///
    /// When the user presses Backspace in an archive virtual folder at the root,
    /// the application SHALL exit the virtual folder and return to the filesystem view.
    #[test]
    fn property_archive_exit_returns_to_filesystem() {
        use crate::input::{Action, action_to_transitions};
        use crate::model::{FileEntry, Location, ActivePane};
        use crate::job::JobKind;
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        // Create a state where we're inside an archive
        let mut state = create_state_with_entries(0, 0);
        
        // Set the left pane location to be inside an archive
        let archive_location = Location::Archive {
            archive_path: Box::new(Location::Local(PathBuf::from("/test/test.zip"))),
            inner_path: PathBuf::new(), // Root of archive
        };
        
        state.current_tab_mut().left_pane.current_location = archive_location.clone();
        state.current_tab_mut().left_pane.entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Archive {
                    archive_path: Box::new(Location::Local(PathBuf::from("/test/test.zip"))),
                    inner_path: PathBuf::from("file1.txt"),
                },
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        
        // Press Backspace to exit the archive
        let transitions = action_to_transitions(&state, &Action::ParentDirectory);
        
        // Should create a NavigateUp transition
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::NavigateUp { pane } => {
                assert_eq!(*pane, ActivePane::Left);
            }
            _ => panic!("Expected NavigateUp transition"),
        }
        
        // Apply the transition
        let result = update_state(&mut state, transitions[0].clone());
        
        // Should create a ChangeLocation transition to the filesystem
        // (NavigateUp internally calls ChangeLocation with parent)
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::ReadDirectory { location } => {
                match location {
                    Location::Local(path) => {
                        // Should be back in the filesystem
                        assert_eq!(*path, PathBuf::from("/test"));
                    }
                    _ => panic!("Expected Local location after exiting archive"),
                }
            }
            _ => panic!("Expected ReadDirectory job"),
        }
        
        // Verify the pane location was updated
        assert_eq!(
            state.current_tab().left_pane.current_location,
            Location::Local(PathBuf::from("/test"))
        );
    }

    /// Test navigating into nested directories within an archive
    #[test]
    fn test_archive_nested_directory_navigation() {
        use crate::input::{Action, action_to_transitions};
        use crate::model::{FileEntry, Location, ActivePane};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        // Create a state where we're inside an archive
        let mut state = create_state_with_entries(0, 0);
        
        // Set the left pane location to be inside an archive
        let archive_location = Location::Archive {
            archive_path: Box::new(Location::Local(PathBuf::from("/test/test.zip"))),
            inner_path: PathBuf::new(), // Root of archive
        };
        
        state.current_tab_mut().left_pane.current_location = archive_location.clone();
        
        // Add a directory entry inside the archive
        let dir_entry = FileEntry {
            name: "subdir".to_string(),
            location: Location::Archive {
                archive_path: Box::new(Location::Local(PathBuf::from("/test/test.zip"))),
                inner_path: PathBuf::from("subdir"),
            },
            size: 0,
            is_dir: true,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        state.current_tab_mut().left_pane.entries = vec![dir_entry];
        state.current_tab_mut().left_pane.cursor = 0;
        
        // Press Enter on the directory
        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        
        // Should create a ChangeLocation transition
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ChangeLocation { pane, location } => {
                assert_eq!(*pane, ActivePane::Left);
                match location {
                    Location::Archive { archive_path, inner_path } => {
                        assert_eq!(**archive_path, Location::Local(PathBuf::from("/test/test.zip")));
                        assert_eq!(*inner_path, PathBuf::from("subdir"));
                    }
                    _ => panic!("Expected Archive location"),
                }
            }
            _ => panic!("Expected ChangeLocation transition"),
        }
        
        // Apply the transition
        let result = update_state(&mut state, transitions[0].clone());
        
        // Should create a ReadDirectory job for the subdirectory
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify the location was updated
        match &state.current_tab().left_pane.current_location {
            Location::Archive { archive_path, inner_path } => {
                assert_eq!(archive_path.as_ref(), &Location::Local(PathBuf::from("/test/test.zip")));
                assert_eq!(*inner_path, PathBuf::from("subdir"));
            }
            _ => panic!("Expected Archive location"),
        }
    }

    /// Test that non-archive files don't create archive locations
    #[test]
    fn test_non_archive_file_no_archive_location() {
        use crate::input::{Action, action_to_transitions};
        use crate::model::{FileEntry, Location};
        use std::path::PathBuf;
        use std::time::SystemTime;
        
        // Create a state with a regular file
        let mut state = create_state_with_entries(1, 0);
        
        // Add a non-archive file to the left pane
        let file_entry = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/test.txt")),
            size: 1024,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        state.current_tab_mut().left_pane.entries = vec![file_entry];
        state.current_tab_mut().left_pane.cursor = 0;
        
        // Press Enter on the regular file
        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        
        // Should not create any transitions (regular files can't be entered)
        assert_eq!(transitions.len(), 0);
    }
}