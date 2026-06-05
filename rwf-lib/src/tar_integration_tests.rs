//! Integration tests for TAR/TGZ archive operations

#[cfg(test)]
mod tests {
    use crate::backend::archive::{ArchiveHandler, TarArchiveHandler, MultiFormatArchiveHandler};
    use crate::model::Location;
    use std::path::PathBuf;
    use tokio_util::sync::CancellationToken;
    use tempfile::TempDir;

    /// Build a simple directory tree in `base` and return source Locations.
    fn setup_sources(base: &std::path::Path) -> Vec<Location> {
        std::fs::write(base.join("readme.txt"), b"hello tar").unwrap();
        let docs = base.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("manual.txt"), b"user manual").unwrap();
        std::fs::write(docs.join("guide.txt"), b"quick guide").unwrap();
        vec![
            Location::Local(base.join("readme.txt")),
            Location::Local(docs),
        ]
    }

    // ── is_archive ────────────────────────────────────────────────────────────

    #[test]
    fn test_tar_handler_is_archive() {
        let h = TarArchiveHandler::new();
        assert!(h.is_archive("file.tar"));
        assert!(h.is_archive("FILE.TAR"));
        assert!(h.is_archive("file.tgz"));
        assert!(h.is_archive("file.tar.gz"));
        assert!(!h.is_archive("file.zip"));
        assert!(!h.is_archive("file.7z"));
        assert!(!h.is_archive("tar"));
    }

    #[test]
    fn test_multi_format_includes_tar() {
        let h = MultiFormatArchiveHandler::new();
        assert!(h.is_archive("file.tar"));
        assert!(h.is_archive("file.tgz"));
        assert!(h.is_archive("file.tar.gz"));
        assert!(h.is_archive("file.zip"));
        assert!(h.is_archive("file.7z"));
    }

    // ── TAR create + list ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_tar_create_and_list_root() {
        let temp = TempDir::new().unwrap();
        let sources = setup_sources(temp.path());
        let archive = temp.path().join("test.tar");
        let dest = Location::Local(archive.clone());
        let cancel = CancellationToken::new();

        TarArchiveHandler::new().create_archive(&sources, &dest, &cancel).await.unwrap();
        assert!(archive.exists());

        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(archive)),
            inner_path: PathBuf::new(),
        };
        let entries = TarArchiveHandler::new().list_entries(&loc, &cancel).await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"readme.txt"), "got: {:?}", names);
        assert!(names.contains(&"docs"), "got: {:?}", names);
        let docs_entry = entries.iter().find(|e| e.name == "docs").unwrap();
        assert!(docs_entry.is_dir);
    }

    #[tokio::test]
    async fn test_tar_list_subdirectory() {
        let temp = TempDir::new().unwrap();
        let sources = setup_sources(temp.path());
        let archive = temp.path().join("sub.tar");
        let cancel = CancellationToken::new();
        TarArchiveHandler::new()
            .create_archive(&sources, &Location::Local(archive.clone()), &cancel)
            .await.unwrap();

        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(archive)),
            inner_path: PathBuf::from("docs"),
        };
        let entries = TarArchiveHandler::new().list_entries(&loc, &cancel).await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"manual.txt"), "got: {:?}", names);
        assert!(names.contains(&"guide.txt"), "got: {:?}", names);
    }

    // ── TAR extract ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_tar_extract_all() {
        let temp = TempDir::new().unwrap();
        let sources = setup_sources(temp.path());
        let archive = temp.path().join("all.tar");
        let cancel = CancellationToken::new();
        TarArchiveHandler::new()
            .create_archive(&sources, &Location::Local(archive.clone()), &cancel)
            .await.unwrap();

        let dest = temp.path().join("out");
        TarArchiveHandler::new()
            .extract_all(&Location::Local(archive), &Location::Local(dest.clone()), &cancel)
            .await.unwrap();

        assert!(dest.join("readme.txt").exists());
        let content = std::fs::read_to_string(dest.join("readme.txt")).unwrap();
        assert_eq!(content, "hello tar");
        assert!(dest.join("docs").join("manual.txt").exists());
    }

    #[tokio::test]
    async fn test_tar_extract_single_file() {
        let temp = TempDir::new().unwrap();
        let sources = setup_sources(temp.path());
        let archive = temp.path().join("single.tar");
        let cancel = CancellationToken::new();
        TarArchiveHandler::new()
            .create_archive(&sources, &Location::Local(archive.clone()), &cancel)
            .await.unwrap();

        let out_file = temp.path().join("extracted_readme.txt");
        TarArchiveHandler::new().extract_file(
            &Location::Archive {
                archive_path: Box::new(Location::Local(archive)),
                inner_path: PathBuf::from("readme.txt"),
            },
            &Location::Local(out_file.clone()),
            &cancel,
        ).await.unwrap();

        assert!(out_file.exists());
        assert_eq!(std::fs::read_to_string(&out_file).unwrap(), "hello tar");
    }

    // ── TGZ create + extract ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_tgz_create_and_extract_all() {
        let temp = TempDir::new().unwrap();
        let sources = setup_sources(temp.path());
        let archive = temp.path().join("test.tgz");
        let cancel = CancellationToken::new();

        TarArchiveHandler::new()
            .create_archive(&sources, &Location::Local(archive.clone()), &cancel)
            .await.unwrap();
        assert!(archive.exists());
        assert!(archive.metadata().unwrap().len() > 0);

        let dest = temp.path().join("tgz_out");
        TarArchiveHandler::new()
            .extract_all(&Location::Local(archive), &Location::Local(dest.clone()), &cancel)
            .await.unwrap();

        assert!(dest.join("readme.txt").exists());
        assert_eq!(std::fs::read_to_string(dest.join("readme.txt")).unwrap(), "hello tar");
    }

    #[tokio::test]
    async fn test_tgz_list_root() {
        let temp = TempDir::new().unwrap();
        let sources = setup_sources(temp.path());
        let archive = temp.path().join("list.tgz");
        let cancel = CancellationToken::new();
        TarArchiveHandler::new()
            .create_archive(&sources, &Location::Local(archive.clone()), &cancel)
            .await.unwrap();

        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(archive)),
            inner_path: PathBuf::new(),
        };
        let entries = TarArchiveHandler::new().list_entries(&loc, &cancel).await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"readme.txt"), "tgz root: {:?}", names);
        assert!(names.contains(&"docs"), "tgz root: {:?}", names);
    }

    // ── MultiFormat routes TAR correctly ──────────────────────────────────────

    #[tokio::test]
    async fn test_multi_format_routes_tar() {
        let temp = TempDir::new().unwrap();
        let sources = setup_sources(temp.path());
        let archive = temp.path().join("multi.tar");
        let cancel = CancellationToken::new();

        MultiFormatArchiveHandler::new()
            .create_archive(&sources, &Location::Local(archive.clone()), &cancel)
            .await.unwrap();

        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(archive)),
            inner_path: PathBuf::new(),
        };
        let entries = MultiFormatArchiveHandler::new()
            .list_entries(&loc, &cancel).await.unwrap();
        assert!(!entries.is_empty(), "MultiFormat should list TAR entries");
    }

    #[tokio::test]
    async fn test_multi_format_routes_tgz() {
        let temp = TempDir::new().unwrap();
        let sources = setup_sources(temp.path());
        let archive = temp.path().join("multi.tgz");
        let cancel = CancellationToken::new();

        MultiFormatArchiveHandler::new()
            .create_archive(&sources, &Location::Local(archive.clone()), &cancel)
            .await.unwrap();

        let dest = temp.path().join("tgz_multi_out");
        MultiFormatArchiveHandler::new()
            .extract_all(&Location::Local(archive), &Location::Local(dest.clone()), &cancel)
            .await.unwrap();

        assert!(dest.join("readme.txt").exists());
    }
}
