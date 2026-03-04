//! Comprehensive edge case property-based tests
//!
//! This module contains additional property tests that verify edge cases,
//! boundary conditions, error conditions, and concurrent operations.

#[cfg(test)]
mod tests {
    use crate::job::{JobManager, JobSpec, JobKind};
    use crate::model::{Location, FileEntry, MarkingModel, ActivePane};
    use crate::state::{AppState, Transition, update_state};
    use crate::config::AppConfig;
    use proptest::prelude::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    // ============================================================================
    // Boundary Condition Tests
    // ============================================================================

    /// **Property: Empty Pane Operations**
    ///
    /// All cursor operations on an empty pane should maintain cursor at 0
    /// and not panic or produce invalid state.
    #[test]
    fn prop_empty_pane_operations() {
        proptest!(|(operations in prop::collection::vec(0u8..10, 0..20))| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Pane is empty, cursor should be 0
            let tab = state.current_tab();
            prop_assert_eq!(tab.left_pane.cursor, 0);
            
            // Apply various cursor operations
            for op in operations {
                let transition = match op % 5 {
                    0 => Transition::CursorMove { pane: ActivePane::Left, delta: 1 },
                    1 => Transition::CursorMove { pane: ActivePane::Left, delta: -1 },
                    2 => Transition::CursorJump { pane: ActivePane::Left, position: 0 },
                    3 => Transition::CursorJump { pane: ActivePane::Left, position: 100 },
                    4 => Transition::CursorMove { pane: ActivePane::Left, delta: 10 },
                    _ => Transition::CursorMove { pane: ActivePane::Left, delta: 1 },
                };
                
                let _ = update_state(&mut state, transition);
                
                // Cursor should always remain at 0 for empty pane
                let tab = state.current_tab();
                prop_assert_eq!(tab.left_pane.cursor, 0, "Cursor should remain at 0 for empty pane");
            }
        });
    }

    /// **Property: Single Entry Pane Operations**
    ///
    /// For a pane with exactly one entry, cursor should always be 0
    /// regardless of operations.
    #[test]
    fn prop_single_entry_pane_operations() {
        proptest!(|(operations in prop::collection::vec(0u8..10, 0..20))| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Add single entry
            state.current_tab_mut().left_pane.entries = vec![FileEntry {
                name: "file.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }];
            
            for op in operations {
                let transition = match op % 5 {
                    0 => Transition::CursorMove { pane: ActivePane::Left, delta: 1 },
                    1 => Transition::CursorMove { pane: ActivePane::Left, delta: -1 },
                    2 => Transition::CursorJump { pane: ActivePane::Left, position: 0 },
                    3 => Transition::CursorJump { pane: ActivePane::Left, position: 100 },
                    4 => Transition::CursorMove { pane: ActivePane::Left, delta: 10 },
                    _ => Transition::CursorMove { pane: ActivePane::Left, delta: 1 },
                };
                
                let _ = update_state(&mut state, transition);
                
                // Cursor should always be 0 for single-entry pane
                let tab = state.current_tab();
                prop_assert_eq!(tab.left_pane.cursor, 0, "Cursor should remain at 0 for single-entry pane");
            }
        });
    }

    /// **Property: Maximum Cursor Position**
    ///
    /// Cursor should never exceed entries.len() - 1, even with
    /// repeated down/page_down operations.
    #[test]
    fn prop_cursor_never_exceeds_max() {
        proptest!(|(
            entry_count in 1usize..100,
            down_operations in 0usize..200
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create entries
            let entries: Vec<FileEntry> = (0..entry_count).map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Perform many down operations
            for _ in 0..down_operations {
                let _ = update_state(&mut state, Transition::CursorMove { 
                    pane: ActivePane::Left, 
                    delta: 1 
                });
            }
            
            // Cursor should be at most entries.len() - 1
            let tab = state.current_tab();
            prop_assert!(tab.left_pane.cursor < entry_count, 
                "Cursor {} should be less than entry count {}", tab.left_pane.cursor, entry_count);
        });
    }

    /// **Property: Minimum Cursor Position**
    ///
    /// Cursor should never be negative (< 0), even with
    /// repeated up/page_up operations.
    #[test]
    fn prop_cursor_never_negative() {
        proptest!(|(
            entry_count in 1usize..100,
            up_operations in 0usize..200
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create entries
            let entries: Vec<FileEntry> = (0..entry_count).map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Start at end
            let _ = update_state(&mut state, Transition::CursorJump { 
                pane: ActivePane::Left, 
                position: entry_count - 1 
            });
            
            // Perform many up operations
            for _ in 0..up_operations {
                let _ = update_state(&mut state, Transition::CursorMove { 
                    pane: ActivePane::Left, 
                    delta: -1 
                });
            }
            
            // Cursor should be at 0 after enough up operations (usize can't be negative)
            let tab = state.current_tab();
            prop_assert!(tab.left_pane.cursor <= entry_count - 1, 
                "Cursor {} should be within bounds (0..{})", tab.left_pane.cursor, entry_count);
            
            // If we did more up operations than the entry count, cursor should be at 0
            if up_operations >= entry_count {
                prop_assert_eq!(tab.left_pane.cursor, 0, 
                    "Cursor should be 0 after {} up operations from position {}", 
                    up_operations, entry_count - 1);
            }
        });
    }

    // ============================================================================
    // Error Condition Tests
    // ============================================================================

    /// **Property: Job Cancellation Idempotence**
    ///
    /// Cancelling the same job multiple times should be safe and idempotent.
    #[test]
    fn prop_job_cancellation_idempotence() {
        proptest!(|(cancel_count in 1usize..10)| {
            let mut manager = JobManager::new(4);
            
            // Enqueue a job
            let id = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                location: Location::Local(PathBuf::from("/test")),
            }));
            
            // Cancel the job multiple times
            let mut results = Vec::new();
            for _ in 0..cancel_count {
                results.push(manager.request_cancel(id));
            }
            
            // First cancellation should succeed, rest should fail
            prop_assert!(results[0], "First cancellation should succeed");
            for i in 1..cancel_count {
                prop_assert!(!results[i], "Subsequent cancellations should fail");
            }
        });
    }

    /// **Property: Invalid Job ID Cancellation**
    ///
    /// Attempting to cancel a non-existent job should return false
    /// and not cause any state corruption.
    #[test]
    fn prop_invalid_job_cancellation() {
        proptest!(|(_invalid_id in 1000u64..10000u64)| {
            let mut manager = JobManager::new(4);
            
            // Enqueue a few jobs
            for i in 0..5 {
                manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                    location: Location::Local(PathBuf::from(format!("/test{}", i))),
                }));
            }
            
            let initial_queue_len = manager.queue.len();
            
            // Try to cancel a non-existent job (generate a random UUID that won't match)
            let result = manager.request_cancel(crate::job::JobId::new());
            
            // Should return false
            prop_assert!(!result, "Cancelling invalid job should return false");
            
            // Queue should be unchanged
            prop_assert_eq!(manager.queue.len(), initial_queue_len, 
                "Queue length should be unchanged");
        });
    }

    /// **Property: Marking Non-Existent Files**
    ///
    /// Marking and unmarking non-existent locations should not cause errors.
    #[test]
    fn prop_marking_nonexistent_files() {
        proptest!(|(paths in prop::collection::vec("[a-z]{3,10}", 1..20))| {
            let mut marking = MarkingModel::new();
            
            // Mark non-existent locations
            for path in &paths {
                let location = Location::Local(PathBuf::from(format!("/nonexistent/{}", path)));
                marking.mark(location.clone());
            }
            
            // Count should match
            prop_assert_eq!(marking.count(), paths.len());
            
            // Unmark all
            marking.unmark_all();
            
            // Count should be 0
            prop_assert_eq!(marking.count(), 0);
        });
    }

    // ============================================================================
    // Concurrent Operation Tests
    // ============================================================================

    /// **Property: Multiple Job Enqueue/Dequeue**
    ///
    /// Rapidly enqueueing and dequeueing jobs should maintain FIFO order
    /// and not lose any jobs.
    #[test]
    fn prop_rapid_job_operations() {
        proptest!(|(
            enqueue_count in 1usize..50,
            dequeue_count in 0usize..50
        )| {
            let mut manager = JobManager::new(4);
            
            // Enqueue jobs
            let mut enqueued_ids = Vec::new();
            for i in 0..enqueue_count {
                let id = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                    location: Location::Local(PathBuf::from(format!("/test{}", i))),
                }));
                enqueued_ids.push(id);
            }
            
            // Dequeue jobs
            let mut dequeued_ids = Vec::new();
            for _ in 0..dequeue_count.min(enqueue_count) {
                if let Some(spec) = manager.pop_next_job() {
                    dequeued_ids.push(spec.id);
                }
            }
            
            // Dequeued IDs should match first N enqueued IDs
            let dequeued_len = dequeued_ids.len();
            prop_assert_eq!(dequeued_ids, enqueued_ids[..dequeued_len].to_vec());
            
            // Remaining queue size should be correct
            prop_assert_eq!(manager.queue.len(), enqueue_count - dequeued_len);
        });
    }

    /// **Property: Interleaved Mark/Unmark Operations**
    ///
    /// Interleaving mark and unmark operations should maintain correct state.
    #[test]
    fn prop_interleaved_marking() {
        proptest!(|(operations in prop::collection::vec((0u8..2, 0usize..10), 1..50))| {
            let mut marking = MarkingModel::new();
            let locations: Vec<_> = (0..10)
                .map(|i| Location::Local(PathBuf::from(format!("/test/file{}.txt", i))))
                .collect();
            
            // Apply operations
            for (op, idx) in operations {
                let location = &locations[idx];
                match op {
                    0 => marking.mark(location.clone()),
                    1 => marking.unmark(location.clone()),
                    _ => {}
                }
            }
            
            // Verify count matches actual marked locations
            let actual_marked: std::collections::HashSet<_> = marking.marked_locations.clone();
            prop_assert_eq!(marking.count(), actual_marked.len());
            
            // Verify all marked locations are in the set
            for location in &locations {
                let is_marked = marking.is_marked(location);
                let in_set = actual_marked.contains(location);
                prop_assert_eq!(is_marked, in_set, 
                    "is_marked should match set membership");
            }
        });
    }

    /// **Property: State Transitions Under Load**
    ///
    /// Applying many state transitions rapidly should maintain consistency.
    #[test]
    fn prop_rapid_state_transitions() {
        proptest!(|(transitions in prop::collection::vec(0u8..5, 1..30))| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Initialize with some entries
            let entries: Vec<FileEntry> = (0..10).map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Apply transitions
            for t in transitions {
                let transition = match t % 5 {
                    0 => Transition::CursorMove { pane: ActivePane::Left, delta: 1 },
                    1 => Transition::CursorMove { pane: ActivePane::Left, delta: -1 },
                    2 => Transition::SwitchPane,
                    3 => Transition::ToggleMark { 
                        location: state.current_tab().left_pane.entries[0].location.clone() 
                    },
                    4 => Transition::CursorJump { pane: ActivePane::Left, position: 0 },
                    _ => Transition::CursorMove { pane: ActivePane::Left, delta: 1 },
                };
                
                let _ = update_state(&mut state, transition);
            }
            
            // Verify invariants
            let tab = state.current_tab();
            prop_assert!(tab.left_pane.cursor < tab.left_pane.entries.len().max(1),
                "Left pane cursor should be within bounds");
            prop_assert!(tab.right_pane.cursor < tab.right_pane.entries.len().max(1),
                "Right pane cursor should be within bounds");
        });
    }

    // ============================================================================
    // Extreme Value Tests
    // ============================================================================

    /// **Property: Very Large File Counts**
    ///
    /// Operations should work correctly with very large numbers of files.
    #[test]
    fn prop_large_file_count() {
        proptest!(|(entry_count in 1000usize..5000)| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create many entries
            let entries: Vec<FileEntry> = (0..entry_count).map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Operations should still work
            let _ = update_state(&mut state, Transition::CursorJump { 
                pane: ActivePane::Left, 
                position: entry_count - 1 
            });
            prop_assert_eq!(state.current_tab().left_pane.cursor, entry_count - 1);
            
            let _ = update_state(&mut state, Transition::CursorJump { 
                pane: ActivePane::Left, 
                position: 0 
            });
            prop_assert_eq!(state.current_tab().left_pane.cursor, 0);
        });
    }

    /// **Property: Very Large Job Queue**
    ///
    /// Job manager should handle very large queues correctly.
    #[test]
    fn prop_large_job_queue() {
        proptest!(|(job_count in 100usize..500)| {
            let mut manager = JobManager::new(4);
            
            // Enqueue many jobs
            let mut ids = Vec::new();
            for i in 0..job_count {
                let id = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                    location: Location::Local(PathBuf::from(format!("/test{}", i))),
                }));
                ids.push(id);
            }
            
            // Queue size should match
            prop_assert_eq!(manager.queue.len(), job_count);
            
            // Dequeue all and verify order
            let mut dequeued = Vec::new();
            while let Some(spec) = manager.pop_next_job() {
                dequeued.push(spec.id);
            }
            
            prop_assert_eq!(dequeued, ids, "FIFO order should be maintained");
        });
    }

    /// **Property: Zero-Size Files**
    ///
    /// Operations should handle zero-size files correctly.
    #[test]
    fn prop_zero_size_files() {
        proptest!(|(file_count in 1usize..20)| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create zero-size entries
            let entries: Vec<FileEntry> = (0..file_count).map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
                size: 0,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries.clone();
            
            // Mark all
            for entry in &entries {
                let _ = update_state(&mut state, Transition::ToggleMark { 
                    location: entry.location.clone() 
                });
            }
            
            // Total size should be 0
            let total = state.marking.total_size(&state.current_tab().left_pane.entries);
            prop_assert_eq!(total, 0, "Total size of zero-size files should be 0");
        });
    }

    // ============================================================================
    // Tab Management Edge Cases
    // ============================================================================

    /// **Property: Cannot Close Last Tab**
    ///
    /// Attempting to close the last remaining tab should fail gracefully.
    #[test]
    fn prop_cannot_close_last_tab() {
        proptest!(|(close_attempts in 1usize..10)| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Should start with 1 tab
            prop_assert_eq!(state.tabs.tabs.len(), 1);
            
            // Try to close tab multiple times
            for _ in 0..close_attempts {
                let _ = update_state(&mut state, Transition::CloseTab { index: 0 });
            }
            
            // Should still have 1 tab
            prop_assert_eq!(state.tabs.tabs.len(), 1, "Last tab should not be closeable");
        });
    }

    /// **Property: Tab Index Bounds After Closure**
    ///
    /// Active tab index should remain valid after closing tabs.
    #[test]
    fn prop_tab_index_valid_after_closure() {
        proptest!(|(
            initial_tabs in 2usize..10,
            close_index in 0usize..9
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create multiple tabs
            for _ in 1..initial_tabs {
                let _ = update_state(&mut state, Transition::CreateTab);
            }
            
            prop_assert_eq!(state.tabs.tabs.len(), initial_tabs);
            
            // Close a tab if index is valid
            if close_index < initial_tabs && initial_tabs > 1 {
                let _ = update_state(&mut state, Transition::CloseTab { index: close_index });
                
                // Active index should be valid
                prop_assert!(state.tabs.active_index < state.tabs.tabs.len(),
                    "Active index {} should be less than tab count {}", 
                    state.tabs.active_index, state.tabs.tabs.len());
            }
        });
    }

    /// **Property: Tab Switching Wraps Around**
    ///
    /// Switching tabs should wrap around at boundaries.
    #[test]
    fn prop_tab_switching_wraps() {
        proptest!(|(tab_count in 2usize..10)| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create tabs
            for _ in 1..tab_count {
                let _ = update_state(&mut state, Transition::CreateTab);
            }
            
            // Switch to last tab
            state.tabs.active_index = tab_count - 1;
            
            // Switch to next (should wrap to 0)
            state.tabs.switch_to_next();
            prop_assert_eq!(state.tabs.active_index, 0, "Should wrap to first tab");
            
            // Switch to previous (should wrap to last)
            state.tabs.switch_to_prev();
            prop_assert_eq!(state.tabs.active_index, tab_count - 1, "Should wrap to last tab");
        });
    }

    // ============================================================================
    // Marking Edge Cases
    // ============================================================================

    /// **Property: Marking Empty Pane**
    ///
    /// Mark all on empty pane should not cause errors.
    #[test]
    fn prop_mark_all_empty_pane() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Pane is empty
        assert_eq!(state.current_tab().left_pane.entries.len(), 0);
        
        // Mark all should not panic
        let _ = update_state(&mut state, Transition::MarkAll);
        
        // Count should be 0
        assert_eq!(state.marking.count(), 0);
    }

    /// **Property: Unmark All Idempotence**
    ///
    /// Calling unmark all multiple times should be safe.
    #[test]
    fn prop_unmark_all_idempotence() {
        proptest!(|(
            file_count in 1usize..20,
            unmark_count in 1usize..10
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create and mark files
            let entries: Vec<FileEntry> = (0..file_count).map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries;
            let _ = update_state(&mut state, Transition::MarkAll);
            
            // Unmark all multiple times
            for _ in 0..unmark_count {
                let _ = update_state(&mut state, Transition::UnmarkAll);
            }
            
            // Count should be 0
            prop_assert_eq!(state.marking.count(), 0);
        });
    }

    /// **Property: Toggle Mark Consistency**
    ///
    /// Toggling mark twice should return to original state.
    #[test]
    fn prop_toggle_mark_twice() {
        proptest!(|(file_count in 1usize..20)| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create files
            let entries: Vec<FileEntry> = (0..file_count).map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries.clone();
            
            // Toggle each file twice
            for entry in &entries {
                let location = entry.location.clone();
                let _ = update_state(&mut state, Transition::ToggleMark { location: location.clone() });
                let _ = update_state(&mut state, Transition::ToggleMark { location });
            }
            
            // All should be unmarked
            prop_assert_eq!(state.marking.count(), 0, "All files should be unmarked after double toggle");
        });
    }

    // ============================================================================
    // Job Queue Edge Cases
    // ============================================================================

    /// **Property: Job Completion Removes From Active**
    ///
    /// Completing a job should remove it from active jobs.
    #[test]
    fn prop_job_completion_cleanup() {
        proptest!(|(job_count in 1usize..20)| {
            let mut manager = JobManager::new(4);
            
            // Enqueue and start jobs
            let mut ids = Vec::new();
            for i in 0..job_count {
                let id = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                    location: Location::Local(PathBuf::from(format!("/test{}", i))),
                }));
                ids.push(id);
                
                if manager.can_start_job() {
                    if let Some(spec) = manager.pop_next_job() {
                        manager.start_job(spec);
                    }
                }
            }
            
            let active_count = manager.active.len();
            
            // Complete all active jobs
            let active_ids: Vec<_> = manager.active.keys().copied().collect();
            for id in active_ids {
                manager.complete_job(id, crate::job::OpResult::Success(
                    crate::job::SuccessData::None
                ));
            }
            
            // Active should be empty
            prop_assert_eq!(manager.active.len(), 0, "Active jobs should be empty after completion");
            
            // Completed should have the jobs
            prop_assert!(manager.completed.len() >= active_count, 
                "Completed should contain at least {} jobs", active_count);
        });
    }

    /// **Property: Max Parallel Jobs Enforced**
    ///
    /// Active jobs should never exceed max_parallel limit.
    #[test]
    fn prop_max_parallel_enforced() {
        proptest!(|(
            max_parallel in 1usize..8,
            job_count in 10usize..50
        )| {
            let mut manager = JobManager::new(max_parallel);
            
            // Enqueue many jobs
            for i in 0..job_count {
                manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                    location: Location::Local(PathBuf::from(format!("/test{}", i))),
                }));
            }
            
            // Start as many as possible
            while manager.can_start_job() {
                if let Some(spec) = manager.pop_next_job() {
                    manager.start_job(spec);
                }
            }
            
            // Active count should not exceed max_parallel
            prop_assert!(manager.active.len() <= max_parallel,
                "Active jobs {} should not exceed max_parallel {}", 
                manager.active.len(), max_parallel);
        });
    }

    /// **Property: Completed Jobs History Limit**
    ///
    /// Completed jobs should be limited to prevent unbounded growth.
    #[test]
    fn prop_completed_jobs_limited() {
        proptest!(|(job_count in 150usize..200)| {
            let mut manager = JobManager::new(4);
            
            // Complete many jobs
            for i in 0..job_count {
                let id = manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                    location: Location::Local(PathBuf::from(format!("/test{}", i))),
                }));
                
                if let Some(spec) = manager.pop_next_job() {
                    manager.start_job(spec);
                    manager.complete_job(id, crate::job::OpResult::Success(
                        crate::job::SuccessData::None
                    ));
                }
            }
            
            // Completed should be limited to 100
            prop_assert!(manager.completed.len() <= 100,
                "Completed jobs {} should not exceed 100", manager.completed.len());
        });
    }

    // ============================================================================
    // Directory Navigation Edge Cases
    // ============================================================================

    /// **Property: Parent Navigation At Root**
    ///
    /// Attempting to navigate to parent at root should not cause errors.
    #[test]
    fn prop_parent_at_root() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set location to root
        state.current_tab_mut().left_pane.current_location = Location::Local(PathBuf::from("/"));
        
        // Try to navigate up (parent)
        let _ = update_state(&mut state, Transition::NavigateUp { pane: ActivePane::Left });
        
        // Should still be at root (or handle gracefully)
        // The exact behavior depends on implementation, but should not panic
    }

    /// **Property: Rapid Pane Switching**
    ///
    /// Rapidly switching panes should maintain correct active pane.
    #[test]
    fn prop_rapid_pane_switching() {
        proptest!(|(switch_count in 1usize..100)| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            let initial_pane = state.ui.active_pane;
            
            // Switch many times
            for _ in 0..switch_count {
                let _ = update_state(&mut state, Transition::SwitchPane);
            }
            
            // Parity should match
            let expected_pane = if switch_count % 2 == 0 {
                initial_pane
            } else {
                initial_pane.opposite()
            };
            
            prop_assert_eq!(state.ui.active_pane, expected_pane,
                "Active pane should match expected after {} switches", switch_count);
        });
    }

    // ============================================================================
    // Scroll Position Edge Cases
    // ============================================================================

    /// **Property: Scroll Offset Bounds**
    ///
    /// Scroll offset should never cause cursor to be invisible.
    #[test]
    fn prop_scroll_offset_valid() {
        proptest!(|(
            entry_count in 10usize..100,
            cursor_pos in 0usize..99
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Create entries
            let entries: Vec<FileEntry> = (0..entry_count).map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }).collect();
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Move cursor
            let actual_pos = cursor_pos.min(entry_count - 1);
            let _ = update_state(&mut state, Transition::CursorJump { 
                pane: ActivePane::Left, 
                position: actual_pos 
            });
            
            let pane = &state.current_tab().left_pane;
            
            // Cursor should be within scroll window
            // (This assumes a reasonable page size, actual implementation may vary)
            prop_assert!(pane.cursor >= pane.scroll_offset,
                "Cursor {} should be >= scroll_offset {}", pane.cursor, pane.scroll_offset);
        });
    }

    // ============================================================================
    // State Consistency Under Errors
    // ============================================================================

    /// **Property: State Remains Valid After Failed Transitions**
    ///
    /// Even if transitions fail, state should remain consistent.
    #[test]
    fn prop_state_valid_after_failures() {
        proptest!(|(operations in prop::collection::vec(0u8..9, 1..30))| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Apply various transitions that might fail
            for op in operations {
                let transition = match op % 9 {
                    0 => Transition::CursorMove { pane: ActivePane::Left, delta: 1 },
                    1 => Transition::CursorMove { pane: ActivePane::Left, delta: -1 },
                    2 => Transition::SwitchPane,
                    3 => Transition::CloseTab { index: 0 }, // Should fail (last tab)
                    4 => Transition::MarkAll,
                    5 => Transition::UnmarkAll,
                    6 => Transition::CursorJump { pane: ActivePane::Left, position: 0 },
                    7 => Transition::NavigateUp { pane: ActivePane::Left },
                    8 => Transition::CreateTab,
                    _ => Transition::SwitchPane,
                };
                
                let _ = update_state(&mut state, transition);
            }
            
            // Verify basic invariants
            prop_assert!(state.tabs.tabs.len() >= 1, "Should have at least one tab");
            prop_assert!(state.tabs.active_index < state.tabs.tabs.len(), 
                "Active tab index should be valid");
            
            let tab = state.current_tab();
            prop_assert!(tab.left_pane.cursor < tab.left_pane.entries.len().max(1),
                "Left cursor should be valid");
            prop_assert!(tab.right_pane.cursor < tab.right_pane.entries.len().max(1),
                "Right cursor should be valid");
        });
    }
}
