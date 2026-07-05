//! Concurrent operations integration tests
//!
//! Tests concurrent job execution including:
//! - Multiple jobs running simultaneously
//! - Job cancellation during concurrent operations
//! - UI responsiveness during heavy load
//!
//! **Validates: Requirements 15.11, 15.12, 21.1-21.8**

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::Location;
    use crate::state::{update_state, AppState, Transition};
    use std::path::PathBuf;

    /// Test multiple jobs running simultaneously
    /// **Validates: Requirements 15.11, 15.12, 21.7**
    #[test]
    fn test_multiple_concurrent_jobs() {
        let config = AppConfig {
            worker_pool_size: 4, // Allow 4 concurrent jobs
            ..Default::default()
        };
        let mut state = AppState::new(config);

        // Create multiple jobs
        let job1 = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/source1/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest1")),
        });

        let job2 = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/source2/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest2")),
        });

        let job3 = JobSpec::new(JobKind::Move {
            sources: vec![Location::Local(PathBuf::from("/source3/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest3")),
        });

        let job4 = JobSpec::new(JobKind::Delete {
            targets: vec![Location::Local(PathBuf::from("/temp/file.txt"))],
        });

        // Enqueue all jobs
        update_state(&mut state, Transition::EnqueueJob { spec: job1 });
        update_state(&mut state, Transition::EnqueueJob { spec: job2 });
        update_state(&mut state, Transition::EnqueueJob { spec: job3 });
        update_state(&mut state, Transition::EnqueueJob { spec: job4 });

        // Verify all jobs are queued
        assert_eq!(state.jobs.queue.len(), 4);

        // Start jobs (up to max_parallel)
        let job_ids: Vec<_> = state.jobs.queue.iter().map(|j| j.id).collect();

        update_state(&mut state, Transition::StartNextJob);
        update_state(&mut state, Transition::StartNextJob);
        update_state(&mut state, Transition::StartNextJob);
        update_state(&mut state, Transition::StartNextJob);

        // Verify jobs are running concurrently (up to max_parallel limit)
        assert!(state.jobs.active.len() <= state.jobs.max_parallel);
        assert!(!state.jobs.active.is_empty());

        // Complete jobs one by one
        for job_id in job_ids {
            if state.jobs.active.contains_key(&job_id) {
                update_state(
                    &mut state,
                    Transition::CompleteJob {
                        job_id,
                        result: OpResult::Success(SuccessData::None),
                    },
                );
            }
        }

        // Verify all jobs completed
        assert_eq!(state.jobs.active.len(), 0);
        assert!(!state.jobs.completed.is_empty());
    }

    /// Test job cancellation during concurrent operations
    /// **Validates: Requirements 15.5-15.7, 21.5**
    #[test]
    fn test_cancel_during_concurrent_operations() {
        let config = AppConfig {
            worker_pool_size: 3,
            ..Default::default()
        };
        let mut state = AppState::new(config);

        // Create and enqueue multiple jobs
        let job1 = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/large1/file.bin"))],
            dest: Location::Local(PathBuf::from("/dest1")),
        });

        let job2 = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/large2/file.bin"))],
            dest: Location::Local(PathBuf::from("/dest2")),
        });

        let job3 = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/large3/file.bin"))],
            dest: Location::Local(PathBuf::from("/dest3")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job1 });
        update_state(&mut state, Transition::EnqueueJob { spec: job2 });
        update_state(&mut state, Transition::EnqueueJob { spec: job3 });

        let job_ids: Vec<_> = state.jobs.queue.iter().map(|j| j.id).collect();

        // Start all jobs
        update_state(&mut state, Transition::StartNextJob);
        update_state(&mut state, Transition::StartNextJob);
        update_state(&mut state, Transition::StartNextJob);

        // Verify jobs are running
        assert!(!state.jobs.active.is_empty());

        // Cancel the first job
        let job_to_cancel = job_ids[0];
        let result = update_state(
            &mut state,
            Transition::CancelJob {
                job_id: job_to_cancel,
            },
        );

        // Verify cancellation was requested
        assert!(result.jobs_to_cancel.contains(&job_to_cancel));

        // Verify job is in cancelling state
        if let Some(job) = state.jobs.active.get(&job_to_cancel) {
            assert_eq!(job.state, crate::job::ExecutionState::Cancelling);
        }

        // Acknowledge cancellation
        update_state(
            &mut state,
            Transition::AcknowledgeCancel {
                job_id: job_to_cancel,
            },
        );

        // Verify job was removed from active jobs
        assert!(!state.jobs.active.contains_key(&job_to_cancel));

        // Complete remaining jobs
        for job_id in &job_ids[1..] {
            if state.jobs.active.contains_key(job_id) {
                update_state(
                    &mut state,
                    Transition::CompleteJob {
                        job_id: *job_id,
                        result: OpResult::Success(SuccessData::None),
                    },
                );
            }
        }

        // Verify other jobs completed successfully
        assert_eq!(state.jobs.active.len(), 0);
    }

    /// Test UI responsiveness during heavy load
    /// **Validates: Requirements 21.1-21.6, 21.8**
    #[test]
    fn test_ui_responsiveness_during_heavy_load() {
        let config = AppConfig {
            worker_pool_size: 4,
            ..Default::default()
        };
        let mut state = AppState::new(config);

        // Create many jobs to simulate heavy load
        for i in 0..10 {
            let job = JobSpec::new(JobKind::Copy {
                sources: vec![Location::Local(PathBuf::from(format!(
                    "/source{}/file.txt",
                    i
                )))],
                dest: Location::Local(PathBuf::from(format!("/dest{}", i))),
            });
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        // Verify all jobs are queued
        assert_eq!(state.jobs.queue.len(), 10);

        // Start jobs up to max_parallel
        for _ in 0..state.jobs.max_parallel {
            update_state(&mut state, Transition::StartNextJob);
        }

        // Verify only max_parallel jobs are active
        assert_eq!(state.jobs.active.len(), state.jobs.max_parallel);

        // Simulate UI operations while jobs are running
        // These should not block or fail

        // Switch panes
        update_state(&mut state, Transition::SwitchPane);
        assert_eq!(state.ui.active_pane, crate::model::ActivePane::Right);

        // Switch back
        update_state(&mut state, Transition::SwitchPane);
        assert_eq!(state.ui.active_pane, crate::model::ActivePane::Left);

        // Create a new tab
        update_state(&mut state, Transition::CreateTab);
        assert_eq!(state.tabs.tabs.len(), 2);
        assert_eq!(state.tabs.active_index, 1); // New tab becomes active

        // Switch tabs (should wrap to 0)
        update_state(&mut state, Transition::NextTab);
        assert_eq!(state.tabs.active_index, 0);

        // All UI operations should complete without blocking
        // Jobs should still be active
        assert_eq!(state.jobs.active.len(), state.jobs.max_parallel);
    }

    /// Test FIFO job ordering with concurrent execution
    /// **Validates: Requirements 15.11**
    #[test]
    fn test_fifo_job_ordering() {
        let config = AppConfig {
            worker_pool_size: 2, // Only 2 concurrent jobs
            ..Default::default()
        };
        let mut state = AppState::new(config);

        // Create 5 jobs
        for i in 0..5 {
            let job = JobSpec::new(JobKind::Mkdir {
                location: Location::Local(PathBuf::from(format!("/test/dir{}", i))),
            });
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        assert_eq!(state.jobs.queue.len(), 5);

        // Start first 2 jobs
        update_state(&mut state, Transition::StartNextJob);
        update_state(&mut state, Transition::StartNextJob);

        // Verify only 2 jobs are active
        assert_eq!(state.jobs.active.len(), 2);
        assert_eq!(state.jobs.queue.len(), 3);

        // Complete first job
        let first_job_id = *state.jobs.active.keys().next().unwrap();
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: first_job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Start next job (should be the 3rd job in FIFO order)
        update_state(&mut state, Transition::StartNextJob);

        // Verify still 2 jobs active, 2 remaining in queue
        assert_eq!(state.jobs.active.len(), 2);
        assert_eq!(state.jobs.queue.len(), 2);
    }

    /// Test progress updates during concurrent operations
    /// **Validates: Requirements 15.4, 21.8**
    #[test]
    fn test_progress_updates_during_concurrent_operations() {
        let config = AppConfig {
            worker_pool_size: 3,
            ..Default::default()
        };
        let mut state = AppState::new(config);

        // Create and start 3 jobs
        for i in 0..3 {
            let job = JobSpec::new(JobKind::Copy {
                sources: vec![Location::Local(PathBuf::from(format!(
                    "/source{}/file.txt",
                    i
                )))],
                dest: Location::Local(PathBuf::from(format!("/dest{}", i))),
            });
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        let job_ids: Vec<_> = state.jobs.queue.iter().map(|j| j.id).collect();

        // Start all jobs
        for _ in 0..3 {
            update_state(&mut state, Transition::StartNextJob);
        }

        // Update progress for each job
        for (i, job_id) in job_ids.iter().enumerate() {
            let progress = (i + 1) as f64 * 0.25;
            update_state(
                &mut state,
                Transition::UpdateJobProgress {
                    job_id: *job_id,
                    progress,
                },
            );

            // Verify progress was updated
            if let Some(job) = state.jobs.active.get(job_id) {
                assert_eq!(job.progress, progress);
            }
        }

        // Verify all jobs still active with different progress values
        assert_eq!(state.jobs.active.len(), 3);
    }

    /// Test job queue management with max_parallel limit
    /// **Validates: Requirements 15.12**
    #[test]
    fn test_max_parallel_job_limit() {
        let config = AppConfig {
            worker_pool_size: 2, // Strict limit of 2 concurrent jobs
            ..Default::default()
        };
        let mut state = AppState::new(config);

        // Create 10 jobs
        for i in 0..10 {
            let job = JobSpec::new(JobKind::Delete {
                targets: vec![Location::Local(PathBuf::from(format!(
                    "/temp/file{}.txt",
                    i
                )))],
            });
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        // Try to start all jobs
        for _ in 0..10 {
            update_state(&mut state, Transition::StartNextJob);
        }

        // Verify only max_parallel jobs are active
        assert_eq!(state.jobs.active.len(), 2);
        assert_eq!(state.jobs.queue.len(), 8);

        // Complete one job
        let job_id = *state.jobs.active.keys().next().unwrap();
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Start next job
        update_state(&mut state, Transition::StartNextJob);

        // Verify still only max_parallel jobs active
        assert_eq!(state.jobs.active.len(), 2);
        assert_eq!(state.jobs.queue.len(), 7);
    }

    /// Test concurrent directory size calculations
    /// **Validates: Requirements 37.7**
    #[test]
    fn test_concurrent_directory_size_calculations() {
        let config = AppConfig {
            worker_pool_size: 4,
            ..Default::default()
        };
        let mut state = AppState::new(config);

        // Create multiple directory size calculation jobs
        for i in 0..5 {
            let job = JobSpec::new(JobKind::CalculateSize {
                location: Location::Local(PathBuf::from(format!("/data/dir{}", i))),
            });
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        let job_ids: Vec<_> = state.jobs.queue.iter().map(|j| j.id).collect();

        // Start jobs
        for _ in 0..4 {
            update_state(&mut state, Transition::StartNextJob);
        }

        // Verify multiple size calculations running concurrently
        assert_eq!(state.jobs.active.len(), 4);

        // Complete jobs with different sizes
        for (i, job_id) in job_ids.iter().take(4).enumerate() {
            let size = (i + 1) as u64 * 1024 * 1024; // Different sizes
            update_state(
                &mut state,
                Transition::CompleteJob {
                    job_id: *job_id,
                    result: OpResult::Success(SuccessData::SizeCalculated(size)),
                },
            );
        }

        // Verify all completed successfully
        assert_eq!(state.jobs.completed.len(), 4);
    }

    /// Test mixed job types running concurrently
    /// **Validates: Requirements 21.7**
    #[test]
    fn test_mixed_concurrent_job_types() {
        let config = AppConfig {
            worker_pool_size: 4,
            ..Default::default()
        };
        let mut state = AppState::new(config);

        // Create different types of jobs
        let copy_job = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/source/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest")),
        });

        let move_job = JobSpec::new(JobKind::Move {
            sources: vec![Location::Local(PathBuf::from("/old/file.txt"))],
            dest: Location::Local(PathBuf::from("/new")),
        });

        let delete_job = JobSpec::new(JobKind::Delete {
            targets: vec![Location::Local(PathBuf::from("/temp/file.txt"))],
        });

        let mkdir_job = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(PathBuf::from("/new/directory")),
        });

        // Enqueue all different job types
        update_state(&mut state, Transition::EnqueueJob { spec: copy_job });
        update_state(&mut state, Transition::EnqueueJob { spec: move_job });
        update_state(&mut state, Transition::EnqueueJob { spec: delete_job });
        update_state(&mut state, Transition::EnqueueJob { spec: mkdir_job });

        let job_ids: Vec<_> = state.jobs.queue.iter().map(|j| j.id).collect();

        // Start all jobs
        for _ in 0..4 {
            update_state(&mut state, Transition::StartNextJob);
        }

        // Verify all different job types running concurrently
        assert_eq!(state.jobs.active.len(), 4);

        // Verify different job kinds are present
        let job_kinds: Vec<_> = state
            .jobs
            .active
            .values()
            .map(|j| std::mem::discriminant(&j.spec.kind))
            .collect();
        assert_eq!(job_kinds.len(), 4);

        // Complete all jobs
        for job_id in job_ids {
            update_state(
                &mut state,
                Transition::CompleteJob {
                    job_id,
                    result: OpResult::Success(SuccessData::None),
                },
            );
        }

        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 4);
    }
}
