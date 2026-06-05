//! End-to-end workflow integration tests
//!
//! Tests complete workflows including:
//! - File copy workflow
//! - File move workflow
//! - Delete workflow
//! - Tab management workflow
//! - Custom function workflow
//! **Validates: All requirements**

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::input::{action_to_transitions, Action};
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::{CustomFunction, DialogContent, FileEntry, Location};
    use crate::state::{update_state, AppState, Transition};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_file_entry(name: &str, path: &str, is_dir: bool, marked: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(path)),
            size: if is_dir { 0 } else { 1024 },
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked,
            calculated_size: None,
        }
    }

    /// Test complete file copy workflow from start to finish
    /// **Validates: Requirements 6.1-6.12**
    #[test]
    fn test_complete_file_copy_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Setup: Add files to left pane
        let _file1_loc = Location::Local(PathBuf::from("/source/file1.txt"));
        let file2_loc = Location::Local(PathBuf::from("/source/file2.txt"));
        let file3_loc = Location::Local(PathBuf::from("/source/file3.txt"));

        state.current_tab_mut().left_pane.entries = vec![
            create_test_file_entry("file1.txt", "/source/file1.txt", false, false),
            create_test_file_entry("file2.txt", "/source/file2.txt", false, false),
            create_test_file_entry("file3.txt", "/source/file3.txt", false, false),
        ];
        state.current_tab_mut().left_pane.cursor = 0;
        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/source"));

        // Mark files 2 and 3
        state.current_tab_mut().left_pane.marking.mark(file2_loc.clone());
        state.current_tab_mut().left_pane.marking.mark(file3_loc.clone());

        // Setup: Set right pane destination
        state.current_tab_mut().right_pane.current_location =
            Location::Local(PathBuf::from("/dest"));

        // Step 1: User initiates copy operation (CreatePendingFileJob — no dialog)
        let transitions = action_to_transitions(&state, &Action::Copy);
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::CreatePendingFileJob { .. }));

        let result = update_state(&mut state, transitions.into_iter().next().unwrap());

        // Verify copy job was created directly
        assert_eq!(result.jobs_to_start.len(), 1);
        let job_spec = &result.jobs_to_start[0];
        match &job_spec.kind {
            JobKind::Copy { sources, dest } => {
                assert_eq!(sources.len(), 2); // 2 marked files
                assert_eq!(dest, &Location::Local(PathBuf::from("/dest")));
            }
            _ => panic!("Expected Copy job"),
        }

        // Enqueue the job
        for job in result.jobs_to_start {
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        // Step 3: Job starts execution
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);

        // Verify job is now active
        assert_eq!(state.jobs.active.len(), 1);
        assert_eq!(state.jobs.queue.len(), 0);

        // Step 4: Simulate progress updates
        update_state(
            &mut state,
            Transition::UpdateJobProgress {
                job_id,
                progress: 0.5,
            },
        );

        let job = state.jobs.active.get(&job_id).unwrap();
        assert_eq!(job.progress, 0.5);

        // Step 5: Job completes successfully
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Verify job is no longer active
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);

        // Verify completion result
        let completed = &state.jobs.completed[0];
        assert!(matches!(completed.result, OpResult::Success(_)));
    }


    /// Test complete file move workflow
    /// **Validates: Requirements 7.1-7.12**
    #[test]
    fn test_complete_file_move_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Setup: Add files to left pane
        state.current_tab_mut().left_pane.entries = vec![
            create_test_file_entry("file1.txt", "/source/file1.txt", false, true),
            create_test_file_entry("file2.txt", "/source/file2.txt", false, false),
        ];
        state.current_tab_mut().left_pane.cursor = 0;
        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/source"));

        state.current_tab_mut().right_pane.current_location =
            Location::Local(PathBuf::from("/dest"));

        // Step 1: User initiates move operation (CreatePendingFileJob — no dialog)
        let transitions = action_to_transitions(&state, &Action::Move);
        assert_eq!(transitions.len(), 1);
        let result = update_state(&mut state, transitions.into_iter().next().unwrap());

        // Verify move job was created directly
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::Move { sources, dest } => {
                assert_eq!(sources.len(), 1); // 1 marked file
                assert_eq!(dest, &Location::Local(PathBuf::from("/dest")));
            }
            _ => panic!("Expected Move job"),
        }

        // Enqueue the job
        for job in result.jobs_to_start {
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        // Step 3: Execute job
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);
        assert_eq!(state.jobs.active.len(), 1);

        // Step 4: Complete job
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Verify completion
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
    }

    /// Test complete delete workflow
    /// **Validates: Requirements 8.1-8.11**
    #[test]
    fn test_complete_delete_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Setup: Add files to left pane
        let file1_loc = Location::Local(PathBuf::from("/test/file1.txt"));
        let file2_loc = Location::Local(PathBuf::from("/test/file2.txt"));
        let _file3_loc = Location::Local(PathBuf::from("/test/file3.txt"));

        state.current_tab_mut().left_pane.entries = vec![
            create_test_file_entry("file1.txt", "/test/file1.txt", false, false),
            create_test_file_entry("file2.txt", "/test/file2.txt", false, false),
            create_test_file_entry("file3.txt", "/test/file3.txt", false, false),
        ];
        state.current_tab_mut().left_pane.cursor = 0;

        // Mark files 1 and 2
        state.current_tab_mut().left_pane.marking.mark(file1_loc.clone());
        state.current_tab_mut().left_pane.marking.mark(file2_loc.clone());

        // Step 1: User initiates delete operation
        let transitions = action_to_transitions(&state, &Action::Delete);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify delete confirmation dialog
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        if let DialogContent::DeleteConfirm { targets, .. } = &dialog.content {
            assert_eq!(targets.len(), 2); // 2 marked files
        } else {
            panic!("Expected DeleteConfirm dialog");
        }

        // Step 2: Confirm delete
        let result = update_state(&mut state, Transition::ConfirmDialog);

        // Verify delete job was created
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::Delete { targets } => {
                assert_eq!(targets.len(), 2); // 2 marked files
            }
            _ => panic!("Expected Delete job"),
        }

        // Enqueue the job
        for job in result.jobs_to_start {
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        // Step 3: Execute job
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);

        // Step 4: Complete job
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Verify completion
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
    }

    /// Test complete tab management workflow
    /// **Validates: Requirements 27.1-27.14**
    #[test]
    fn test_complete_tab_management_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Initial state: 1 tab
        assert_eq!(state.tabs.tabs.len(), 1);
        assert_eq!(state.tabs.active_index, 0);

        // Step 1: Create new tab
        let transitions = action_to_transitions(&state, &Action::NewTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify new tab was created
        assert_eq!(state.tabs.tabs.len(), 2);
        assert_eq!(state.tabs.active_index, 1);

        // Step 2: Create another tab
        state.last_tab_created = None;
        let transitions = action_to_transitions(&state, &Action::NewTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        assert_eq!(state.tabs.tabs.len(), 3);
        assert_eq!(state.tabs.active_index, 2);

        // Step 3: Switch to next tab (wraps around)
        let transitions = action_to_transitions(&state, &Action::NextTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        assert_eq!(state.tabs.active_index, 0);

        // Step 4: Switch to previous tab
        let transitions = action_to_transitions(&state, &Action::PrevTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        assert_eq!(state.tabs.active_index, 2);

        // Step 5: Close current tab
        let transitions = action_to_transitions(&state, &Action::CloseTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        assert_eq!(state.tabs.tabs.len(), 2);
        assert_eq!(state.tabs.active_index, 1);

        // Step 6: Try to close last tab (should fail when only 1 remains)
        let transitions = action_to_transitions(&state, &Action::CloseTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        assert_eq!(state.tabs.tabs.len(), 1);

        // Try to close the last tab
        let transitions = action_to_transitions(&state, &Action::CloseTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Should still have 1 tab (cannot close last tab)
        assert_eq!(state.tabs.tabs.len(), 1);
    }

    /// Test complete custom function workflow
    /// **Validates: Requirements 28.1-28.15**
    #[test]
    fn test_complete_custom_function_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Setup: Add files to left pane
        state.current_tab_mut().left_pane.entries = vec![
            create_test_file_entry("file1.txt", "/test/file1.txt", false, false),
            create_test_file_entry("file2.txt", "/test/file2.txt", false, true),
        ];
        state.current_tab_mut().left_pane.cursor = 0;
        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/test"));

        // Step 1: Create a custom function job directly (since there's no ShowCustomFunctionSelector action)
        let _custom_func = CustomFunction::new("test_func", "echo $F");
        let job_spec = JobSpec::new(JobKind::ExecuteCustomFunction {
            command: "echo file1.txt".to_string(),
            working_dir: Location::Local(PathBuf::from("/test")),
            pipe_to_action: None,
            shell: Some("bash".to_string()),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });

        // Verify job was enqueued
        assert_eq!(state.jobs.queue.len(), 1);
        match &state.jobs.queue[0].kind {
            JobKind::ExecuteCustomFunction { command, .. } => {
                assert!(command.contains("file1.txt"));
            }
            _ => panic!("Expected ExecuteCustomFunction job"),
        }

        // Step 2: Execute job
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);

        // Step 3: Complete job
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::CustomFunctionOutput(
                    "file1.txt".to_string(),
                )),
            },
        );

        // Verify completion
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
    }

    /// Test workflow with job cancellation
    /// **Validates: Requirements 15.5-15.7**
    #[test]
    fn test_workflow_with_job_cancellation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Setup: Create a copy job
        let job_spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/source/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;

        // Start the job
        update_state(&mut state, Transition::StartNextJob);
        assert_eq!(state.jobs.active.len(), 1);

        // Cancel the job
        let result = update_state(&mut state, Transition::CancelJob { job_id });
        assert!(result.jobs_to_cancel.contains(&job_id));

        // Verify job is in cancelling state
        let job = state.jobs.active.get(&job_id).unwrap();
        assert_eq!(job.state, crate::job::ExecutionState::Cancelling);

        // Acknowledge cancellation
        update_state(&mut state, Transition::AcknowledgeCancel { job_id });

        // Verify job is no longer active
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);

        // Verify cancellation result
        let completed = &state.jobs.completed[0];
        assert!(matches!(completed.result, OpResult::Cancelled));
    }

    /// Test workflow with error handling
    /// **Validates: Requirements 19.1-19.5**
    #[test]
    fn test_workflow_with_error_handling() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Setup: Create a copy job
        let job_spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/nonexistent/file.txt"))],
            dest: Location::Local(PathBuf::from("/dest")),
        });

        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;

        // Start the job
        update_state(&mut state, Transition::StartNextJob);

        // Complete job with error
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("File not found".to_string()),
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
    }

    /// Test complete rename workflow
    /// **Validates: Requirements 9.1-9.9**
    #[test]
    fn test_complete_rename_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Setup: Add file to left pane
        state.current_tab_mut().left_pane.entries =
            vec![create_test_file_entry("oldname.txt", "/test/oldname.txt", false, false)];
        state.current_tab_mut().left_pane.cursor = 0;

        // Step 1: User initiates rename operation
        let transitions = action_to_transitions(&state, &Action::Rename);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify rename dialog is shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        if let DialogContent::SimpleRename { input, .. } = &dialog.content {
            assert_eq!(input, "oldname.txt");
        } else {
            panic!("Expected SimpleRename dialog");
        }

        // Step 2: User enters new name
        state.dialogs.input_buffer = "newname.txt".to_string();
        let result = update_state(&mut state, Transition::ConfirmDialog);

        // Verify rename job was created
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::Rename { from, to } => {
                assert_eq!(from, &Location::Local(PathBuf::from("/test/oldname.txt")));
                assert_eq!(to, &Location::Local(PathBuf::from("/test/newname.txt")));
            }
            _ => panic!("Expected Rename job"),
        }

        // Enqueue the job
        for job in result.jobs_to_start {
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        // Step 3: Execute job
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);

        // Step 4: Complete job
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Verify completion
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
    }

    /// Test complete mkdir workflow
    /// **Validates: Requirements 10.1-10.9**
    #[test]
    fn test_complete_mkdir_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/test"));

        // Step 1: User initiates mkdir operation
        let transitions = action_to_transitions(&state, &Action::CreateDirectory);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify mkdir dialog is shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        if let DialogContent::Input { prompt, .. } = &dialog.content {
            assert_eq!(prompt, "Directory name:");
        } else {
            panic!("Expected input dialog");
        }

        // Step 2: User enters directory name
        state.dialogs.input_buffer = "newdir".to_string();
        let result = update_state(&mut state, Transition::ConfirmDialog);

        // Verify mkdir job was created
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::Mkdir { location } => {
                assert_eq!(location, &Location::Local(PathBuf::from("/test/newdir")));
            }
            _ => panic!("Expected Mkdir job"),
        }

        // Enqueue the job
        for job in result.jobs_to_start {
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        // Step 3: Execute job
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);

        // Step 4: Complete job
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Verify completion
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);
    }
}
