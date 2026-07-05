//! Integration tests for 7z archive operations

#[cfg(test)]
mod tests {
    use crate::backend::archive::{
        ArchiveHandler, MultiFormatArchiveHandler, SevenZArchiveHandler,
    };
    use crate::model::Location;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    /// Create a test 7z archive with sample files using SevenZArchiveHandler itself
    async fn create_test_sevenz_archive(path: &std::path::Path) -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;

        // Create source files
        let readme = temp_dir.path().join("readme.txt");
        std::fs::write(&readme, b"This is a test archive")?;

        let docs_dir = temp_dir.path().join("docs");
        std::fs::create_dir_all(&docs_dir)?;
        std::fs::write(docs_dir.join("manual.txt"), b"User manual content")?;
        std::fs::write(docs_dir.join("guide.txt"), b"Quick start guide")?;

        let handler = SevenZArchiveHandler::new();
        let sources = vec![Location::Local(readme), Location::Local(docs_dir)];
        let dest = Location::Local(path.to_path_buf());
        let cancel = CancellationToken::new();
        handler.create_archive(&sources, &dest, &cancel).await?;
        Ok(())
    }

    // ── is_archive ────────────────────────────────────────────────────────────

    #[test]
    fn test_sevenz_handler_is_archive() {
        let h = SevenZArchiveHandler::new();
        assert!(h.is_archive("file.7z"));
        assert!(h.is_archive("FILE.7Z"));
        assert!(!h.is_archive("file.zip"));
        assert!(!h.is_archive("file.tar"));
        assert!(!h.is_archive("7z"));
    }

    #[test]
    fn test_multi_format_is_archive() {
        let h = MultiFormatArchiveHandler::new();
        assert!(h.is_archive("file.7z"));
        assert!(h.is_archive("file.zip"));
        assert!(h.is_archive("FILE.7Z"));
        assert!(h.is_archive("FILE.ZIP"));
        assert!(h.is_archive("file.tar"));
        assert!(h.is_archive("file.rar"));
        assert!(!h.is_archive("file.docx"));
    }

    // ── create + list ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sevenz_create_and_list_root() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.7z");
        create_test_sevenz_archive(&archive_path).await.unwrap();

        assert!(archive_path.exists(), "7z archive should be created");

        let handler = SevenZArchiveHandler::new();
        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(archive_path)),
            inner_path: PathBuf::new(),
        };
        let cancel = CancellationToken::new();
        let entries = handler.list_entries(&loc, &cancel).await.unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"readme.txt"),
            "root should contain readme.txt, got: {:?}",
            names
        );
        assert!(
            names.contains(&"docs"),
            "root should contain docs/, got: {:?}",
            names
        );

        let docs_entry = entries.iter().find(|e| e.name == "docs").unwrap();
        assert!(docs_entry.is_dir);
    }

    #[tokio::test]
    async fn test_sevenz_list_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.7z");
        create_test_sevenz_archive(&archive_path).await.unwrap();

        let handler = SevenZArchiveHandler::new();
        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(archive_path)),
            inner_path: PathBuf::from("docs"),
        };
        let cancel = CancellationToken::new();
        let entries = handler.list_entries(&loc, &cancel).await.unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"manual.txt"),
            "docs/ should contain manual.txt, got: {:?}",
            names
        );
        assert!(
            names.contains(&"guide.txt"),
            "docs/ should contain guide.txt, got: {:?}",
            names
        );
    }

    // ── extract ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sevenz_extract_all() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.7z");
        create_test_sevenz_archive(&archive_path).await.unwrap();

        let dest_dir = temp_dir.path().join("extracted");
        let handler = SevenZArchiveHandler::new();
        let cancel = CancellationToken::new();
        handler
            .extract_all(
                &Location::Local(archive_path),
                &Location::Local(dest_dir.clone()),
                &cancel,
            )
            .await
            .unwrap();

        assert!(
            dest_dir.join("readme.txt").exists(),
            "readme.txt should be extracted"
        );
        let content = std::fs::read_to_string(dest_dir.join("readme.txt")).unwrap();
        assert_eq!(content, "This is a test archive");
    }

    #[tokio::test]
    async fn test_sevenz_extract_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.7z");
        create_test_sevenz_archive(&archive_path).await.unwrap();

        let dest_file = temp_dir.path().join("out_readme.txt");
        let handler = SevenZArchiveHandler::new();
        let cancel = CancellationToken::new();
        handler
            .extract_file(
                &Location::Archive {
                    archive_path: Box::new(Location::Local(archive_path)),
                    inner_path: PathBuf::from("readme.txt"),
                },
                &Location::Local(dest_file.clone()),
                &cancel,
            )
            .await
            .unwrap();

        assert!(dest_file.exists());
        let content = std::fs::read_to_string(&dest_file).unwrap();
        assert_eq!(content, "This is a test archive");
    }

    // ── MultiFormatArchiveHandler routes correctly ─────────────────────────────

    #[tokio::test]
    async fn test_multi_format_routes_sevenz() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("multi.7z");
        create_test_sevenz_archive(&archive_path).await.unwrap();

        let handler = MultiFormatArchiveHandler::new();
        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(archive_path.clone())),
            inner_path: PathBuf::new(),
        };
        let cancel = CancellationToken::new();
        let entries = handler.list_entries(&loc, &cancel).await.unwrap();
        assert!(
            !entries.is_empty(),
            "MultiFormatArchiveHandler should list 7z entries"
        );
    }

    #[tokio::test]
    async fn test_multi_format_create_sevenz_by_extension() {
        let temp_dir = TempDir::new().unwrap();
        let src_file = temp_dir.path().join("hello.txt");
        std::fs::write(&src_file, b"hello 7z").unwrap();

        let dest_7z = temp_dir.path().join("out.7z");
        let handler = MultiFormatArchiveHandler::new();
        let cancel = CancellationToken::new();
        handler
            .create_archive(
                &[Location::Local(src_file)],
                &Location::Local(dest_7z.clone()),
                &cancel,
            )
            .await
            .unwrap();

        assert!(dest_7z.exists(), "7z file should be created");
        assert!(
            dest_7z.metadata().unwrap().len() > 0,
            "7z file should not be empty"
        );
    }

    #[tokio::test]
    async fn test_multi_format_create_zip_by_extension() {
        let temp_dir = TempDir::new().unwrap();
        let src_file = temp_dir.path().join("hello.txt");
        std::fs::write(&src_file, b"hello zip").unwrap();

        let dest_zip = temp_dir.path().join("out.zip");
        let handler = MultiFormatArchiveHandler::new();
        let cancel = CancellationToken::new();
        handler
            .create_archive(
                &[Location::Local(src_file)],
                &Location::Local(dest_zip.clone()),
                &cancel,
            )
            .await
            .unwrap();

        assert!(dest_zip.exists(), "zip file should be created");
        // Verify it's a valid zip
        let file = std::fs::File::open(&dest_zip).unwrap();
        assert!(
            zip::ZipArchive::new(file).is_ok(),
            "should be a valid zip archive"
        );
    }
}
