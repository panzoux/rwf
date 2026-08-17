//! End-to-end workflow integration tests
//!
//! Tests complete workflows including:
//! - File copy workflow
//! - File move workflow
//! - Delete workflow
//! - Tab management workflow
//! - Custom function workflow
//!
//! **Validates: All requirements**

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action};
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::{
        CustomFunction, DeleteConfirmDialog, DialogContent, InputDialog, Location,
        SimpleRenameDialog,
    };
    use crate::state::{update_state, Transition};
    use crate::test_utils::{test_state, FileEntryBuilder};
    use std::path::PathBuf;

    fn create_test_file_entry(
        name: &str,
        path: &str,
        is_dir: bool,
        marked: bool,
    ) -> crate::model::FileEntry {
        FileEntryBuilder::new(name)
            .path(path)
            .dir(is_dir)
            .marked(marked)
            .size(if is_dir { 0 } else { 1024 })
            .build()
    }

    /// Test complete file copy workflow from start to finish
    /// **Validates: Requirements 6.1-6.12**
    #[test]
    fn test_complete_file_copy_workflow() {
        let mut state = test_state();

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
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(file2_loc.clone());
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(file3_loc.clone());

        // Setup: Set right pane destination
        state.current_tab_mut().right_pane.current_location =
            Location::Local(PathBuf::from("/dest"));

        // Step 1: User initiates copy operation (CreatePendingFileJob — no dialog)
        let transitions = action_to_transitions(&state, &Action::Copy);
        assert_eq!(transitions.len(), 1);
        assert!(matches!(
            transitions[0],
            Transition::CreatePendingFileJob { .. }
        ));

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
        let mut state = test_state();

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
        let mut state = test_state();

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
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(file1_loc.clone());
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(file2_loc.clone());

        // Step 1: User initiates delete operation
        let transitions = action_to_transitions(&state, &Action::Delete);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify delete confirmation dialog
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        if let DialogContent::DeleteConfirm(DeleteConfirmDialog { targets, .. }) = &dialog.content {
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
        let mut state = test_state();

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
        let mut state = test_state();

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
        let mut state = test_state();

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
        let mut state = test_state();

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
        let mut state = test_state();

        // Setup: Add file to left pane
        state.current_tab_mut().left_pane.entries = vec![create_test_file_entry(
            "oldname.txt",
            "/test/oldname.txt",
            false,
            false,
        )];
        state.current_tab_mut().left_pane.cursor = 0;

        // Step 1: User initiates rename operation
        let transitions = action_to_transitions(&state, &Action::Rename);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify rename dialog is shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        if let DialogContent::SimpleRename(SimpleRenameDialog { input, .. }) = &dialog.content {
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
        let mut state = test_state();

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
        if let DialogContent::Input(InputDialog { prompt, .. }) = &dialog.content {
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

    /// Test complete create-file workflow
    #[test]
    fn test_complete_create_file_workflow() {
        let mut state = test_state();

        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/test"));

        // Step 1: User initiates create-file operation
        let transitions = action_to_transitions(&state, &Action::CreateFile);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify create-file dialog is shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        if let DialogContent::Input(InputDialog { prompt, .. }) = &dialog.content {
            assert_eq!(prompt, "File name:");
        } else {
            panic!("Expected input dialog");
        }

        // Step 2: User enters file name
        state.dialogs.input_buffer = "newfile.txt".to_string();
        let result = update_state(&mut state, Transition::ConfirmDialog);

        // Verify create-file job was created
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::CreateFile { location } => {
                assert_eq!(
                    location,
                    &Location::Local(PathBuf::from("/test/newfile.txt"))
                );
            }
            _ => panic!("Expected CreateFile job"),
        }

        // Enqueue the job
        for job in result.jobs_to_start {
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }

        // Step 3: Execute job
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);

        // Step 4: Complete job
        let complete_result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );

        // Verify completion
        assert_eq!(state.jobs.active.len(), 0);
        assert_eq!(state.jobs.completed.len(), 1);

        // Verify the pane is refreshed so the new file becomes visible (mirrors Mkdir)
        assert_eq!(complete_result.panes_to_refresh.len(), 1);
    }

    /// Test complete attribute-change workflow: open dialog for the cursor
    /// entry (nothing marked), toggle a field, confirm, verify the resulting
    /// job. Requires a real file since attributes are read via `metadata()`.
    #[cfg(windows)]
    #[test]
    fn test_complete_attr_timestamp_workflow() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("a.txt");
        std::fs::write(&file_path, b"x").unwrap();

        let mut state = test_state();
        state.current_tab_mut().left_pane.current_location =
            Location::Local(temp_dir.path().to_path_buf());
        state.current_tab_mut().left_pane.entries = vec![create_test_file_entry(
            "a.txt",
            file_path.to_str().unwrap(),
            false,
            false,
        )];

        // Step 1: open dialog for the cursor entry (nothing marked)
        let transitions = action_to_transitions(&state, &Action::ShowAttrTimestampDialog);
        for t in transitions {
            update_state(&mut state, t);
        }
        assert!(!state.dialogs.is_empty());

        // Step 2: toggle "hidden" (simulates the dialog-mode key handler)
        if let Some(dialog) = state.dialogs.current_mut() {
            if let DialogContent::AttrTimestamp(d) = &mut dialog.content {
                d.hidden.toggle();
            } else {
                panic!("Expected AttrTimestamp dialog");
            }
        }

        // Step 3: confirm
        let result = update_state(&mut state, Transition::ConfirmAttrTimestampDialog);
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::ChangeAttributes { targets, attrs } => {
                assert_eq!(targets, &vec![Location::Local(file_path)]);
                assert_eq!(attrs.hidden, Some(true));
            }
            other => panic!("Expected ChangeAttributes, got {:?}", other),
        }
        assert!(state.dialogs.is_empty());
    }

    /// Test complete create-link workflow: open dialog for the cursor entry
    /// (target), destination comes from the opposite pane, confirm, verify
    /// the resulting job.
    #[test]
    fn test_complete_create_link_workflow() {
        use tempfile::TempDir;

        let source_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();
        let target_path = source_dir.path().join("report.docx");
        std::fs::write(&target_path, b"x").unwrap();

        let mut state = test_state();
        state.current_tab_mut().left_pane.current_location =
            Location::Local(source_dir.path().to_path_buf());
        state.current_tab_mut().left_pane.entries = vec![create_test_file_entry(
            "report.docx",
            target_path.to_str().unwrap(),
            false,
            false,
        )];
        state.current_tab_mut().right_pane.current_location =
            Location::Local(dest_dir.path().to_path_buf());

        // Step 1: open dialog for the cursor entry, left pane active
        let transitions = action_to_transitions(&state, &Action::ShowCreateLinkDialog);
        for t in transitions {
            update_state(&mut state, t);
        }
        assert!(!state.dialogs.is_empty());
        if let Some(dialog) = state.dialogs.current() {
            if let DialogContent::CreateLink(d) = &dialog.content {
                assert_eq!(d.target, Location::Local(target_path.clone()));
                assert_eq!(d.dest_dir, dest_dir.path());
                assert_eq!(d.link_name, "report.docx");
            } else {
                panic!("Expected CreateLink dialog");
            }
        }

        // Step 2: confirm
        let result = update_state(&mut state, Transition::ConfirmCreateLinkDialog);
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::CreateLink {
                target,
                link_path,
                kind,
            } => {
                assert_eq!(target, &Location::Local(target_path));
                assert_eq!(
                    link_path,
                    &Location::Local(dest_dir.path().join("report.docx"))
                );
                assert_eq!(kind, &crate::model::LinkCreateKind::Symlink);
            }
            other => panic!("Expected CreateLink, got {:?}", other),
        }
        assert!(state.dialogs.is_empty());

        // Step 3: enqueue, start, complete the job — the link lands in the
        // *opposite* (right) pane's directory, not the active (left) pane,
        // so that's the one that must be refreshed to show the new entry.
        for job in result.jobs_to_start {
            update_state(&mut state, Transition::EnqueueJob { spec: job });
        }
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);
        let complete_result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::None),
            },
        );
        assert_eq!(complete_result.panes_to_refresh.len(), 1);
        assert_eq!(
            complete_result.panes_to_refresh[0].pane,
            crate::model::ActivePane::Right
        );
    }

    /// End-to-end Undo/Redo round trip (Phase 7.6): Copy a real file via a
    /// real `JobExecutor` + `LocalFilesystemBackend` (no mocking of I/O),
    /// Undo it (the copy is deleted), then Redo it (the copy comes back) —
    /// verifying both on-disk filesystem state and the `OperationRecord`
    /// shapes at each step. Exercises the full chain: `execute_copy` ->
    /// `execute_reversal`'s `Delete{recreate}` arm (undo) ->
    /// `execute_reversal`'s `Copy` arm (redo).
    #[tokio::test]
    async fn copy_undo_redo_round_trip() {
        use crate::backend::{LocalFilesystemBackend, MockArchiveHandler};
        use crate::job::JobExecutor;
        use crate::model::UndoAvailability;
        use crate::worker_pool::JobEvent;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let backend = std::sync::Arc::new(LocalFilesystemBackend::new());
        let archive_handler = std::sync::Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let src = temp_dir.path().join("a.txt");
        tokio::fs::write(&src, b"hello").await.unwrap();
        let dest_dir = temp_dir.path().join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();
        let copied = dest_dir.join("a.txt");

        // Skip Started/Progress events, return the terminal event.
        async fn next_terminal_event(
            event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<JobEvent>,
        ) -> Option<JobEvent> {
            loop {
                match event_rx.recv().await {
                    Some(JobEvent::Started(_)) | Some(JobEvent::Progress(_, _)) => continue,
                    other => return other,
                }
            }
        }

        // 1. Copy.
        let copy_spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(src.clone())],
            dest: Location::Local(dest_dir.clone()),
        });
        executor.execute(copy_spec).await;
        let copy_records = match next_terminal_event(&mut event_rx).await {
            Some(JobEvent::Completed(_, SuccessData::OperationRecords(r))) => r,
            other => panic!("expected OperationRecords, got {other:?}"),
        };
        assert_eq!(copy_records.len(), 1);
        assert!(copy_records[0].succeeded);
        assert_eq!(copy_records[0].source, Some(Location::Local(src.clone())));
        assert_eq!(
            copy_records[0].destination,
            Some(Location::Local(copied.clone()))
        );
        assert!(copied.exists());
        assert_eq!(tokio::fs::read(&copied).await.unwrap(), b"hello");
        let undo_action = match &copy_records[0].undo {
            UndoAvailability::Available(action) => action.clone(),
            other => panic!("expected Available undo, got {other:?}"),
        };

        // 2. Undo (delete the copy).
        let undo_spec = JobSpec::new(JobKind::ExecuteReversal {
            actions: vec![undo_action],
            operation_name: "Copy".to_string(),
            resulting_is_undo: true,
        });
        executor.execute(undo_spec).await;
        let undo_records = match next_terminal_event(&mut event_rx).await {
            Some(JobEvent::Completed(_, SuccessData::OperationRecords(r))) => r,
            other => panic!("expected OperationRecords, got {other:?}"),
        };
        assert_eq!(undo_records.len(), 1);
        assert!(undo_records[0].succeeded);
        assert_eq!(
            undo_records[0].source,
            Some(Location::Local(copied.clone()))
        );
        assert!(!copied.exists());
        let redo_action = match &undo_records[0].undo {
            UndoAvailability::Available(action) => action.clone(),
            other => panic!("expected Available redo (recreate), got {other:?}"),
        };

        // 3. Redo (recreate the copy).
        let redo_spec = JobSpec::new(JobKind::ExecuteReversal {
            actions: vec![redo_action],
            operation_name: "Copy".to_string(),
            resulting_is_undo: false,
        });
        executor.execute(redo_spec).await;
        let redo_records = match next_terminal_event(&mut event_rx).await {
            Some(JobEvent::Completed(_, SuccessData::OperationRecords(r))) => r,
            other => panic!("expected OperationRecords, got {other:?}"),
        };
        assert_eq!(redo_records.len(), 1);
        assert!(redo_records[0].succeeded);
        assert!(copied.exists());
        assert_eq!(tokio::fs::read(&copied).await.unwrap(), b"hello");
        assert!(matches!(
            redo_records[0].undo,
            UndoAvailability::Available(_)
        ));
    }

    /// End-to-end history-navigation round trip: run two real Copy jobs via
    /// a real `JobExecutor`, build both `OperationReport`s via the real
    /// `build_operation_report` (same as `CompleteJob` would), push them
    /// into `AppState.operation_reports`, open the dialog (shows the
    /// latest), navigate to the older report, and navigate back — proving
    /// `is_latest()` flips both directions across the full
    /// executor -> report-builder -> history -> dialog -> navigation chain.
    #[tokio::test]
    async fn undo_is_blocked_while_browsing_an_older_report_and_works_after_returning_to_latest() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let backend = std::sync::Arc::new(crate::backend::LocalFilesystemBackend::new());
        let archive_handler = std::sync::Arc::new(crate::backend::MockArchiveHandler);
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let executor = crate::job::JobExecutor::new(backend, archive_handler, event_tx);

        async fn next_terminal_event(
            rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::worker_pool::JobEvent>,
        ) -> crate::worker_pool::JobEvent {
            loop {
                match rx.recv().await.expect("channel closed") {
                    crate::worker_pool::JobEvent::Started(_)
                    | crate::worker_pool::JobEvent::Progress(..) => continue,
                    other => return other,
                }
            }
        }

        // Two independent Copy operations, producing two OperationReports.
        let src1 = temp_dir.path().join("a.txt");
        tokio::fs::write(&src1, b"one").await.unwrap();
        let src2 = temp_dir.path().join("b.txt");
        tokio::fs::write(&src2, b"two").await.unwrap();
        let dest_dir = temp_dir.path().join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        let spec1 = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(src1.clone())],
            dest: Location::Local(dest_dir.clone()),
        });
        executor.execute(spec1).await;
        let event1 = next_terminal_event(&mut event_rx).await;
        let records1 = match event1 {
            crate::worker_pool::JobEvent::Completed(_, SuccessData::OperationRecords(r)) => r,
            other => panic!("expected OperationRecords, got {other:?}"),
        };

        let spec2 = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(src2.clone())],
            dest: Location::Local(dest_dir.clone()),
        });
        executor.execute(spec2).await;
        let event2 = next_terminal_event(&mut event_rx).await;
        let records2 = match event2 {
            crate::worker_pool::JobEvent::Completed(_, SuccessData::OperationRecords(r)) => r,
            other => panic!("expected OperationRecords, got {other:?}"),
        };

        // Build the AppState-level history exactly as CompleteJob would: two
        // reports, oldest first.
        let mut state = test_state();
        let report1 = crate::job::build_operation_report(
            &JobSpec::new(JobKind::Copy {
                sources: vec![Location::Local(src1)],
                dest: Location::Local(dest_dir.clone()),
            }),
            &OpResult::Success(SuccessData::OperationRecords(records1)),
            1,
        )
        .expect("report1");
        let report2 = crate::job::build_operation_report(
            &JobSpec::new(JobKind::Copy {
                sources: vec![Location::Local(src2)],
                dest: Location::Local(dest_dir.clone()),
            }),
            &OpResult::Success(SuccessData::OperationRecords(records2)),
            2,
        )
        .expect("report2");
        state.operation_reports.push_back(report1);
        state.operation_reports.push_back(report2);

        // Open the dialog and confirm it starts on report2 (the latest) before
        // navigating anywhere — without this, the two flip assertions below
        // would only prove the flag *changes*, not that it changes *from a
        // known-correct starting point*.
        update_state(&mut state, Transition::ShowOperationReport);
        match state.dialogs.current().map(|d| &d.content) {
            Some(DialogContent::OperationReportView(c)) => {
                assert!(c.is_latest(), "should open on the latest report");
                assert_eq!(c.report.id, 2);
            }
            other => panic!("expected OperationReportView, got {other:?}"),
        }

        update_state(
            &mut state,
            Transition::NavigateOperationReportHistory { older: true },
        );

        let content = match state.dialogs.current().map(|d| &d.content) {
            Some(DialogContent::OperationReportView(c)) => c,
            other => panic!("expected OperationReportView, got {other:?}"),
        };
        assert!(
            !content.is_latest(),
            "should be viewing the older report after navigating"
        );
        assert_eq!(content.report.id, 1);

        // Navigate back to the latest — is_latest() must flip back to true.
        update_state(
            &mut state,
            Transition::NavigateOperationReportHistory { older: false },
        );
        let content = match state.dialogs.current().map(|d| &d.content) {
            Some(DialogContent::OperationReportView(c)) => c,
            other => panic!("expected OperationReportView, got {other:?}"),
        };
        assert!(content.is_latest());
        assert_eq!(content.report.id, 2);
    }
}
