//! Snapshots for `DialogContent::FileInfo`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;
use rwf_lib::model::{FileEntry, LinkKind, Location};
use std::path::PathBuf;
use std::time::SystemTime;

#[test]
fn file_info_regular_file() {
    let state = test_state();
    let entry = FileEntry {
        name: "test.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/test.txt")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let dialog = Dialog::file_info(&entry);
    snapshot_dialog("file_info_regular_file", &dialog, &state);
}

#[test]
fn file_info_directory_with_size() {
    let state = test_state();
    let entry = FileEntry {
        name: "my_folder".to_string(),
        location: Location::Local(PathBuf::from("/test/my_folder")),
        size: 0,
        is_dir: true,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: Some(1048576), // 1 MB calculated
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let dialog = Dialog::file_info(&entry);
    snapshot_dialog("file_info_directory_with_size", &dialog, &state);
}

#[test]
fn file_info_symlink() {
    let state = test_state();
    let entry = FileEntry {
        name: "link".to_string(),
        location: Location::Local(PathBuf::from("/test/link")),
        size: 0,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: true,
        link_target: Some(PathBuf::from("/test/target.txt")),
        link_kind: Some(LinkKind::Symlink),
    };
    let dialog = Dialog::file_info(&entry);
    snapshot_dialog("file_info_symlink", &dialog, &state);
}

#[test]
fn file_info_detected_type() {
    let state = test_state();
    let entry = FileEntry {
        name: "photo.png".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/photo.png")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detected_type = Some("PNG image".to_string());
    }
    snapshot_dialog("file_info_detected_type", &dialog, &state);
}

#[test]
fn file_info_detecting() {
    let state = test_state();
    let entry = FileEntry {
        name: "photo.png".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/photo.png")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detecting = true;
    }
    snapshot_dialog("file_info_detecting", &dialog, &state);
}

#[test]
fn file_info_header_bytes_hex_mode() {
    let state = test_state();
    let entry = FileEntry {
        name: "photo.png".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/photo.png")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detected_type = Some("PNG image".to_string());
        d.header_bytes = Some(vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ]);
        d.header_hex_mode = true;
    }
    snapshot_dialog("file_info_header_bytes_hex_mode", &dialog, &state);
}

#[test]
fn file_info_header_bytes_text_mode() {
    let state = test_state();
    let entry = FileEntry {
        name: "notes.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/notes.txt")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detected_type = Some("Unknown / plain text".to_string());
        d.header_bytes = Some(b"Hello, world! This is plain text.".to_vec());
        d.header_hex_mode = false;
    }
    snapshot_dialog("file_info_header_bytes_text_mode", &dialog, &state);
}

