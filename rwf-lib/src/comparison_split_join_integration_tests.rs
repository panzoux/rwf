//! Integration tests for file comparison and split/join operations
//!
//! These tests verify that file comparison, split, and join operations work correctly
//! as background jobs with proper progress reporting and UI responsiveness.

#[cfg(test)]
mod tests {
    use crate::job::{JobSpec, JobKind, SuccessData, DiffType};
    use crate::model::Location;
    use crate::state::{AppState, update_state, Transition};
    use crate::config::AppConfig;
    use tempfile::TempDir;
    use std::path::PathBuf;

    /// Test file comparison job creation and execution
    #[tokio::test]
    async fn test_file_comparison() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create two test files with different content
        let left_file = temp_dir.path().join("left.txt");
        let right_file = temp_dir.path().join("right.txt");
        
        tokio::fs::write(&left_file, "line 1\nline 2\nline 3\n").await.unwrap();
        tokio::fs::write(&right_file, "line 1\nline 2 modified\nline 3\n").await.unwrap();
        
        // Create comparison job
        let left_location = Location::Local(left_file.clone());
        let right_location = Location::Local(right_file.clone());
        
        let job_spec = JobSpec::new(JobKind::CompareFiles {
            left: left_location.clone(),
            right: right_location.clone(),
        });
        
        // Execute the job using the job executor
        use crate::job::JobExecutor;
        use crate::backend::LocalFilesystemBackend;
        use crate::backend::MockArchiveHandler;
        use tokio::sync::mpsc;
        use std::sync::Arc;
        
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        let executor = JobExecutor::new(backend, archive_handler, event_tx);
        executor.execute(job_spec).await;
        
