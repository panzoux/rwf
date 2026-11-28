//! Runs against the 7-Zip installed on this machine. Skipped (passes with a note)
//! when 7-Zip is not found, unless `RWF_REQUIRE_7ZIP=1` is set (CI sets it on
//! runners that install 7-Zip).

use rwf_7zlib::{file_sink, Library, OpResult};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rwf-lib/test_data")
        .join(name)
}

fn library() -> Option<Library> {
    match Library::find(None) {
        Ok(l) => Some(l),
        Err(e) => {
            if std::env::var_os("RWF_REQUIRE_7ZIP").is_some_and(|v| v == "1") {
                panic!("7-Zip is required here but was not loaded: {e}");
            }
            eprintln!("skipping: {e}");
            None
        }
    }
}

const RAR_FILES: [(&str, u64); 6] = [
    ("types.ini", 2481),
    ("editdisk.cnt", 888),
    ("editdisk.exe", 560_640),
    ("editdisk.hlp", 55_393),
    ("editdisk.ini", 280),
    ("readme.txt", 792),
];

#[test]
fn loads_and_lists_formats() {
    let Some(lib) = library() else { return };
    assert!(lib.version().0 >= 23, "{:?}", lib.version());
    let names: Vec<&str> = lib.formats().iter().map(|f| f.name.as_str()).collect();
    for f in ["zip", "7z", "Rar", "Lzh", "tar"] {
        assert!(names.contains(&f), "{f} missing from {names:?}");
    }
    assert!(lib.formats_for("x.RAR").iter().any(|f| f.name == "Rar"));
}

#[test]
fn lists_rar_items() {
    let Some(lib) = library() else { return };
    let a = lib.open(&fixture("editd169e.rar")).expect("open rar");
    let got: Vec<(&str, u64)> = a
        .items()
        .iter()
        .map(|i| (i.path.as_str(), i.size))
        .collect();
    assert_eq!(got, RAR_FILES);
    assert!(a
        .items()
        .iter()
        .all(|i| !i.is_dir && !i.encrypted && i.modified.is_some()));
}

#[test]
fn extracts_rar_or_reports_unsupported_method() {
    let Some(lib) = library() else { return };
    let a = lib.open(&fixture("editd169e.rar")).expect("open rar");
    let dir = tempfile::tempdir().unwrap();
    let items = a.items().to_vec();
    let mut target = |i: u32| {
        let p = dir.path().join(&items[i as usize].path);
        Ok(Some(file_sink(std::fs::File::create(p)?)))
    };
    let results = a.extract(&[], &mut target, &|| false).expect("extract");
    assert_eq!(results.len(), 6);
    if results.iter().all(|(_, r)| *r == OpResult::Ok) {
        for (name, size) in RAR_FILES {
            assert_eq!(
                std::fs::metadata(dir.path().join(name)).unwrap().len(),
                size
            );
        }
    } else {
        // Homebrew's and Debian's 7z.so carry no RAR decoder.
        // Windows' 7z.dll has one, so there this branch must not be reached.
        if cfg!(windows) {
            panic!("Windows' 7z.dll should extract RAR: {results:?}");
        }
        assert!(
            results
                .iter()
                .all(|(_, r)| *r == OpResult::UnsupportedMethod),
            "{results:?}"
        );
    }
}

#[test]
fn extracts_selected_lzh_items() {
    let Some(lib) = library() else { return };
    let a = lib.open(&fixture("editd169.lzh")).expect("open lzh");
    assert_eq!(a.format(), "Lzh");
    let wanted: Vec<u32> = a
        .items()
        .iter()
        .filter(|i| i.path.ends_with(".txt"))
        .map(|i| i.index)
        .collect();
    assert!(!wanted.is_empty());
    let dir = tempfile::tempdir().unwrap();
    let items = a.items().to_vec();
    let mut target = |i: u32| {
        let p = dir.path().join(items[i as usize].path.replace('/', "_"));
        Ok(Some(file_sink(std::fs::File::create(p)?)))
    };
    let results = a.extract(&wanted, &mut target, &|| false).expect("extract");
    assert_eq!(results.iter().map(|(i, _)| *i).collect::<Vec<_>>(), wanted);
    assert!(
        results.iter().all(|(_, r)| *r == OpResult::Ok),
        "{results:?}"
    );
    for i in &wanted {
        let it = &items[*i as usize];
        let len = std::fs::metadata(dir.path().join(it.path.replace('/', "_")))
            .unwrap()
            .len();
        assert_eq!(len, it.size);
    }
}

#[test]
fn cancel_stops_extraction() {
    let Some(lib) = library() else { return };
    let a = lib.open(&fixture("editd169.lzh")).expect("open lzh");
    let mut target = |_: u32| {
        Ok(Some(
            Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>
        ))
    };
    let err = a.extract(&[], &mut target, &|| true).unwrap_err();
    assert!(matches!(err, rwf_7zlib::Error::Cancelled), "{err}");
}

#[test]
fn write_errors_surface_as_io_errors() {
    let Some(lib) = library() else { return };
    let a = lib.open(&fixture("editd169.lzh")).expect("open lzh");
    let mut target = |_: u32| Err(std::io::Error::other("disk full"));
    let err = a.extract(&[], &mut target, &|| false).unwrap_err();
    assert!(matches!(err, rwf_7zlib::Error::Io(_)), "{err}");
}

#[test]
fn non_archives_do_not_open() {
    let Some(lib) = library() else { return };
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake.rar");
    std::fs::write(&fake, b"not a rar archive at all").unwrap();
    assert!(matches!(
        lib.open(&fake),
        Err(rwf_7zlib::Error::CannotOpen(_))
    ));
    let unknown = dir.path().join("plain.txt");
    std::fs::write(&unknown, b"x").unwrap();
    assert!(matches!(
        lib.open(&unknown),
        Err(rwf_7zlib::Error::CannotOpen(_))
    ));
}

#[test]
fn missing_configured_library_is_a_load_error() {
    let err = Library::find(Some(Path::new("/no/such/7z.so"))).unwrap_err();
    assert!(matches!(err, rwf_7zlib::Error::Load { .. }), "{err}");
}