/// Task 12: the single most important test in this task. Proves the manually
/// cycled `header_encoding` override genuinely takes effect at render time —
/// i.e. the render function stopped calling `TextEncoding::detect` fresh on
/// every render (Task 11 behavior) and instead decodes with the persisted,
/// possibly-overridden value.
///
/// Real Shift-JIS bytes are used, but `header_encoding` is force-set to
/// `Iso8859_1` — a deliberately "wrong" choice relative to auto-detect. If
/// the render function still called `TextEncoding::detect(bytes)` fresh (the
/// OLD, Task-11 code), it would silently ignore the override and re-detect
/// ShiftJis, producing the correct Japanese text instead of ISO-8859-1
/// mojibake. This test fails against that old code because the correct
/// Japanese text WOULD appear (via detect()) when it must not.
#[test]
fn file_info_header_encoding_override_takes_effect_over_auto_detect() {
    let state = test_state();
    let entry = FileEntry {
        name: "notes_ja.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/notes_ja.txt")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let original = "こんにちは";
    let (encoded, _, had_errors) = encoding_rs::SHIFT_JIS.encode(original);
    assert!(
        !had_errors,
        "Shift-JIS encoding of the fixture must succeed"
    );
    let shift_jis_bytes = encoded.into_owned();
    assert!(
        std::str::from_utf8(&shift_jis_bytes).is_err(),
        "fixture must be genuinely non-UTF-8 to exercise a real encoding mismatch"
    );
    // Sanity check: auto-detect must actually recognize this as ShiftJis, or
    // forcing Iso8859_1 wouldn't be a meaningful override at all.
    assert_eq!(
        rwf_lib::model::viewer::TextEncoding::detect(&shift_jis_bytes),
        rwf_lib::model::viewer::TextEncoding::ShiftJis
    );

    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detected_type = Some("Unknown / plain text".to_string());
        d.header_bytes = Some(shift_jis_bytes.clone());
        d.header_hex_mode = false;
        // Force the override away from what auto-detect would produce.
        d.header_encoding = Some(rwf_lib::model::viewer::TextEncoding::Iso8859_1);
    }

    let rendered = super::render_dialog_to_string(&dialog, &state, 80, 24);

    // The correctly auto-detected Japanese text must NOT appear — if it did,
    // the override was silently discarded and detect() ran fresh instead.
    assert!(
        !rendered.contains(original),
        "override was silently discarded and the text was re-detected as ShiftJis instead: {rendered}"
    );

    // Positive check: the ISO-8859-1 decoding of these exact bytes must be
    // what actually rendered.
    let iso_decoded = rwf_lib::model::viewer::TextEncoding::Iso8859_1.decode(&shift_jis_bytes);
    let iso_sanitized = crate::ui::sanitize_for_display(&iso_decoded);
    assert!(
        rendered.contains(iso_sanitized.trim()),
        "expected the ISO-8859-1-decoded override text to appear in the rendered output: \
         expected {iso_sanitized:?} within {rendered}"
    );
}

/// Task 12: the current encoding's `.name()` must be visible somewhere in the
/// dialog (its own row, per the placement chosen here — see
/// `render_file_info_dialog`'s "Text encoding: ..." row) once
/// `header_encoding` has been set, so the user can see what they're viewing
/// as without guessing. The hint line separately mentions the `e` key itself.
#[test]
fn file_info_shows_current_encoding_name_and_hint() {
    let state = test_state();
    let entry = FileEntry {
        name: "notes.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/notes.txt")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detected_type = Some("Unknown / plain text".to_string());
        d.header_bytes = Some(b"Hello, world!".to_vec());
        d.header_hex_mode = false;
        d.header_encoding = Some(rwf_lib::model::viewer::TextEncoding::ShiftJis);
    }

    let rendered = super::render_dialog_to_string(&dialog, &state, 120, 40);

    assert!(
        rendered.contains("e: encoding"),
        "hint line must mention the encoding-cycle key: {rendered}"
    );
    assert!(
        rendered.contains(&format!(
            "Text encoding: {}",
            rwf_lib::model::viewer::TextEncoding::ShiftJis.name()
        )),
        "dialog must show the current encoding's name ({}): {rendered}",
        rwf_lib::model::viewer::TextEncoding::ShiftJis.name()
    );
}

/// CJK regression coverage for the Task 10 bug fix: the old byte-to-char
/// mapping could never render CJK (a CJK char is multiple UTF-8 bytes).
/// This snapshot proves the actual Japanese characters render, not dots.
#[test]
fn file_info_header_bytes_text_mode_cjk() {
    let state = test_state();
    let entry = FileEntry {
        name: "notes_ja.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/notes_ja.txt")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::UNIX_EPOCH,
        marked: false,
        calculated_size: None,
        is_symlink: false,
        link_target: None,
        link_kind: None,
    };
    let mut dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut dialog.content {
        d.detected_type = Some("Unknown / plain text".to_string());
        d.header_bytes = Some("こんにちは世界".as_bytes().to_vec());
        d.header_hex_mode = false;
    }
    snapshot_dialog("file_info_header_bytes_text_mode_cjk", &dialog, &state);
}
