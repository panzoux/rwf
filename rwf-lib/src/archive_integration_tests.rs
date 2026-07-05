//! Integration tests for archive operations
//!
//! Tests archive browsing, extraction, and creation.
//! **Validates: Requirements 29.1-29.11**

#[cfg(test)]
mod tests {
    use crate::backend::archive::{ArchiveHandler, ZipArchiveHandler};
    use crate::job::JobKind;
    use crate::model::Location;
    use crate::test_utils::test_state;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    /// Create a test ZIP archive with sample files
    fn create_test_archive(path: &std::path::Path) -> anyhow::Result<()> {
        let file = File::create(path)?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // Add a file at root
        zip.start_file("readme.txt", options)?;
        zip.write_all(b"This is a test archive")?;

        // Add a directory with files
        zip.add_directory("docs/", options)?;
        zip.start_file("docs/manual.txt", options)?;
        zip.write_all(b"User manual content")?;

        zip.start_file("docs/guide.txt", options)?;
        zip.write_all(b"Quick start guide")?;

        // Add nested directory
        zip.add_directory("docs/images/", options)?;
        zip.start_file("docs/images/logo.png", options)?;
        zip.write_all(b"fake png data")?;

        zip.finish()?;
        Ok(())
    }

    /// Test archive browsing - listing root contents
    /// **Validates: Requirements 29.1, 29.2**
    #[tokio::test]
    async fn test_archive_browsing_root() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.zip");
        create_test_archive(&archive_path).unwrap();

        let handler = ZipArchiveHandler::new();
        let archive_location = Location::Archive {
            archive_path: Box::new(Location::Local(archive_path.clone())),
            inner_path: PathBuf::new(),
        };

        let cancel_token = CancellationToken::new();
        let entries = handler
            .list_entries(&archive_location, &cancel_token)
            .await
            .unwrap();

        // Should have 2 entries at root: readme.txt and docs/
        assert_eq!(entries.len(), 2);

        // Check for readme.txt
        let readme = entries.iter().find(|e| e.name == "readme.txt");
        assert!(readme.is_some());
        let readme = readme.unwrap();
        assert!(!readme.is_dir);
        assert!(readme.size > 0);

