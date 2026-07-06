//! Integration tests for file operation key handlers

#[cfg(test)]
mod tests {
    use crate::backend::MockArchiveHandler;
    use crate::input::{action_to_transitions, Action};
    use crate::model::Location;
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::{test_state, FileEntryBuilder};
    use std::path::PathBuf;

    fn create_test_state() -> AppState {
        let mut state = test_state();

        // Add some test entries to the active pane
        let entries = vec![
            FileEntryBuilder::new("file1.txt").size(100).build(),
            FileEntryBuilder::new("file2.txt").size(200).build(),
            FileEntryBuilder::new("dir1").dir(true).size(0).build(),
        ];

        state.current_tab_mut().left_pane.entries = entries;
        state.current_tab_mut().left_pane.current_location =
            Location::Local(PathBuf::from("/test"));
        state.current_tab_mut().right_pane.current_location =
            Location::Local(PathBuf::from("/dest"));

        state
    }

    #[test]
    fn test_copy_action_shows_dialog() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::Copy);

        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::CreatePendingFileJob { spec, name, .. } => {
                assert!(name.contains("Copy"));
                match &spec.kind {
                    crate::job::JobKind::Copy { sources, dest } => {
                        assert_eq!(sources.len(), 1);
                        assert_eq!(
                            sources[0],
                            Location::Local(PathBuf::from("/test/file1.txt"))
                        );
                        assert_eq!(*dest, Location::Local(PathBuf::from("/dest")));
                    }
                    _ => panic!("Expected Copy job kind"),
                }
            }
            _ => panic!("Expected CreatePendingFileJob transition"),
        }
    }

    #[test]
    fn test_copy_action_with_marked_files() {
        let mut state = create_test_state();

        // Mark two files
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(Location::Local(PathBuf::from("/test/file1.txt")));
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(Location::Local(PathBuf::from("/test/file2.txt")));

        let transitions = action_to_transitions(&state, &Action::Copy);

        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::CreatePendingFileJob { spec, .. } => match &spec.kind {
                crate::job::JobKind::Copy { sources, .. } => {
                    assert_eq!(sources.len(), 2);
                }
                _ => panic!("Expected Copy job kind"),
            },
            _ => panic!("Expected CreatePendingFileJob transition"),
        }
    }

    #[test]
    fn test_move_action_shows_dialog() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::Move);

        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::CreatePendingFileJob { spec, name, .. } => {
                assert!(name.contains("Move"));
                match &spec.kind {
                    crate::job::JobKind::Move { sources, dest } => {
                        assert_eq!(sources.len(), 1);
                        assert_eq!(*dest, Location::Local(PathBuf::from("/dest")));
                    }
                    _ => panic!("Expected Move job kind"),
                }
            }
            _ => panic!("Expected CreatePendingFileJob transition"),
        }
    }

    #[test]
    fn test_delete_action_shows_dialog() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::Delete);

        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ShowDialog { dialog } => {
                assert!(dialog.title.contains("Delete"));
                assert!(matches!(
                    dialog.content,
                    crate::model::DialogContent::DeleteConfirm(_)
                ));
            }
            _ => panic!("Expected ShowDialog transition"),
        }
    }

    #[test]
    fn test_rename_action_shows_input_dialog() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::Rename);

        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ShowDialog { dialog } => {
                assert_eq!(dialog.title, "Rename");
                match &dialog.content {
                    crate::model::DialogContent::SimpleRename { input, .. } => {
                        assert_eq!(input, "file1.txt");
                    }
                    _ => panic!("Expected SimpleRename dialog content"),
                }
            }
            _ => panic!("Expected ShowDialog transition"),
        }
    }

    #[test]
    fn test_create_directory_action_shows_input_dialog() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::CreateDirectory);

        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ShowDialog { dialog } => {
                assert_eq!(dialog.title, "Create Directory");
                match &dialog.content {
                    crate::model::DialogContent::Input {
                        prompt,
                        default_value,
                        ..
                    } => {
                        assert_eq!(prompt, "Directory name:");
                        assert_eq!(default_value, "");
                    }
                    _ => panic!("Expected input dialog"),
                }
            }
            _ => panic!("Expected ShowDialog transition"),
        }
    }

    #[test]
    fn test_confirm_copy_dialog_creates_job() {
        let mut state = create_test_state();

        // Copy now uses CreatePendingFileJob — no dialog shown
        let transitions = action_to_transitions(&state, &Action::Copy);
        assert_eq!(transitions.len(), 1);

        let result = update_state(&mut state, transitions.into_iter().next().unwrap());

        // CreatePendingFileJob immediately produces jobs_to_start
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            crate::job::JobKind::Copy { sources, dest } => {
                assert_eq!(sources.len(), 1);
                assert_eq!(
                    sources[0],
                    Location::Local(PathBuf::from("/test/file1.txt"))
                );
                assert_eq!(*dest, Location::Local(PathBuf::from("/dest")));
            }
            _ => panic!("Expected Copy job"),
        }

        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn test_confirm_move_dialog_creates_job() {
        let mut state = create_test_state();

        let transitions = action_to_transitions(&state, &Action::Move);
        assert_eq!(transitions.len(), 1);

        let result = update_state(&mut state, transitions.into_iter().next().unwrap());

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            crate::job::JobKind::Move { sources, dest } => {
                assert_eq!(sources.len(), 1);
                assert_eq!(*dest, Location::Local(PathBuf::from("/dest")));
            }
            _ => panic!("Expected Move job"),
        }
    }

    #[test]
    fn test_confirm_delete_dialog_creates_job() {
        let mut state = create_test_state();

        // Show and confirm delete dialog
        let transitions = action_to_transitions(&state, &Action::Delete);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        let result = update_state(&mut state, Transition::ConfirmDialog);

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            crate::job::JobKind::Delete { targets } => {
                assert_eq!(targets.len(), 1);
                assert_eq!(
                    targets[0],
                    Location::Local(PathBuf::from("/test/file1.txt"))
                );
            }
            _ => panic!("Expected Delete job"),
        }
    }

    #[test]
    fn test_confirm_rename_dialog_creates_job() {
        let mut state = create_test_state();

        // Show rename dialog
        let transitions = action_to_transitions(&state, &Action::Rename);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Set the new name in the input buffer
        state.dialogs.input_buffer = "newname.txt".to_string();

        let result = update_state(&mut state, Transition::ConfirmDialog);

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            crate::job::JobKind::Rename { from, to } => {
                assert_eq!(*from, Location::Local(PathBuf::from("/test/file1.txt")));
                assert_eq!(*to, Location::Local(PathBuf::from("/test/newname.txt")));
            }
            _ => panic!("Expected Rename job"),
        }
    }

    #[test]
    fn test_confirm_mkdir_dialog_creates_job() {
        let mut state = create_test_state();

        // Show create directory dialog
        let transitions = action_to_transitions(&state, &Action::CreateDirectory);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Set the directory name in the input buffer
        state.dialogs.input_buffer = "newdir".to_string();

        let result = update_state(&mut state, Transition::ConfirmDialog);

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            crate::job::JobKind::Mkdir { location } => {
                assert_eq!(*location, Location::Local(PathBuf::from("/test/newdir")));
            }
            _ => panic!("Expected Mkdir job"),
        }
    }

    #[test]
    fn test_cancel_dialog_closes_without_job() {
        let mut state = create_test_state();

        // Delete still shows a dialog — use it to test cancel
        let transitions = action_to_transitions(&state, &Action::Delete);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        assert!(!state.dialogs.is_empty());

        // Cancel the dialog
        let result = update_state(&mut state, Transition::CancelDialog);

        // Should not create any jobs
        assert_eq!(result.jobs_to_start.len(), 0);

        // Dialog should be closed
        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn test_copy_with_no_entries_returns_empty() {
        let state = test_state();

        let transitions = action_to_transitions(&state, &Action::Copy);

        // Should return empty transitions when there are no entries
        assert_eq!(transitions.len(), 0);
    }

    #[test]
    fn test_rename_with_no_entries_returns_empty() {
        let state = test_state();

        let transitions = action_to_transitions(&state, &Action::Rename);

        // Should return empty transitions when there are no entries
        assert_eq!(transitions.len(), 0);
    }

    // Integration tests for actual copy execution

    #[tokio::test]
    async fn test_single_file_copy_execution() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create source file
        let source_file = temp_path.join("source.txt");
        tokio::fs::write(&source_file, b"test content")
            .await
            .unwrap();

        // Create destination directory
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        // Execute copy job
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(source_file.clone())],
            dest: Location::Local(dest_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify events
        let mut received_started = false;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Started(_) => received_started = true,
                crate::worker_pool::JobEvent::Completed(_, _) => received_completed = true,
                _ => {}
            }
        }

        assert!(received_started, "Should receive Started event");
        assert!(received_completed, "Should receive Completed event");

        // Verify file was copied
        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists(), "Destination file should exist");

        let content = tokio::fs::read(&dest_file).await.unwrap();
        assert_eq!(content, b"test content", "File content should match");

        // Verify source still exists
        assert!(source_file.exists(), "Source file should still exist");
    }

    #[tokio::test]
    async fn test_multiple_file_copy_execution() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create multiple source files
        let source1 = temp_path.join("file1.txt");
        let source2 = temp_path.join("file2.txt");
        let source3 = temp_path.join("file3.txt");

        tokio::fs::write(&source1, b"content 1").await.unwrap();
        tokio::fs::write(&source2, b"content 2").await.unwrap();
        tokio::fs::write(&source3, b"content 3").await.unwrap();

        // Create destination directory
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        // Execute copy job with multiple sources
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Copy {
            sources: vec![
                Location::Local(source1.clone()),
                Location::Local(source2.clone()),
                Location::Local(source3.clone()),
            ],
            dest: Location::Local(dest_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify events
        let mut received_progress = false;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Progress(_, _) => received_progress = true,
                crate::worker_pool::JobEvent::Completed(_, _) => received_completed = true,
                _ => {}
            }
        }

        assert!(received_progress, "Should receive Progress events");
        assert!(received_completed, "Should receive Completed event");

        // Verify all files were copied
        assert!(
            dest_dir.join("file1.txt").exists(),
            "file1.txt should be copied"
        );
        assert!(
            dest_dir.join("file2.txt").exists(),
            "file2.txt should be copied"
        );
        assert!(
            dest_dir.join("file3.txt").exists(),
            "file3.txt should be copied"
        );

        // Verify content
        let content1 = tokio::fs::read(dest_dir.join("file1.txt")).await.unwrap();
        assert_eq!(content1, b"content 1");

        let content2 = tokio::fs::read(dest_dir.join("file2.txt")).await.unwrap();
        assert_eq!(content2, b"content 2");

        let content3 = tokio::fs::read(dest_dir.join("file3.txt")).await.unwrap();
        assert_eq!(content3, b"content 3");
    }

    #[tokio::test]
    async fn test_copy_with_overwrite() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create source file
        let source_file = temp_path.join("source.txt");
        tokio::fs::write(&source_file, b"new content")
            .await
            .unwrap();

        // Create destination directory with existing file
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        let dest_file = dest_dir.join("source.txt");
        tokio::fs::write(&dest_file, b"old content").await.unwrap();

        // Execute copy job (should overwrite)
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(source_file.clone())],
            dest: Location::Local(dest_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify completion
        let mut received_completed = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Completed(_, _)) {
                received_completed = true;
            }
        }

        assert!(received_completed, "Should complete successfully");

        // Verify file was overwritten with new content
        let content = tokio::fs::read(&dest_file).await.unwrap();
        assert_eq!(
            content, b"new content",
            "File should be overwritten with new content"
        );
    }

    #[tokio::test]
    async fn test_copy_cancellation() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create multiple source files to increase chance of catching cancellation
        let mut sources = Vec::new();
        for i in 0..10 {
            let source = temp_path.join(format!("file{}.txt", i));
            tokio::fs::write(&source, vec![0u8; 1024]).await.unwrap();
            sources.push(Location::Local(source));
        }

        // Create destination directory
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        // Execute copy job with cancellation
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Copy {
            sources,
            dest: Location::Local(dest_dir.clone()),
        });

        // Cancel the job immediately
        spec.cancel_token.cancel();

        executor.execute(spec).await;

        // Verify we received a cancelled event
        let mut received_cancelled = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Cancelled(_)) {
                received_cancelled = true;
            }
        }

        assert!(received_cancelled, "Should receive Cancelled event");
    }

    #[tokio::test]
    async fn test_copy_directory_recursive() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create source directory with nested structure
        let source_dir = temp_path.join("source_dir");
        tokio::fs::create_dir(&source_dir).await.unwrap();

        tokio::fs::write(source_dir.join("file1.txt"), b"content 1")
            .await
            .unwrap();

        let subdir = source_dir.join("subdir");
        tokio::fs::create_dir(&subdir).await.unwrap();
        tokio::fs::write(subdir.join("file2.txt"), b"content 2")
            .await
            .unwrap();

        // Create destination directory
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        // Execute copy job
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(source_dir.clone())],
            dest: Location::Local(dest_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify completion
        let mut received_completed = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Completed(_, _)) {
                received_completed = true;
            }
        }

        assert!(received_completed, "Should complete successfully");

        // Verify directory structure was copied
        let copied_dir = dest_dir.join("source_dir");
        assert!(copied_dir.exists(), "Directory should be copied");
        assert!(
            copied_dir.join("file1.txt").exists(),
            "file1.txt should be copied"
        );
        assert!(
            copied_dir.join("subdir").exists(),
            "subdir should be copied"
        );
        assert!(
            copied_dir.join("subdir").join("file2.txt").exists(),
            "file2.txt should be copied"
        );

        // Verify content
        let content1 = tokio::fs::read(copied_dir.join("file1.txt")).await.unwrap();
        assert_eq!(content1, b"content 1");

        let content2 = tokio::fs::read(copied_dir.join("subdir").join("file2.txt"))
            .await
            .unwrap();
        assert_eq!(content2, b"content 2");
    }

    // Integration tests for move operation execution

    #[tokio::test]
    async fn test_single_file_move_execution() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create source file
        let source_file = temp_path.join("source.txt");
        tokio::fs::write(&source_file, b"test content")
            .await
            .unwrap();

        // Create destination directory
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        // Execute move job
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Move {
            sources: vec![Location::Local(source_file.clone())],
            dest: Location::Local(dest_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify events
        let mut received_started = false;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Started(_) => received_started = true,
                crate::worker_pool::JobEvent::Completed(_, _) => received_completed = true,
                _ => {}
            }
        }

        assert!(received_started, "Should receive Started event");
        assert!(received_completed, "Should receive Completed event");

        // Verify file was moved
        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists(), "Destination file should exist");

        let content = tokio::fs::read(&dest_file).await.unwrap();
        assert_eq!(content, b"test content", "File content should match");

        // Verify source no longer exists
        assert!(
            !source_file.exists(),
            "Source file should be removed after move"
        );
    }

    #[tokio::test]
    async fn test_multiple_file_move_execution() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create multiple source files
        let source1 = temp_path.join("file1.txt");
        let source2 = temp_path.join("file2.txt");
        let source3 = temp_path.join("file3.txt");

        tokio::fs::write(&source1, b"content 1").await.unwrap();
        tokio::fs::write(&source2, b"content 2").await.unwrap();
        tokio::fs::write(&source3, b"content 3").await.unwrap();

        // Create destination directory
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        // Execute move job with multiple sources
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Move {
            sources: vec![
                Location::Local(source1.clone()),
                Location::Local(source2.clone()),
                Location::Local(source3.clone()),
            ],
            dest: Location::Local(dest_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify events
        let mut received_progress = false;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Progress(_, _) => received_progress = true,
                crate::worker_pool::JobEvent::Completed(_, _) => received_completed = true,
                _ => {}
            }
        }

        assert!(received_progress, "Should receive Progress events");
        assert!(received_completed, "Should receive Completed event");

        // Verify all files were moved
        assert!(
            dest_dir.join("file1.txt").exists(),
            "file1.txt should be moved"
        );
        assert!(
            dest_dir.join("file2.txt").exists(),
            "file2.txt should be moved"
        );
        assert!(
            dest_dir.join("file3.txt").exists(),
            "file3.txt should be moved"
        );

        // Verify content
        let content1 = tokio::fs::read(dest_dir.join("file1.txt")).await.unwrap();
        assert_eq!(content1, b"content 1");

        let content2 = tokio::fs::read(dest_dir.join("file2.txt")).await.unwrap();
        assert_eq!(content2, b"content 2");

        let content3 = tokio::fs::read(dest_dir.join("file3.txt")).await.unwrap();
        assert_eq!(content3, b"content 3");

        // Verify source files no longer exist
        assert!(!source1.exists(), "source1 should be removed");
        assert!(!source2.exists(), "source2 should be removed");
        assert!(!source3.exists(), "source3 should be removed");
    }

    #[tokio::test]
    async fn test_move_with_overwrite() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create source file
        let source_file = temp_path.join("source.txt");
        tokio::fs::write(&source_file, b"new content")
            .await
            .unwrap();

        // Create destination directory with existing file
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        let dest_file = dest_dir.join("source.txt");
        tokio::fs::write(&dest_file, b"old content").await.unwrap();

        // Execute move job (should overwrite)
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Move {
            sources: vec![Location::Local(source_file.clone())],
            dest: Location::Local(dest_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify completion
        let mut received_completed = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Completed(_, _)) {
                received_completed = true;
            }
        }

        assert!(received_completed, "Should complete successfully");

        // Verify file was overwritten with new content
        let content = tokio::fs::read(&dest_file).await.unwrap();
        assert_eq!(
            content, b"new content",
            "File should be overwritten with new content"
        );

        // Verify source no longer exists
        assert!(
            !source_file.exists(),
            "Source file should be removed after move"
        );
    }

    #[tokio::test]
    async fn test_move_cancellation() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create multiple source files to increase chance of catching cancellation
        let mut sources = Vec::new();
        for i in 0..10 {
            let source = temp_path.join(format!("file{}.txt", i));
            tokio::fs::write(&source, vec![0u8; 1024]).await.unwrap();
            sources.push(Location::Local(source));
        }

        // Create destination directory
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        // Execute move job with cancellation
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Move {
            sources,
            dest: Location::Local(dest_dir.clone()),
        });

        // Cancel the job immediately
        spec.cancel_token.cancel();

        executor.execute(spec).await;

        // Verify we received a cancelled event
        let mut received_cancelled = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Cancelled(_)) {
                received_cancelled = true;
            }
        }

        assert!(received_cancelled, "Should receive Cancelled event");
    }

    #[tokio::test]
    async fn test_single_file_delete_execution() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file to delete
        let file_path = temp_path.join("to_delete.txt");
        tokio::fs::write(&file_path, b"delete me").await.unwrap();

        // Execute delete job
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Delete {
            targets: vec![Location::Local(file_path.clone())],
        });

        executor.execute(spec).await;

        // Verify file was deleted
        assert!(!file_path.exists(), "File should be deleted");

        // Verify we received progress and completed events
        let mut received_progress = false;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Progress(_, progress) => {
                    received_progress = true;
                    assert!(
                        (0.0..=1.0).contains(&progress),
                        "Progress should be between 0 and 1"
                    );
                }
                crate::worker_pool::JobEvent::Completed(_, _) => {
                    received_completed = true;
                }
                _ => {}
            }
        }

        assert!(received_progress, "Should receive progress updates");
        assert!(received_completed, "Should receive completed event");
    }

    #[tokio::test]
    async fn test_multiple_file_delete_execution() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create multiple files to delete
        let file1 = temp_path.join("file1.txt");
        let file2 = temp_path.join("file2.txt");
        let file3 = temp_path.join("file3.txt");

        tokio::fs::write(&file1, b"content1").await.unwrap();
        tokio::fs::write(&file2, b"content2").await.unwrap();
        tokio::fs::write(&file3, b"content3").await.unwrap();

        // Execute delete job with multiple targets
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let targets = vec![
            Location::Local(file1.clone()),
            Location::Local(file2.clone()),
            Location::Local(file3.clone()),
        ];

        let spec = JobSpec::new(JobKind::Delete { targets });

        executor.execute(spec).await;

        // Verify all files were deleted
        assert!(!file1.exists(), "File1 should be deleted");
        assert!(!file2.exists(), "File2 should be deleted");
        assert!(!file3.exists(), "File3 should be deleted");

        // Verify we received progress and completed events
        let mut progress_count = 0;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Progress(_, progress) => {
                    progress_count += 1;
                    assert!(
                        (0.0..=1.0).contains(&progress),
                        "Progress should be between 0 and 1"
                    );
                }
                crate::worker_pool::JobEvent::Completed(_, _) => {
                    received_completed = true;
                }
                _ => {}
            }
        }

        assert!(progress_count > 0, "Should receive progress updates");
        assert!(received_completed, "Should receive completed event");
    }

    #[tokio::test]
    async fn test_delete_cancellation() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create multiple files to increase chance of catching cancellation
        let mut targets = Vec::new();
        for i in 0..10 {
            let file = temp_path.join(format!("file{}.txt", i));
            tokio::fs::write(&file, vec![0u8; 1024]).await.unwrap();
            targets.push(Location::Local(file));
        }

        // Execute delete job with cancellation
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Delete {
            targets: targets.clone(),
        });

        // Cancel the job immediately
        spec.cancel_token.cancel();

        executor.execute(spec).await;

        // Verify we received a cancelled event
        let mut received_cancelled = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Cancelled(_)) {
                received_cancelled = true;
            }
        }

        assert!(received_cancelled, "Should receive Cancelled event");
    }

    #[tokio::test]
    async fn test_delete_directory_recursive() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a directory with nested files
        let dir_to_delete = temp_path.join("dir_to_delete");
        tokio::fs::create_dir(&dir_to_delete).await.unwrap();

        let file1 = dir_to_delete.join("file1.txt");
        let subdir = dir_to_delete.join("subdir");
        tokio::fs::write(&file1, b"content1").await.unwrap();
        tokio::fs::create_dir(&subdir).await.unwrap();

        let file2 = subdir.join("file2.txt");
        tokio::fs::write(&file2, b"content2").await.unwrap();

        // Execute delete job
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Delete {
            targets: vec![Location::Local(dir_to_delete.clone())],
        });

        executor.execute(spec).await;

        // Verify directory and all contents were deleted
        assert!(!dir_to_delete.exists(), "Directory should be deleted");

        // Verify we received completed event
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            if let crate::worker_pool::JobEvent::Completed(_, _) = event {
                received_completed = true;
            }
        }

        assert!(received_completed, "Should receive completed event");
    }

    #[test]
    fn test_delete_unmarking() {
        use crate::job::{JobKind, JobSpec};
        use crate::state::{update_state, Transition};

        let mut state = create_test_state();

        // Mark some files
        let file1 = Location::Local(PathBuf::from("/test/file1.txt"));
        let file2 = Location::Local(PathBuf::from("/test/file2.txt"));

        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(file1.clone());
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(file2.clone());

        assert_eq!(
            state.current_tab_mut().left_pane.marking.count(),
            2,
            "Should have 2 marked files"
        );

        // Create and enqueue delete job
        let spec = JobSpec::new(JobKind::Delete {
            targets: vec![file1, file2],
        });
        let job_id = spec.id;

        // Enqueue the job
        update_state(&mut state, Transition::EnqueueJob { spec: spec.clone() });

        // Start the job
        state.jobs.start_job(spec);

        // Simulate delete job completion
        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: crate::job::OpResult::Success(crate::job::SuccessData::None),
            },
        );

        // Verify files were unmarked
        assert_eq!(
            state.current_tab_mut().left_pane.marking.count(),
            0,
            "All files should be unmarked after delete"
        );
        // Successful delete does in-memory removal — no pane refresh needed
        assert!(result.ui_changed || result.panes_to_refresh.is_empty());
    }

    // Integration tests for rename operation execution

    #[tokio::test]
    async fn test_rename_with_valid_name() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file to rename
        let old_path = temp_path.join("old_name.txt");
        tokio::fs::write(&old_path, b"test content").await.unwrap();

        let new_path = temp_path.join("new_name.txt");

        // Execute rename job
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Rename {
            from: Location::Local(old_path.clone()),
            to: Location::Local(new_path.clone()),
        });

        executor.execute(spec).await;

        // Verify events
        let mut received_started = false;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Started(_) => received_started = true,
                crate::worker_pool::JobEvent::Completed(_, _) => received_completed = true,
                _ => {}
            }
        }

        assert!(received_started, "Should receive Started event");
        assert!(received_completed, "Should receive Completed event");

        // Verify file was renamed
        assert!(!old_path.exists(), "Old file should not exist");
        assert!(new_path.exists(), "New file should exist");

        let content = tokio::fs::read(&new_path).await.unwrap();
        assert_eq!(content, b"test content", "File content should be preserved");
    }

    #[tokio::test]
    async fn test_rename_with_conflict() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create two files
        let old_path = temp_path.join("old_name.txt");
        let new_path = temp_path.join("existing_name.txt");

        tokio::fs::write(&old_path, b"old content").await.unwrap();
        tokio::fs::write(&new_path, b"existing content")
            .await
            .unwrap();

        // Execute rename job (should fail due to conflict)
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Rename {
            from: Location::Local(old_path.clone()),
            to: Location::Local(new_path.clone()),
        });

        executor.execute(spec).await;

        // Verify we received a failed event
        let mut received_failed = false;
        while let Ok(event) = event_rx.try_recv() {
            if let crate::worker_pool::JobEvent::Failed(_, error) = event {
                received_failed = true;
                assert!(
                    error.contains("already exists"),
                    "Error should mention file already exists"
                );
            }
        }

        assert!(received_failed, "Should receive Failed event");

        // Verify original files are unchanged
        assert!(old_path.exists(), "Old file should still exist");
        assert!(new_path.exists(), "Existing file should still exist");

        let old_content = tokio::fs::read(&old_path).await.unwrap();
        assert_eq!(
            old_content, b"old content",
            "Old file content should be unchanged"
        );

        let existing_content = tokio::fs::read(&new_path).await.unwrap();
        assert_eq!(
            existing_content, b"existing content",
            "Existing file content should be unchanged"
        );
    }

    #[tokio::test]
    async fn test_rename_with_invalid_characters() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file to rename
        let old_path = temp_path.join("old_name.txt");
        tokio::fs::write(&old_path, b"test content").await.unwrap();

        // Try to rename with invalid characters
        let new_path = temp_path.join("invalid<name>.txt");

        // Execute rename job (should fail due to invalid characters)
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Rename {
            from: Location::Local(old_path.clone()),
            to: Location::Local(new_path.clone()),
        });

        executor.execute(spec).await;

        // Verify we received a failed event
        let mut received_failed = false;
        while let Ok(event) = event_rx.try_recv() {
            if let crate::worker_pool::JobEvent::Failed(_, error) = event {
                received_failed = true;
                assert!(
                    error.contains("Invalid characters"),
                    "Error should mention invalid characters"
                );
            }
        }

        assert!(received_failed, "Should receive Failed event");

        // Verify original file is unchanged
        assert!(old_path.exists(), "Old file should still exist");
        assert!(!new_path.exists(), "New file should not be created");
    }

    #[tokio::test]
    async fn test_rename_directory() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a directory to rename
        let old_dir = temp_path.join("old_dir");
        tokio::fs::create_dir(&old_dir).await.unwrap();

        // Add a file inside the directory
        let file_in_dir = old_dir.join("file.txt");
        tokio::fs::write(&file_in_dir, b"content").await.unwrap();

        let new_dir = temp_path.join("new_dir");

        // Execute rename job
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Rename {
            from: Location::Local(old_dir.clone()),
            to: Location::Local(new_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify completion
        let mut received_completed = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Completed(_, _)) {
                received_completed = true;
            }
        }

        assert!(received_completed, "Should complete successfully");

        // Verify directory was renamed
        assert!(!old_dir.exists(), "Old directory should not exist");
        assert!(new_dir.exists(), "New directory should exist");

        // Verify file inside directory is preserved
        let file_in_new_dir = new_dir.join("file.txt");
        assert!(
            file_in_new_dir.exists(),
            "File inside directory should be preserved"
        );

        let content = tokio::fs::read(&file_in_new_dir).await.unwrap();
        assert_eq!(content, b"content", "File content should be preserved");
    }

    // Integration tests for mkdir operation execution

    #[tokio::test]
    async fn test_mkdir_with_valid_name() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let new_dir = temp_path.join("new_directory");

        // Execute mkdir job
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(new_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify events
        let mut received_started = false;
        let mut received_completed = false;

        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Started(_) => received_started = true,
                crate::worker_pool::JobEvent::Completed(_, _) => received_completed = true,
                _ => {}
            }
        }

        assert!(received_started, "Should receive Started event");
        assert!(received_completed, "Should receive Completed event");

        // Verify directory was created
        assert!(new_dir.exists(), "Directory should be created");
        assert!(new_dir.is_dir(), "Should be a directory");
    }

    #[tokio::test]
    async fn test_mkdir_with_conflict() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create an existing directory
        let existing_dir = temp_path.join("existing_dir");
        tokio::fs::create_dir(&existing_dir).await.unwrap();

        // Execute mkdir job (should fail due to conflict)
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(existing_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify we received a failed event
        let mut received_failed = false;
        while let Ok(event) = event_rx.try_recv() {
            if let crate::worker_pool::JobEvent::Failed(_, error) = event {
                received_failed = true;
                assert!(
                    error.contains("already exists"),
                    "Error should mention directory already exists"
                );
            }
        }

        assert!(received_failed, "Should receive Failed event");

        // Verify directory still exists
        assert!(existing_dir.exists(), "Directory should still exist");
    }

    #[tokio::test]
    async fn test_mkdir_with_invalid_characters() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Try to create directory with invalid characters
        let invalid_dir = temp_path.join("invalid<dir>");

        // Execute mkdir job (should fail due to invalid characters)
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(invalid_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify we received a failed event
        let mut received_failed = false;
        while let Ok(event) = event_rx.try_recv() {
            if let crate::worker_pool::JobEvent::Failed(_, error) = event {
                received_failed = true;
                assert!(
                    error.contains("Invalid characters"),
                    "Error should mention invalid characters"
                );
            }
        }

        assert!(received_failed, "Should receive Failed event");

        // Verify directory was not created
        assert!(!invalid_dir.exists(), "Directory should not be created");
    }

    #[tokio::test]
    async fn test_mkdir_nested_path() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create nested directory path
        let nested_dir = temp_path.join("parent").join("child").join("grandchild");

        // Execute mkdir job (should create all parent directories)
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(nested_dir.clone()),
        });

        executor.execute(spec).await;

        // Verify completion
        let mut received_completed = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Completed(_, _)) {
                received_completed = true;
            }
        }

        assert!(received_completed, "Should complete successfully");

        // Verify all directories were created
        assert!(nested_dir.exists(), "Nested directory should be created");
        assert!(nested_dir.is_dir(), "Should be a directory");

        // Verify parent directories were also created
        assert!(
            temp_path.join("parent").exists(),
            "Parent directory should be created"
        );
        assert!(
            temp_path.join("parent").join("child").exists(),
            "Child directory should be created"
        );
    }

    #[tokio::test]
    async fn test_mkdir_cancellation() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let new_dir = temp_path.join("cancelled_dir");

        // Execute mkdir job with cancellation
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(new_dir.clone()),
        });

        // Cancel the job immediately
        spec.cancel_token.cancel();

        executor.execute(spec).await;

        // Verify we received a cancelled event
        let mut received_cancelled = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Cancelled(_)) {
                received_cancelled = true;
            }
        }

        assert!(received_cancelled, "Should receive Cancelled event");
    }

    #[tokio::test]
    async fn test_rename_cancellation() {
        use crate::backend::LocalFilesystemBackend;
        use crate::job::job_executor::JobExecutor;
        use crate::job::{JobKind, JobSpec};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file to rename
        let old_path = temp_path.join("old_name.txt");
        tokio::fs::write(&old_path, b"test content").await.unwrap();

        let new_path = temp_path.join("new_name.txt");

        // Execute rename job with cancellation
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let executor = JobExecutor::new(backend, archive_handler, event_tx);

        let spec = JobSpec::new(JobKind::Rename {
            from: Location::Local(old_path.clone()),
            to: Location::Local(new_path.clone()),
        });

        // Cancel the job immediately
        spec.cancel_token.cancel();

        executor.execute(spec).await;

        // Verify we received a cancelled event
        let mut received_cancelled = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, crate::worker_pool::JobEvent::Cancelled(_)) {
                received_cancelled = true;
            }
        }

        assert!(received_cancelled, "Should receive Cancelled event");

        // Verify file was not renamed
        assert!(old_path.exists(), "Old file should still exist");
        assert!(!new_path.exists(), "New file should not be created");
    }
}