        // Verify we received a started event
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(crate::worker_pool::JobEvent::Started(_))));
        
        // Verify we received a completed event with comparison result
        let event = event_rx.recv().await;
        match event {
            Some(crate::worker_pool::JobEvent::Completed(_, SuccessData::ComparisonResult(diff))) => {
                assert_eq!(diff.left_path, left_file.display().to_string());
                assert_eq!(diff.right_path, right_file.display().to_string());
                
                // Should have differences
                assert!(!diff.differences.is_empty());
                
                // Check that we detected the modification
                let has_modification = diff.differences.iter().any(|chunk| {
                    chunk.chunk_type == DiffType::Modified
                });
                assert!(has_modification, "Should detect modified line");
            }
            _ => panic!("Expected ComparisonResult"),
        }
    }

    /// Test file comparison with identical files
    #[tokio::test]
    async fn test_file_comparison_identical() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create two identical test files
        let left_file = temp_dir.path().join("left.txt");
        let right_file = temp_dir.path().join("right.txt");
        
        let content = "line 1\nline 2\nline 3\n";
        tokio::fs::write(&left_file, content).await.unwrap();
        tokio::fs::write(&right_file, content).await.unwrap();
        
        // Create comparison job
        let left_location = Location::Local(left_file.clone());
        let right_location = Location::Local(right_file.clone());
        
        let job_spec = JobSpec::new(JobKind::CompareFiles {
            left: left_location,
            right: right_location,
        });
        
        // Execute the job
        use crate::job::JobExecutor;
        use crate::backend::LocalFilesystemBackend;
        use crate::backend::MockArchiveHandler;
        use tokio::sync::mpsc;
        use std::sync::Arc;
        
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        let executor = JobExecutor::new(backend, archive_handler, event_tx);
        executor.execute(job_spec).await;
        
        // Skip started event
        let _ = event_rx.recv().await;
        
        // Verify we received a completed event with comparison result
        let event = event_rx.recv().await;
        match event {
            Some(crate::worker_pool::JobEvent::Completed(_, SuccessData::ComparisonResult(diff))) => {
                // All chunks should be Equal
                let all_equal = diff.differences.iter().all(|chunk| {
                    chunk.chunk_type == DiffType::Equal
                });
                assert!(all_equal, "All chunks should be equal for identical files");
            }
            _ => panic!("Expected ComparisonResult"),
        }
    }

    /// Test file split operation
    #[tokio::test]
    async fn test_file_split() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create a test file with known content
        let source_file = temp_dir.path().join("source.txt");
        let content = "a".repeat(1000); // 1000 bytes
        tokio::fs::write(&source_file, &content).await.unwrap();
        
        let dest_dir = temp_dir.path().join("parts");
        tokio::fs::create_dir(&dest_dir).await.unwrap();
        
        // Create split job with 300 byte chunks
        let source_location = Location::Local(source_file.clone());
        let dest_location = Location::Local(dest_dir.clone());
        
        let job_spec = JobSpec::new(JobKind::SplitFile {
            source: source_location,
            dest_dir: dest_location,
            chunk_size: 300,
        });
        
        // Execute the job
        use crate::job::JobExecutor;
        use crate::backend::LocalFilesystemBackend;
        use crate::backend::MockArchiveHandler;
        use tokio::sync::mpsc;
        use std::sync::Arc;
        
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        let executor = JobExecutor::new(backend, archive_handler, event_tx);
        executor.execute(job_spec).await;
        
        // Skip started event
        let _ = event_rx.recv().await;
        
        // Collect progress events
        let mut received_progress = false;
        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Progress(_, _) => {
                    received_progress = true;
                }
                crate::worker_pool::JobEvent::Completed(_, _) => break,
                _ => {}
            }
        }
        
        assert!(received_progress, "Should receive progress updates");
        
        // Verify split files were created
        let part_files: Vec<_> = std::fs::read_dir(&dest_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        
        // Should have 4 parts (1000 / 300 = 3.33, rounded up to 4)
        assert_eq!(part_files.len(), 4, "Should create 4 part files");
        
        // Verify total size matches original
        let total_size: u64 = part_files.iter()
            .map(|e| e.metadata().unwrap().len())
            .sum();
        assert_eq!(total_size, 1000, "Total size should match original");
    }

    /// Test file join operation
    #[tokio::test]
    async fn test_file_join() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test part files
        let parts_dir = temp_dir.path().join("parts");
        tokio::fs::create_dir(&parts_dir).await.unwrap();
        
        let part1 = parts_dir.join("file.part000");
        let part2 = parts_dir.join("file.part001");
        let part3 = parts_dir.join("file.part002");
        
        tokio::fs::write(&part1, "aaa").await.unwrap();
        tokio::fs::write(&part2, "bbb").await.unwrap();
        tokio::fs::write(&part3, "ccc").await.unwrap();
        
        let dest_file = temp_dir.path().join("joined.txt");
        
        // Create join job
        let parts = vec![
            Location::Local(part1),
            Location::Local(part2),
            Location::Local(part3),
        ];
        let dest_location = Location::Local(dest_file.clone());
        
        let job_spec = JobSpec::new(JobKind::JoinFiles {
            parts,
            dest: dest_location,
        });
        
        // Execute the job
        use crate::job::JobExecutor;
        use crate::backend::LocalFilesystemBackend;
        use crate::backend::MockArchiveHandler;
        use tokio::sync::mpsc;
        use std::sync::Arc;
        
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        let executor = JobExecutor::new(backend, archive_handler, event_tx);
        executor.execute(job_spec).await;
        
        // Skip started event
        let _ = event_rx.recv().await;
        
        // Collect progress events
        let mut received_progress = false;
        while let Ok(event) = event_rx.try_recv() {
            match event {
                crate::worker_pool::JobEvent::Progress(_, _) => {
                    received_progress = true;
                }
                crate::worker_pool::JobEvent::Completed(_, _) => break,
                _ => {}
            }
        }
        
        assert!(received_progress, "Should receive progress updates");
        
        // Verify joined file was created with correct content
        let joined_content = tokio::fs::read_to_string(&dest_file).await.unwrap();
        assert_eq!(joined_content, "aaabbbccc", "Joined content should match parts in order");
    }

    /// Test comparison transition creates job
    #[test]
    fn test_comparison_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let left = Location::Local(PathBuf::from("/test/left.txt"));
        let right = Location::Local(PathBuf::from("/test/right.txt"));
        
        let result = update_state(&mut state, Transition::CompareFiles {
            left: left.clone(),
            right: right.clone(),
        });
        
        // Should create a job
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify job kind
        match &result.jobs_to_start[0].kind {
            JobKind::CompareFiles { left: l, right: r } => {
                assert_eq!(l, &left);
                assert_eq!(r, &right);
            }
            _ => panic!("Expected CompareFiles job"),
        }
    }

    /// Test split transition creates job
    #[test]
    fn test_split_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let source = Location::Local(PathBuf::from("/test/source.txt"));
        let dest_dir = Location::Local(PathBuf::from("/test/parts"));
        let chunk_size = 1024 * 1024; // 1MB
        
        let result = update_state(&mut state, Transition::ExecuteFileSplit {
            source: source.clone(),
            dest_dir: dest_dir.clone(),
            chunk_size,
        });
        
        // Should create a job
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify job kind
        match &result.jobs_to_start[0].kind {
            JobKind::SplitFile { source: s, dest_dir: d, chunk_size: c } => {
                assert_eq!(s, &source);
                assert_eq!(d, &dest_dir);
                assert_eq!(*c, chunk_size);
            }
            _ => panic!("Expected SplitFile job"),
        }
    }

    /// Test join transition creates job
    #[test]
    fn test_join_transition() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let parts = vec![
            Location::Local(PathBuf::from("/test/file.part000")),
            Location::Local(PathBuf::from("/test/file.part001")),
        ];
        let dest = Location::Local(PathBuf::from("/test/joined.txt"));
        
        let result = update_state(&mut state, Transition::ExecuteFileJoin {
            parts: parts.clone(),
            dest: dest.clone(),
        });
        
        // Should create a job
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify job kind
        match &result.jobs_to_start[0].kind {
            JobKind::JoinFiles { parts: p, dest: d } => {
                assert_eq!(p, &parts);
                assert_eq!(d, &dest);
            }
            _ => panic!("Expected JoinFiles job"),
        }
    }
}
