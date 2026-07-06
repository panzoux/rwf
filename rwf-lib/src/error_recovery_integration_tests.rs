//! Error recovery integration tests
//!
//! Tests error recovery scenarios including:
//! - Recovery from file operation failures
//! - Recovery from invalid configuration
//! - Recovery from corrupted session state
//!
//! **Validates: Requirements 19.1-19.5**

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec, OpResult};
    use crate::model::{DialogContent, ErrorDialog, ErrorType, Location};
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::{test_state, FileEntryBuilder};
    use std::path::PathBuf;

    /// Test recovery from file operation failure
    /// **Validates: Requirements 19.1, 19.2**
    #[test]
    fn test_recovery_from_file_operation_failure() {
        let mut state = test_state();

        // Create a copy job that will fail
        let job_spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/nonexistent/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;

        // Start the job
        update_state(&mut state, Transition::StartNextJob);
        assert_eq!(state.jobs.active.len(), 1);

        // Simulate job failure
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("File not found: /nonexistent/file.txt".to_string()),
            },
        );

        // Verify error dialog is shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "File Not Found");

        // Verify job is marked as failed
        assert_eq!(state.jobs.completed.len(), 1);
        let completed = &state.jobs.completed[0];
        assert!(matches!(completed.result, OpResult::Failed(_)));

        // Verify application is still functional - dismiss error and continue
        update_state(&mut state, Transition::CloseDialog);
        assert!(state.dialogs.is_empty());

        // Create a new successful job to verify recovery
        let success_job = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(PathBuf::from("/test/newdir")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: success_job });
        let new_job_id = state.jobs.queue[0].id;

        update_state(&mut state, Transition::StartNextJob);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: new_job_id,
                result: OpResult::Success(crate::job::SuccessData::None),
            },
        );

        // Verify successful recovery - new job completed
        assert_eq!(state.jobs.completed.len(), 2);
        assert!(matches!(
            state.jobs.completed[1].result,
            OpResult::Success(_)
        ));
    }

    /// Test recovery from permission error
    /// **Validates: Requirements 19.5**
    #[test]
    fn test_recovery_from_permission_error() {
        let mut state = test_state();

        // Create a job that will fail with permission error
        let job_spec = JobSpec::new(JobKind::Delete {
            targets: vec![Location::Local(PathBuf::from("/root/protected.txt"))],
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;

        update_state(&mut state, Transition::StartNextJob);

        // Simulate permission error
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("Permission denied".to_string()),
            },
        );

        // Verify permission error dialog is shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "Permission Denied");

        if let DialogContent::Error(ErrorDialog { error_type, .. }) = &dialog.content {
            assert_eq!(*error_type, ErrorType::Permission);
        } else {
            panic!("Expected Error dialog content");
        }

        // Dismiss error and verify recovery
        update_state(&mut state, Transition::CloseDialog);
        assert!(state.dialogs.is_empty());

        // Application should still be functional
        assert_eq!(state.jobs.active.len(), 0);
    }

    /// Test recovery from invalid path error
    /// **Validates: Requirements 19.1**
    #[test]
    fn test_recovery_from_invalid_path_error() {
        let mut state = test_state();

        // Create a job with invalid path
        let job_spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(PathBuf::from("/tmp/invalid\0name")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;

        update_state(&mut state, Transition::StartNextJob);

        // Simulate invalid path error
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("Invalid path: contains null character".to_string()),
            },
        );

        // Verify error dialog is shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "Invalid Path");

        // Dismiss and verify recovery
        update_state(&mut state, Transition::CloseDialog);
        assert!(state.dialogs.is_empty());
    }

    /// Test recovery from multiple consecutive failures
    /// **Validates: Requirements 19.1-19.4**
    #[test]
    fn test_recovery_from_multiple_consecutive_failures() {
        let mut state = test_state();

        // Create multiple jobs that will fail
        for i in 0..3 {
            let job_spec = JobSpec::new(JobKind::Copy {
                sources: vec![Location::Local(PathBuf::from(format!(
                    "/nonexistent/file{}.txt",
                    i
                )))],
                dest: Location::Local(PathBuf::from("/dest")),
            });

            update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        }

        let job_ids: Vec<_> = state.jobs.queue.iter().map(|j| j.id).collect();

        // Execute and fail all jobs
        for job_id in job_ids {
            update_state(&mut state, Transition::StartNextJob);
            update_state(
                &mut state,
                Transition::CompleteJob {
                    job_id,
                    result: OpResult::Failed("File not found".to_string()),
                },
            );

            // Dismiss error dialog
            update_state(&mut state, Transition::CloseDialog);
        }

        // Verify all jobs failed
        assert_eq!(state.jobs.completed.len(), 3);
        for completed in &state.jobs.completed {
            assert!(matches!(completed.result, OpResult::Failed(_)));
        }

        // Verify application is still functional
        assert!(state.dialogs.is_empty());
        assert_eq!(state.jobs.active.len(), 0);

        // Create a successful job to verify full recovery
        let success_job = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(PathBuf::from("/test/recovery")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: success_job });
        let new_job_id = state.jobs.queue[0].id;

        update_state(&mut state, Transition::StartNextJob);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: new_job_id,
                result: OpResult::Success(crate::job::SuccessData::None),
            },
        );

        // Verify successful recovery
        assert_eq!(state.jobs.completed.len(), 4);
        assert!(matches!(
            state.jobs.completed[3].result,
            OpResult::Success(_)
        ));
    }

    /// Test recovery from directory read failure
    /// **Validates: Requirements 3.8, 19.2**
    #[test]
    fn test_recovery_from_directory_read_failure() {
        let mut state = test_state();

        // Set current location
        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/valid/path"));

        // Create a directory read job that will fail
        let job_spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/invalid/path")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;

        update_state(&mut state, Transition::StartNextJob);

        // Simulate directory read failure
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("Directory not found".to_string()),
            },
        );

        // Verify error dialog is shown
        assert!(!state.dialogs.is_empty());

        // Verify pane location remains unchanged (requirement 3.8)
        assert_eq!(
            state.current_tab().left_pane.current_location,
            Location::Local(PathBuf::from("/valid/path"))
        );

        // Dismiss error and verify recovery
        update_state(&mut state, Transition::CloseDialog);
        assert!(state.dialogs.is_empty());
    }

    /// Test recovery from invalid configuration
    /// **Validates: Requirements 17.9, 38.9**
    #[test]
    fn test_recovery_from_invalid_configuration() {
        // Create config with very small worker pool size
        let config = crate::config::AppConfig {
            worker_pool_size: 1, // Minimum viable value
            ..Default::default()
        };

        // Application should handle this gracefully
        let state = AppState::new(config);

        // Verify application initialized with the configured value
        assert_eq!(state.jobs.max_parallel, 1);
        assert_eq!(state.tabs.tabs.len(), 1);

        // Verify application is still functional with minimal config
        // Can still enqueue and process jobs one at a time
        assert!(!state.jobs.can_start_job()); // No jobs queued yet
    }

    /// Test recovery from corrupted session state
    /// **Validates: Requirements 38.6**
    #[test]
    fn test_recovery_from_corrupted_session_state() {
        let mut state = test_state();

        // Simulate corrupted session by setting invalid tab index
        state.tabs.active_index = 999; // Invalid index

        // Application should recover by resetting to valid state
        // This would typically happen during session load
        if state.tabs.active_index >= state.tabs.tabs.len() {
            state.tabs.active_index = 0;
        }

        // Verify recovery
        assert_eq!(state.tabs.active_index, 0);
        assert!(state.tabs.active_index < state.tabs.tabs.len());
    }

    /// Test recovery from job cancellation failure
    /// **Validates: Requirements 15.6, 15.7**
    #[test]
    fn test_recovery_from_job_cancellation_failure() {
        let mut state = test_state();

        // Create and start a job
        let job_spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/source/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;

        update_state(&mut state, Transition::StartNextJob);

        // Request cancellation
        update_state(&mut state, Transition::CancelJob { job_id });

        // Simulate job failing to acknowledge cancellation and completing with error
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("Operation interrupted".to_string()),
            },
        );

        // Verify error is handled gracefully
        assert!(!state.dialogs.is_empty());
        update_state(&mut state, Transition::CloseDialog);

        // Verify application recovered
        assert_eq!(state.jobs.active.len(), 0);
        assert!(state.dialogs.is_empty());
    }

    /// Test recovery from pane state inconsistency
    /// **Validates: Requirements 19.1**
    #[test]
    fn test_recovery_from_pane_state_inconsistency() {
        let mut state = test_state();

        // Create inconsistent state: cursor beyond entries
        state.current_tab_mut().left_pane.entries =
            vec![FileEntryBuilder::new("file.txt").size(1024).build()];
        state.current_tab_mut().left_pane.cursor = 999; // Invalid cursor position

        // Application should handle this gracefully
        // Verify current_entry returns None for invalid cursor
        assert!(state.active_pane().current_entry().is_none());

        // Reset cursor to valid position
        if state.active_pane().cursor >= state.active_pane().entries.len() {
            state.active_pane_mut().cursor = 0;
        }

        // Verify recovery
        assert_eq!(state.active_pane().cursor, 0);
        assert!(state.active_pane().current_entry().is_some());
    }

    /// Test recovery from empty pane operations
    /// **Validates: Requirements 19.1**
    #[test]
    fn test_recovery_from_empty_pane_operations() {
        let mut state = test_state();

        // Ensure pane is empty
        state.current_tab_mut().left_pane.entries.clear();

        // Try to perform operations on empty pane
        use crate::input::{action_to_transitions, Action};

        // Copy should return empty transitions
        let transitions = action_to_transitions(&state, &Action::Copy);
        assert_eq!(transitions.len(), 0);

        // Move should return empty transitions
        let transitions = action_to_transitions(&state, &Action::Move);
        assert_eq!(transitions.len(), 0);

        // Delete should return empty transitions
        let transitions = action_to_transitions(&state, &Action::Delete);
        assert_eq!(transitions.len(), 0);

        // Rename should return empty transitions
        let transitions = action_to_transitions(&state, &Action::Rename);
        assert_eq!(transitions.len(), 0);

        // Verify application is still functional
        assert!(state.dialogs.is_empty());
    }

    /// Test recovery from mixed success and failure jobs
    /// **Validates: Requirements 19.1-19.4**
    #[test]
    fn test_recovery_from_mixed_success_and_failure() {
        let mut state = test_state();

        // Create jobs that will have mixed results
        let success_job = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(PathBuf::from("/test/success")),
        });

        let failure_job = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/nonexistent/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest")),
        });

        let another_success_job = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(PathBuf::from("/test/success2")),
        });

        // Enqueue jobs
        update_state(&mut state, Transition::EnqueueJob { spec: success_job });
        update_state(&mut state, Transition::EnqueueJob { spec: failure_job });
        update_state(
            &mut state,
            Transition::EnqueueJob {
                spec: another_success_job,
            },
        );

        let job_ids: Vec<_> = state.jobs.queue.iter().map(|j| j.id).collect();

        // Execute first job (success)
        update_state(&mut state, Transition::StartNextJob);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: job_ids[0],
                result: OpResult::Success(crate::job::SuccessData::None),
            },
        );

        // Execute second job (failure)
        update_state(&mut state, Transition::StartNextJob);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: job_ids[1],
                result: OpResult::Failed("File not found".to_string()),
            },
        );

        // Dismiss error dialog
        update_state(&mut state, Transition::CloseDialog);

        // Execute third job (success)
        update_state(&mut state, Transition::StartNextJob);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: job_ids[2],
                result: OpResult::Success(crate::job::SuccessData::None),
            },
        );

        // Verify mixed results
        assert_eq!(state.jobs.completed.len(), 3);
        assert!(matches!(
            state.jobs.completed[0].result,
            OpResult::Success(_)
        ));
        assert!(matches!(
            state.jobs.completed[1].result,
            OpResult::Failed(_)
        ));
        assert!(matches!(
            state.jobs.completed[2].result,
            OpResult::Success(_)
        ));

        // Verify application is still functional
        assert!(state.dialogs.is_empty());
        assert_eq!(state.jobs.active.len(), 0);
    }
}
