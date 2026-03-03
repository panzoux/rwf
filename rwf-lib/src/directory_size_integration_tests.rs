//! Integration tests for directory size calculation
//!
//! Tests the complete workflow of calculating directory sizes including:
//! - Single directory calculation
//! - Concurrent calculations
//! - Cancellation support
//! - FileEntry update with calculated size

#[cfg(test)]
mod tests {
    use crate::state::{AppState, update_state};
    use crate::config::AppConfig;
    use crate::model::{Location, FileEntry};
    use crate::state::Transition;
    use crate::job::{JobSpec, JobKind, OpResult, SuccessData};
    use std::path::PathBuf;
    use std::time::SystemTime;

    /// Test single directory size calculation
    #[test]
    fn test_single_directory_size_calculation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up a directory entry in the active pane
        let dir_location = Location::Local(PathBuf::from("/test/mydir"));
        let dir_entry = FileEntry {
            name: "mydir".to_string(),
            location: dir_location.clone(),
            size: 0,
            is_dir: true,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        state.current_tab_mut().left_pane.entries = vec![dir_entry.clone()];
        state.current_tab_mut().left_pane.cursor = 0;
        
        // Create a calculate size job
        let job_spec = JobSpec::new(JobKind::CalculateSize {
            location: dir_location.clone(),
        });
        
        // Enqueue the job
        let result = update_state(&mut state, Transition::EnqueueJob { spec: job_spec.clone() });
        assert!(result.ui_changed);
        assert_eq!(state.jobs.queue.len(), 1);
        
        // Start the job
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);
        assert_eq!(state.jobs.active.len(), 1);
        assert_eq!(state.jobs.queue.len(), 0);
        
