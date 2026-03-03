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
            
            Ok(())
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
            
            Ok(())
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
            
            // Cursor should be at least 0 (usize can't be negative, but checking bounds)
            let tab = state.current_tab();
            prop_assert_eq!(tab.left_pane.cursor, 0, "Cursor should be 0 after many up operations");
            
            Ok(())
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
            
            Ok(())
        });
    }

    /// **Property: Invalid Job ID Cancellation**
    ///
    /// Attempting to cancel a non-existent job should return false
    /// and not cause any state corruption.
    #[test]
    fn prop_invalid_job_cancellation() {
        proptest!(|(invalid_id in 1000u64..10000u64)| {
            let mut manager = JobManager::new(4);
            
            // Enqueue a few jobs
            for i in 0..5 {
                manager.enqueue(JobSpec::new(JobKind::ReadDirectory {
                    location: Location::Local(PathBuf::from(format!("/test{}", i))),
                }));
            }
            
            let initial_queue_len = manager.queue.len();
            
            // Try to cancel a non-existent job
            let result = manager.request_cancel(crate::job::JobId(invalid_id));
            
            // Should return false
            prop_assert!(!result, "Cancelling invalid job should return false");
            
            // Queue should be unchanged
            prop_assert_eq!(manager.queue.len(), initial_queue_len, 
                "Queue length should be unchanged");
            
            Ok(())
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
            
            Ok(())
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
            prop_assert_eq!(dequeued_ids, enqueued_ids[..dequeued_ids.len()].to_vec());
            
            // Remaining queue size should be correct
            prop_assert_eq!(manager.queue.len(), enqueue_count - dequeued_ids.len());
            
            Ok(())
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
            
            Ok(())
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
            
            Ok(())
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
            
            Ok(())
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
            
            Ok(())
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
            
            Ok(())
        });
    }
}