        // Check for docs directory
        let docs = entries.iter().find(|e| e.name == "docs");
        assert!(docs.is_some());
        let docs = docs.unwrap();
        assert!(docs.is_dir);
    }

    /// Test archive browsing - navigating into subdirectory
    /// **Validates: Requirements 29.2, 29.3**
    #[tokio::test]
    async fn test_archive_browsing_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.zip");
        create_test_archive(&archive_path).unwrap();

        let handler = ZipArchiveHandler::new();
        let archive_location = Location::Archive {
            archive_path: Box::new(Location::Local(archive_path.clone())),
            inner_path: PathBuf::from("docs"),
        };

        let cancel_token = CancellationToken::new();
        let entries = handler
            .list_entries(&archive_location, &cancel_token)
            .await
            .unwrap();

        // Should have 3 entries in docs/: manual.txt, guide.txt, and images/
        assert_eq!(entries.len(), 3);

        // Check for files
        assert!(entries.iter().any(|e| e.name == "manual.txt" && !e.is_dir));
        assert!(entries.iter().any(|e| e.name == "guide.txt" && !e.is_dir));

        // Check for subdirectory
        assert!(entries.iter().any(|e| e.name == "images" && e.is_dir));
    }

    /// Test archive browsing - nested directory navigation
    /// **Validates: Requirements 29.3**
    #[tokio::test]
    async fn test_archive_browsing_nested_directory() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.zip");
        create_test_archive(&archive_path).unwrap();

        let handler = ZipArchiveHandler::new();
        let archive_location = Location::Archive {
            archive_path: Box::new(Location::Local(archive_path.clone())),
            inner_path: PathBuf::from("docs/images"),
        };

        let cancel_token = CancellationToken::new();
        let entries = handler
            .list_entries(&archive_location, &cancel_token)
            .await
            .unwrap();

        // Should have 1 entry in docs/images/: logo.png
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "logo.png");
        assert!(!entries[0].is_dir);
    }

    /// Test archive extraction - extract entire archive
    /// **Validates: Requirements 29.5, 29.7, 29.8**
    #[tokio::test]
    async fn test_archive_extraction_all() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.zip");
        create_test_archive(&archive_path).unwrap();

        let extract_dir = temp_dir.path().join("extracted");
        std::fs::create_dir(&extract_dir).unwrap();

        let handler = ZipArchiveHandler::new();
        let archive_location = Location::Local(archive_path.clone());
        let dest_location = Location::Local(extract_dir.clone());

        let cancel_token = CancellationToken::new();
        handler
            .extract_all(&archive_location, &dest_location, &cancel_token)
            .await
            .unwrap();

        // Verify extracted files exist
        assert!(extract_dir.join("readme.txt").exists());
        assert!(extract_dir.join("docs").is_dir());
        assert!(extract_dir.join("docs/manual.txt").exists());
        assert!(extract_dir.join("docs/guide.txt").exists());
        assert!(extract_dir.join("docs/images").is_dir());
        assert!(extract_dir.join("docs/images/logo.png").exists());

        // Verify content
        let content = std::fs::read_to_string(extract_dir.join("readme.txt")).unwrap();
        assert_eq!(content, "This is a test archive");
    }

    /// Test archive extraction - extract subdirectory
    /// **Validates: Requirements 29.5**
    #[tokio::test]
    async fn test_archive_extraction_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.zip");
        create_test_archive(&archive_path).unwrap();

        let extract_dir = temp_dir.path().join("extracted");
        std::fs::create_dir(&extract_dir).unwrap();

        let handler = ZipArchiveHandler::new();
        let archive_location = Location::Archive {
            archive_path: Box::new(Location::Local(archive_path.clone())),
            inner_path: PathBuf::from("docs"),
        };
        let dest_location = Location::Local(extract_dir.clone());

        let cancel_token = CancellationToken::new();
        handler
            .extract_all(&archive_location, &dest_location, &cancel_token)
            .await
            .unwrap();

        // Verify only docs contents were extracted (not readme.txt)
        assert!(!extract_dir.join("readme.txt").exists());
        assert!(extract_dir.join("manual.txt").exists());
        assert!(extract_dir.join("guide.txt").exists());
        assert!(extract_dir.join("images").is_dir());
        assert!(extract_dir.join("images/logo.png").exists());
    }

    /// Test archive creation - create archive from files
    /// **Validates: Requirements 29.6, 29.7, 29.8**
    #[tokio::test]
    async fn test_archive_creation() {
        let temp_dir = TempDir::new().unwrap();

        // Create source files
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, b"Content of file 1").unwrap();
        std::fs::write(&file2, b"Content of file 2").unwrap();

        // Create source directory with files
        let dir1 = temp_dir.path().join("subdir");
        std::fs::create_dir(&dir1).unwrap();
        let file3 = dir1.join("file3.txt");
        std::fs::write(&file3, b"Content of file 3").unwrap();

        let archive_path = temp_dir.path().join("output.zip");

        let handler = ZipArchiveHandler::new();
        let sources = vec![
            Location::Local(file1.clone()),
            Location::Local(file2.clone()),
            Location::Local(dir1.clone()),
        ];
        let dest_location = Location::Local(archive_path.clone());

        let cancel_token = CancellationToken::new();
        handler
            .create_archive(&sources, &dest_location, &cancel_token)
            .await
            .unwrap();

        // Verify archive was created
        assert!(archive_path.exists());

        // Verify archive contents by extracting
        let extract_dir = temp_dir.path().join("verify");
        std::fs::create_dir(&extract_dir).unwrap();

        let archive_location = Location::Local(archive_path.clone());
        let extract_location = Location::Local(extract_dir.clone());
        handler
            .extract_all(&archive_location, &extract_location, &cancel_token)
            .await
            .unwrap();

        // Verify extracted files
        assert!(extract_dir.join("file1.txt").exists());
        assert!(extract_dir.join("file2.txt").exists());
        assert!(extract_dir.join("subdir").is_dir());
        assert!(extract_dir.join("subdir/file3.txt").exists());

        // Verify content
        let content1 = std::fs::read_to_string(extract_dir.join("file1.txt")).unwrap();
        assert_eq!(content1, "Content of file 1");
    }

    /// Test archive operation cancellation
    /// **Validates: Requirements 29.7, 29.10**
    #[tokio::test]
    async fn test_archive_operation_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.zip");
        create_test_archive(&archive_path).unwrap();

        let handler = ZipArchiveHandler::new();
        let archive_location = Location::Archive {
            archive_path: Box::new(Location::Local(archive_path.clone())),
            inner_path: PathBuf::new(),
        };

        // Create a cancelled token
        let cancel_token = CancellationToken::new();
        cancel_token.cancel();

        // Attempt to list entries with cancelled token
        let result = handler.list_entries(&archive_location, &cancel_token).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    /// Test archive operations as jobs - job creation
    /// **Validates: Requirements 29.7, 29.8**
    #[test]
    fn test_archive_job_creation() {
        let mut state = test_state();

        // Create extract archive job
        let archive_location = Location::Local(PathBuf::from("/test/archive.zip"));
        let dest_location = Location::Local(PathBuf::from("/test/extracted"));

        let job_spec = crate::job::JobSpec::new(JobKind::ExtractArchive {
            archive: archive_location.clone(),
            dest: dest_location.clone(),
        });

        let _job_id = state.jobs.enqueue(job_spec);

        // Verify job was queued
        assert_eq!(state.jobs.queue.len(), 1);
        assert!(state.jobs.active.is_empty());

        // Verify job kind
        let queued_job = &state.jobs.queue[0];
        match &queued_job.kind {
            JobKind::ExtractArchive { archive, dest } => {
                assert_eq!(*archive, archive_location);
                assert_eq!(*dest, dest_location);
            }
            _ => panic!("Expected ExtractArchive job"),
        }
    }

    /// Test create archive job
    /// **Validates: Requirements 29.6, 29.7, 29.8**
    #[test]
    fn test_create_archive_job() {
        let mut state = test_state();

        // Create archive job
        let sources = vec![
            Location::Local(PathBuf::from("/test/file1.txt")),
            Location::Local(PathBuf::from("/test/file2.txt")),
        ];
        let dest_location = Location::Local(PathBuf::from("/test/archive.zip"));

        let job_spec = crate::job::JobSpec::new(JobKind::CreateArchive {
            sources: sources.clone(),
            dest: dest_location.clone(),
            original_size: 0,
        });

        let _job_id = state.jobs.enqueue(job_spec);

        // Verify job was queued
        assert_eq!(state.jobs.queue.len(), 1);

        // Verify job kind
        let queued_job = &state.jobs.queue[0];
        match &queued_job.kind {
            JobKind::CreateArchive {
                sources: job_sources,
                dest,
                original_size: _,
            } => {
                assert_eq!(*job_sources, sources);
                assert_eq!(*dest, dest_location);
            }
            _ => panic!("Expected CreateArchive job"),
        }
    }

    /// Test archive format detection
    /// **Validates: Requirements 29.9**
    #[test]
    fn test_archive_format_detection() {
        let handler = ZipArchiveHandler::new();

        // Test ZIP detection
        assert!(handler.is_archive("test.zip"));
        assert!(handler.is_archive("archive.ZIP"));
        assert!(handler.is_archive("/path/to/file.zip"));

        // Test non-archive files
        assert!(!handler.is_archive("test.txt"));
        assert!(!handler.is_archive("document.pdf"));
        assert!(!handler.is_archive("image.png"));
    }

    /// Test UI remains responsive during archive operations
    /// **Validates: Requirements 29.10**
    #[test]
    fn test_archive_operations_non_blocking() {
        let mut state = test_state();

        // Queue multiple archive operations
        for i in 0..5 {
            let job_spec = crate::job::JobSpec::new(JobKind::ExtractArchive {
                archive: Location::Local(PathBuf::from(format!("/test/archive{}.zip", i))),
                dest: Location::Local(PathBuf::from(format!("/test/extracted{}", i))),
            });
            state.jobs.enqueue(job_spec);
        }

        // All jobs should be queued (not blocking)
        assert_eq!(state.jobs.queue.len(), 5);
        assert!(state.jobs.active.is_empty());

        // State should still be accessible and modifiable
        state.ui.active_pane = crate::model::ActivePane::Right;
        assert_eq!(state.ui.active_pane, crate::model::ActivePane::Right);
    }
}
