//! Integration tests for reading archives through 7-Zip's shared library
//! (plan/8.11), via `MultiFormatArchiveHandler`.
//!
//! They adapt to the machine: with 7-Zip installed they check that RAR / LZH open
//! and that zip / 7z listings match the built-in handlers; without it they check
//! the "needs 7-Zip" errors and the built-in fallback. Set `RWF_REQUIRE_7ZIP=1` to
//! fail instead of adapting when 7-Zip is missing (CI does on its runners).

#[cfg(test)]
mod tests {
    use crate::backend::archive::{
        ArchiveHandler, MultiFormatArchiveHandler, SevenZArchiveHandler, ZipArchiveHandler,
    };
    use crate::model::{FileEntry, Location};
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test_data")
            .join(name)
    }

    fn have_7zip() -> bool {
        let found = rwf_7zlib::Library::find(None).is_ok();
        if !found && std::env::var_os("RWF_REQUIRE_7ZIP").is_some_and(|v| v == "1") {
            panic!("RWF_REQUIRE_7ZIP=1 but 7-Zip's library was not found");
        }
        found
    }

    fn at(archive: &Path, inner: &str) -> Location {
        Location::Archive {
            archive_path: Box::new(Location::Local(archive.to_path_buf())),
            inner_path: PathBuf::from(inner),
        }
    }

    fn sorted(mut v: Vec<FileEntry>) -> Vec<(String, bool, u64)> {
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v.into_iter().map(|e| (e.name, e.is_dir, e.size)).collect()
    }

    /// RAR works everywhere except on a Windows machine without 7-Zip.
    fn rar_readable() -> bool {
        cfg!(not(windows)) || have_7zip()
    }

    #[tokio::test]
    async fn rar_lists_and_extracts() {
        let h = MultiFormatArchiveHandler::new();
        let rar = fixture("editd169e.rar");
        let cancel = CancellationToken::new();
        let listed = h.list_entries(&at(&rar, ""), &cancel).await;
        if !rar_readable() {
            let err = listed.unwrap_err().to_string();
            assert!(err.contains("7-Zip"), "{err}");
            return;
        }
        let v = sorted(listed.unwrap());
        assert_eq!(v.len(), 6, "{v:?}");
        assert!(v.contains(&("editdisk.exe".into(), false, 560_640)));

        let dir = TempDir::new().unwrap();
        let one = dir.path().join("single").join("readme.txt");
        h.extract_file(
            &at(&rar, "readme.txt"),
            &Location::Local(one.clone()),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::metadata(&one).unwrap().len(), 792);

        let all = dir.path().join("all");
        h.extract_all(
            &Location::Local(rar.clone()),
            &Location::Local(all.clone()),
            &cancel,
        )
        .await
        .unwrap();
        for (name, size) in [("editdisk.exe", 560_640), ("types.ini", 2481)] {
            assert_eq!(std::fs::metadata(all.join(name)).unwrap().len(), size);
        }
    }

    #[tokio::test]
    async fn lzh_needs_7zip() {
        let h = MultiFormatArchiveHandler::new();
        let lzh = fixture("editd169.lzh");
        let cancel = CancellationToken::new();
        let listed = h.list_entries(&at(&lzh, ""), &cancel).await;
        if !have_7zip() {
            let err = listed.unwrap_err().to_string();
            assert!(err.contains("7-Zip"), "{err}");
            return;
        }
        let v = listed.unwrap();
        assert_eq!(v.len(), 6);
        let dir = TempDir::new().unwrap();
        h.extract_all(
            &Location::Local(lzh.clone()),
            &Location::Local(dir.path().to_path_buf()),
            &cancel,
        )
        .await
        .unwrap();
        for e in v.iter().filter(|e| !e.is_dir) {
            let len = std::fs::metadata(dir.path().join(&e.name)).unwrap().len();
            assert_eq!(len, e.size, "{}", e.name);
        }
    }

    /// A zip and a 7z made by the built-in writers, with a nested directory and a
    /// non-ASCII name.
    async fn make_archives(dir: &Path) -> (PathBuf, PathBuf) {
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("docs").join("deep")).unwrap();
        std::fs::write(src.join("readme.txt"), b"hello").unwrap();
        std::fs::write(src.join("docs").join("日本語.txt"), "あいう".as_bytes()).unwrap();
        std::fs::write(src.join("docs").join("deep").join("x.bin"), [7u8; 300]).unwrap();
        let sources = vec![
            Location::Local(src.join("readme.txt")),
            Location::Local(src.join("docs")),
        ];
        let cancel = CancellationToken::new();
        let zip = dir.join("t.zip");
        ZipArchiveHandler::new()
            .create_archive(&sources, &Location::Local(zip.clone()), &cancel)
            .await
            .unwrap();
        let sz = dir.join("t.7z");
        SevenZArchiveHandler::new()
            .create_archive(&sources, &Location::Local(sz.clone()), &cancel)
            .await
            .unwrap();
        (zip, sz)
    }

    #[tokio::test]
    async fn listings_match_the_builtin_handlers() {
        let dir = TempDir::new().unwrap();
        let (zip, sz) = make_archives(dir.path()).await;
        let multi = MultiFormatArchiveHandler::new();
        let cancel = CancellationToken::new();
        for inner in ["", "docs", "docs/deep"] {
            let via_multi = sorted(multi.list_entries(&at(&zip, inner), &cancel).await.unwrap());
            let builtin = sorted(
                ZipArchiveHandler::new()
                    .list_entries(&at(&zip, inner), &cancel)
                    .await
                    .unwrap(),
            );
            assert_eq!(via_multi, builtin, "zip {inner:?}");
            let via_multi = sorted(multi.list_entries(&at(&sz, inner), &cancel).await.unwrap());
            let builtin = sorted(
                SevenZArchiveHandler::new()
                    .list_entries(&at(&sz, inner), &cancel)
                    .await
                    .unwrap(),
            );
            assert_eq!(via_multi, builtin, "7z {inner:?}");
        }
    }

    #[tokio::test]
    async fn subdirectory_extraction_keeps_relative_layout() {
        let dir = TempDir::new().unwrap();
        let (zip, sz) = make_archives(dir.path()).await;
        let multi = MultiFormatArchiveHandler::new();
        let cancel = CancellationToken::new();
        for (tag, archive) in [("zip", &zip), ("7z", &sz)] {
            let out = dir.path().join(format!("out-{tag}"));
            multi
                .extract_all(&at(archive, "docs"), &Location::Local(out.clone()), &cancel)
                .await
                .unwrap();
            assert_eq!(
                std::fs::read_to_string(out.join("日本語.txt")).unwrap(),
                "あいう"
            );
            assert_eq!(
                std::fs::read(out.join("deep").join("x.bin")).unwrap(),
                [7u8; 300]
            );
            assert!(
                !out.join("readme.txt").exists(),
                "{tag}: outside the subtree"
            );
        }
    }

    #[tokio::test]
    async fn a_bad_configured_library_falls_back_to_builtin() {
        let dir = TempDir::new().unwrap();
        let (zip, _) = make_archives(dir.path()).await;
        let multi =
            MultiFormatArchiveHandler::with_sevenzip_library(Some(dir.path().join("no-7z.dll")));
        let cancel = CancellationToken::new();
        let v = sorted(multi.list_entries(&at(&zip, ""), &cancel).await.unwrap());
        assert_eq!(
            v,
            [("docs".into(), true, 0), ("readme.txt".into(), false, 5)]
        );
        // The same handler picks up a corrected path after a reload.
        multi.set_sevenzip_library(None);
        assert_eq!(
            sorted(multi.list_entries(&at(&zip, ""), &cancel).await.unwrap()),
            v
        );
    }

    #[tokio::test]
    async fn a_cancelled_extraction_reports_cancelled() {
        if !have_7zip() {
            return;
        }
        let multi = MultiFormatArchiveHandler::new();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let dir = TempDir::new().unwrap();
        let err = multi
            .extract_all(
                &Location::Local(fixture("editd169.lzh")),
                &Location::Local(dir.path().to_path_buf()),
                &cancel,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("cancelled"), "{err}");
    }

    /// ZIP's MS-DOS time is local wall-clock time: an entry stamped 23:48:20 must
    /// show as 23:48:20 in the local zone, from the built-in reader and from 7-Zip.
    /// (Only a non-UTC zone catches the old "read it as UTC" bug.)
    #[tokio::test]
    async fn zip_dos_time_is_local_wall_clock() {
        use std::io::Write;
        let dir = TempDir::new().unwrap();
        let zip_path = dir.path().join("t.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let stamp = zip::DateTime::from_date_and_time(2010, 1, 8, 23, 48, 20).unwrap();
            let opts = zip::write::SimpleFileOptions::default().last_modified_time(stamp);
            w.start_file("a.txt", opts).unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        let cancel = CancellationToken::new();
        let shown = |e: &FileEntry| {
            let local: chrono::DateTime<chrono::Local> = e.modified.into();
            local.format("%Y-%m-%d %H:%M:%S").to_string()
        };
        let builtin = ZipArchiveHandler::new()
            .list_entries(&at(&zip_path, ""), &cancel)
            .await
            .unwrap();
        assert_eq!(shown(&builtin[0]), "2010-01-08 23:48:20", "built-in zip");
        let multi = MultiFormatArchiveHandler::new()
            .list_entries(&at(&zip_path, ""), &cancel)
            .await
            .unwrap();
        assert_eq!(shown(&multi[0]), "2010-01-08 23:48:20", "via MultiFormat");
    }

    #[test]
    fn suffix_table_is_the_single_source() {
        let h = MultiFormatArchiveHandler::new();
        for name in [
            "a.rar", "a.LZH", "a.lha", "a.arj", "a.cab", "a.cpio", "a.wim", "a.tar.gz",
        ] {
            assert!(h.is_archive(name), "{name}");
            assert!(crate::backend::is_archive_name(name), "{name}");
        }
        for name in ["a.gz", "a.xz", "a.chm", "a.exe", "a.docx"] {
            assert!(!h.is_archive(name), "{name}");
        }
    }

    #[tokio::test]
    async fn sevenzip_only_formats_cannot_be_created() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("f.txt");
        std::fs::write(&src, b"x").unwrap();
        let err = MultiFormatArchiveHandler::new()
            .create_archive(
                &[Location::Local(src)],
                &Location::Local(dir.path().join("out.cab")),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("can only create"), "{err}");
    }
}