        // Simulate job completion with calculated size
        let calculated_size = 1024 * 1024 * 10; // 10 MB
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::SizeCalculated(calculated_size)),
        });
        
        // Verify the FileEntry was updated with calculated size
        let updated_entry = &state.current_tab().left_pane.entries[0];
        assert_eq!(updated_entry.calculated_size, Some(calculated_size));
        assert_eq!(updated_entry.location, dir_location);
        
        // Verify job is no longer active
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
    }
    
    /// Test concurrent directory size calculations
    #[test]
    fn test_concurrent_directory_size_calculations() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up multiple directory entries
        let dir1_location = Location::Local(PathBuf::from("/test/dir1"));
        let dir2_location = Location::Local(PathBuf::from("/test/dir2"));
        let dir3_location = Location::Local(PathBuf::from("/test/dir3"));
        
        let entries = vec![
            FileEntry {
                name: "dir1".to_string(),
                location: dir1_location.clone(),
                size: 0,
                is_dir: true,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "dir2".to_string(),
                location: dir2_location.clone(),
                size: 0,
                is_dir: true,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "dir3".to_string(),
                location: dir3_location.clone(),
                size: 0,
                is_dir: true,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        
        state.current_tab_mut().left_pane.entries = entries;
        
        // Enqueue multiple size calculation jobs
        let job1_spec = JobSpec::new(JobKind::CalculateSize { location: dir1_location.clone() });
        let job2_spec = JobSpec::new(JobKind::CalculateSize { location: dir2_location.clone() });
        let job3_spec = JobSpec::new(JobKind::CalculateSize { location: dir3_location.clone() });
        
        update_state(&mut state, Transition::EnqueueJob { spec: job1_spec });
        update_state(&mut state, Transition::EnqueueJob { spec: job2_spec });
        update_state(&mut state, Transition::EnqueueJob { spec: job3_spec });
        
        assert_eq!(state.jobs.queue.len(), 3);
        
        // Start jobs (up to max_parallel)
        let job1_id = state.jobs.queue[0].id;
        let job2_id = state.jobs.queue[1].id;
        let job3_id = state.jobs.queue[2].id;
        
        update_state(&mut state, Transition::StartNextJob);
        update_state(&mut state, Transition::StartNextJob);
        update_state(&mut state, Transition::StartNextJob);
        
        // Verify jobs are running concurrently (up to max_parallel limit)
        let active_count = state.jobs.active.len();
        assert!(active_count > 0 && active_count <= state.jobs.max_parallel);
        
        // Complete the jobs
        update_state(&mut state, Transition::CompleteJob {
            job_id: job1_id,
            result: OpResult::Success(SuccessData::SizeCalculated(1024)),
        });
        
        update_state(&mut state, Transition::CompleteJob {
            job_id: job2_id,
            result: OpResult::Success(SuccessData::SizeCalculated(2048)),
        });
        
        update_state(&mut state, Transition::CompleteJob {
            job_id: job3_id,
            result: OpResult::Success(SuccessData::SizeCalculated(4096)),
        });
        
        // Verify all entries were updated
        let entries = &state.current_tab().left_pane.entries;
        assert_eq!(entries[0].calculated_size, Some(1024));
        assert_eq!(entries[1].calculated_size, Some(2048));
        assert_eq!(entries[2].calculated_size, Some(4096));
    }
    
    /// Test cancellation of directory size calculation
    #[test]
    fn test_directory_size_calculation_cancellation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up a directory entry
        let dir_location = Location::Local(PathBuf::from("/test/largedir"));
        let dir_entry = FileEntry {
            name: "largedir".to_string(),
            location: dir_location.clone(),
            size: 0,
            is_dir: true,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        state.current_tab_mut().left_pane.entries = vec![dir_entry];
        
        // Create and enqueue a calculate size job
        let job_spec = JobSpec::new(JobKind::CalculateSize {
            location: dir_location.clone(),
        });
        
        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;
        
        // Start the job
        update_state(&mut state, Transition::StartNextJob);
        assert_eq!(state.jobs.active.len(), 1);
        
        // Cancel the job
        let result = update_state(&mut state, Transition::CancelJob { job_id });
        assert!(result.jobs_to_cancel.contains(&job_id));
        
        // Verify the job is in cancelling state
        let job = state.jobs.active.get(&job_id).unwrap();
        assert_eq!(job.state, crate::job::ExecutionState::Cancelling);
        
        // Acknowledge cancellation
        update_state(&mut state, Transition::AcknowledgeCancel { job_id });
        
        // Verify job is no longer active
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
        
        // Verify the entry was not updated (cancellation occurred)
        let entry = &state.current_tab().left_pane.entries[0];
        assert_eq!(entry.calculated_size, None);
    }
    
    /// Test size calculation updates entry in multiple panes
    #[test]
    fn test_size_calculation_updates_multiple_panes() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up the same directory in both panes
        let dir_location = Location::Local(PathBuf::from("/test/shareddir"));
        let dir_entry = FileEntry {
            name: "shareddir".to_string(),
            location: dir_location.clone(),
            size: 0,
            is_dir: true,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        state.current_tab_mut().left_pane.entries = vec![dir_entry.clone()];
        state.current_tab_mut().right_pane.entries = vec![dir_entry.clone()];
        
        // Create and execute a calculate size job
        let job_spec = JobSpec::new(JobKind::CalculateSize {
            location: dir_location.clone(),
        });
        
        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;
        
        update_state(&mut state, Transition::StartNextJob);
        
        // Complete the job
        let calculated_size = 5 * 1024 * 1024; // 5 MB
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::SizeCalculated(calculated_size)),
        });
        
        // Verify both panes were updated
        assert_eq!(state.current_tab().left_pane.entries[0].calculated_size, Some(calculated_size));
        assert_eq!(state.current_tab().right_pane.entries[0].calculated_size, Some(calculated_size));
    }
    
    /// Test size calculation with job progress updates
    #[test]
    fn test_size_calculation_with_progress_updates() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up a directory entry
        let dir_location = Location::Local(PathBuf::from("/test/progressdir"));
        let dir_entry = FileEntry {
            name: "progressdir".to_string(),
            location: dir_location.clone(),
            size: 0,
            is_dir: true,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        state.current_tab_mut().left_pane.entries = vec![dir_entry];
        
        // Create and start a calculate size job
        let job_spec = JobSpec::new(JobKind::CalculateSize {
            location: dir_location.clone(),
        });
        
        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;
        
        update_state(&mut state, Transition::StartNextJob);
        
        // Simulate progress updates
        update_state(&mut state, Transition::UpdateJobProgress {
            job_id,
            progress: 0.3,
        });
        
        let job = state.jobs.active.get(&job_id).unwrap();
        assert_eq!(job.progress, 0.3);
        
        update_state(&mut state, Transition::UpdateJobProgress {
            job_id,
            progress: 0.6,
        });
        
        let job = state.jobs.active.get(&job_id).unwrap();
        assert_eq!(job.progress, 0.6);
        
        // Complete the job
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::SizeCalculated(1024 * 1024)),
        });
        
        // Verify completion
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.current_tab().left_pane.entries[0].calculated_size, Some(1024 * 1024));
    }
    
    /// Test size calculation for non-directory entry (should not create job)
    #[test]
    fn test_size_calculation_for_file_does_nothing() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up a file entry (not a directory)
        let file_location = Location::Local(PathBuf::from("/test/file.txt"));
        let file_entry = FileEntry {
            name: "file.txt".to_string(),
            location: file_location.clone(),
            size: 1024,
            is_dir: false, // This is a file, not a directory
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        state.current_tab_mut().left_pane.entries = vec![file_entry];
        state.current_tab_mut().left_pane.cursor = 0;
        
        // Try to trigger size calculation via action
        use crate::input::{Action, action_to_transitions};
        let transitions = action_to_transitions(&state, &Action::CalculateDirectorySize);
        
        // Should return empty transitions since it's not a directory
        assert_eq!(transitions.len(), 0);
        assert_eq!(state.jobs.queue.len(), 0);
    }
}
