//! Tests that all supported archive formats are correctly recognized.

#[cfg(test)]
mod tests {
    use crate::backend::archive::{
        ArchiveHandler, MultiFormatArchiveHandler,
        RarArchiveHandler, IsoArchiveHandler, LzhArchiveHandler,
    };
    use crate::model::Location;
    use std::path::PathBuf;
    use tokio_util::sync::CancellationToken;

    // ── Per-handler is_archive ─────────────────────────────────────────────────

    #[test]
    fn test_rar_handler_is_archive() {
        let h = RarArchiveHandler::new();
        assert!(h.is_archive("archive.rar"));
        assert!(h.is_archive("ARCHIVE.RAR"));
        assert!(!h.is_archive("archive.zip"));
        assert!(!h.is_archive("rar"));
    }

    #[test]
    fn test_iso_handler_is_archive() {
        let h = IsoArchiveHandler::new();
        assert!(h.is_archive("disc.iso"));
        assert!(h.is_archive("DISC.ISO"));
        assert!(!h.is_archive("disc.img"));
        assert!(!h.is_archive("iso"));
    }

    #[test]
    fn test_lzh_handler_is_archive() {
        let h = LzhArchiveHandler::new();
        assert!(h.is_archive("file.lzh"));
        assert!(h.is_archive("file.lha"));
        assert!(h.is_archive("FILE.LZH"));
        assert!(!h.is_archive("file.zip"));
        assert!(!h.is_archive("lzh"));
    }

    // ── MultiFormatArchiveHandler covers all formats ──────────────────────────

    #[test]
    fn test_multi_format_recognizes_all() {
        let h = MultiFormatArchiveHandler::new();
        // ZIP/7Z/TAR already covered in other test files; verify full set here
        assert!(h.is_archive("file.zip"));
        assert!(h.is_archive("file.7z"));
        assert!(h.is_archive("file.tar"));
        assert!(h.is_archive("file.tgz"));
        assert!(h.is_archive("file.tar.gz"));
        assert!(h.is_archive("file.rar"));
        assert!(h.is_archive("file.iso"));
        assert!(h.is_archive("file.lzh"));
        assert!(h.is_archive("file.lha"));
        // Non-archives
        assert!(!h.is_archive("file.txt"));
        assert!(!h.is_archive("file.exe"));
        assert!(!h.is_archive("file.pdf"));
    }

    // ── Stub handlers return clear error messages ─────────────────────────────

    #[tokio::test]
    async fn test_rar_list_returns_error() {
        let h = RarArchiveHandler::new();
        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(PathBuf::from("/test.rar"))),
            inner_path: PathBuf::new(),
        };
        let cancel = CancellationToken::new();
        let err = h.list_entries(&loc, &cancel).await.unwrap_err();
        let msg = err.to_string().to_lowercase();
        assert!(msg.contains("rar") || msg.contains("unrar"), "error should mention RAR/unrar: {}", err);
    }

    #[tokio::test]
    async fn test_rar_create_returns_proprietary_error() {
        let h = RarArchiveHandler::new();
        let cancel = CancellationToken::new();
        let err = h.create_archive(
            &[Location::Local(PathBuf::from("/file.txt"))],
            &Location::Local(PathBuf::from("/out.rar")),
            &cancel,
        ).await.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("proprietary") || err.to_string().to_lowercase().contains("cannot"), "got: {}", err);
    }

    #[tokio::test]
    async fn test_iso_list_returns_error() {
        let h = IsoArchiveHandler::new();
        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(PathBuf::from("/disc.iso"))),
            inner_path: PathBuf::new(),
        };
        let cancel = CancellationToken::new();
        let err = h.list_entries(&loc, &cancel).await.unwrap_err();
        assert!(!err.to_string().is_empty(), "error should not be empty: {}", err);
    }

    #[tokio::test]
    async fn test_lzh_list_returns_error() {
        let h = LzhArchiveHandler::new();
        let loc = Location::Archive {
            archive_path: Box::new(Location::Local(PathBuf::from("/archive.lzh"))),
            inner_path: PathBuf::new(),
        };
        let cancel = CancellationToken::new();
        let err = h.list_entries(&loc, &cancel).await.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("lzh") || err.to_string().to_lowercase().contains("lha"), "got: {}", err);
    }

    // ── input::is_archive recognizes all formats ──────────────────────────────

    #[test]
    fn test_input_is_archive_rar() {
        // Verify via MultiFormatArchiveHandler which mirrors is_archive()
        let h = MultiFormatArchiveHandler::new();
        assert!(h.is_archive("backup.rar"));
        assert!(h.is_archive("backup.RAR"));
    }

    #[test]
    fn test_input_is_archive_iso() {
        let h = MultiFormatArchiveHandler::new();
        assert!(h.is_archive("ubuntu.iso"));
    }

    #[test]
    fn test_input_is_archive_lzh() {
        let h = MultiFormatArchiveHandler::new();
        assert!(h.is_archive("game.lzh"));
        assert!(h.is_archive("game.lha"));
    }
}
