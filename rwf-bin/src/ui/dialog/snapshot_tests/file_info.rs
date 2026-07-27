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
    // Not a pinned snapshot (Phase 7.3b, Task 13b): since the "Detecting..."
    // row now renders a real wall-clock spinner (`spinner::current_frame`),
    // asserting an exact rendered frame would be flaky — the frame depends on
    // `SystemTime::now()` at render time. Instead, a single-element
    // `spinner_frames` list is used, which `current_frame`/`frame_index`
    // deterministically always resolves to index 0 regardless of the clock
    // (see `spinner.rs`'s own `test_frame_index_single_frame_always_zero`) —
    // this proves the row renders the REAL spinner widget's output (not the
    // old literal static string) while staying deterministic.
    let mut state = test_state();
    state.config.display.spinner_frames = vec!["*".to_string()];
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

/// Companion to `file_info_detecting` above: proves the spinner row is
/// genuinely driven by `spinner::current_frame` (not a hardcoded literal) by
/// swapping in a distinctive single-frame spinner and checking it shows up
/// verbatim, alongside the "Detecting..." label prefix. A single-frame list
/// is deterministic (see the comment on `file_info_detecting`), so this is
/// not flaky despite depending on wall-clock time internally.
#[test]
fn file_info_detecting_shows_dynamic_spinner_not_static_string() {
    let mut state = test_state();
    state.config.display.spinner_frames = vec!["<>".to_string()];
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

    let rendered = super::render_dialog_to_string(&dialog, &state, 80, 24);

    assert!(
        rendered.contains("Detecting... <>"),
        "expected the real spinner frame to appear after the label: {rendered}"
    );
    assert!(
        !rendered.contains("Detecting...                "),
        "must not render the old static string with no spinner content: {rendered}"
    );
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

/// Task 13a: the complaint was that pressing `e` in File Info's hex mode
/// visibly did nothing, because the ASCII column came from `format_hex_row`'s
/// pure byte-value mapping, independent of `header_encoding`. This proves the
/// fix at the full render level: the SAME single byte (0x92) renders
/// differently in the hex-mode ASCII column depending on `header_encoding` —
/// '\u{2019}' under Windows-1252 (a genuine printable char in that encoding)
/// vs '.' under Shift-JIS (0x92 is a lead byte with no trailing byte to pair
/// with in a 1-byte row, so it falls back to the dot placeholder). See
/// `decode_row_chars_differs_between_windows1252_and_shift_jis_for_0x92` in
/// `dialog::file_info`'s own test module for the byte-value reasoning.
#[test]
fn file_info_hex_mode_ascii_column_reflects_encoding() {
    let state = test_state();
    let entry = FileEntry {
        name: "notes.bin".to_string(),
        location: Location::Local(PathBuf::from("/test/documents/notes.bin")),
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

    // A wide terminal is used deliberately: `format_hex_row`'s ASCII column
    // is padded out to a fixed 16-byte-aligned offset regardless of how many
    // bytes are actually in the row (so short rows line up with full ones),
    // which pushes even a 1-byte row's ASCII column out past column ~58.
    // The dialog's default width (60% of a narrow 80-col screen) truncates
    // that far before the ASCII column at all — a pre-existing width
    // limitation independent of this test — so a wide screen is used here to
    // actually see the column, rather than asserting on truncated output.
    let mut win1252_dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut win1252_dialog.content {
        d.detected_type = Some("Unknown / plain text".to_string());
        d.header_bytes = Some(vec![0x92]);
        d.header_hex_mode = true;
        d.header_encoding = Some(rwf_lib::model::viewer::TextEncoding::Windows1252);
    }
    let win1252_rendered = super::render_dialog_to_string(&win1252_dialog, &state, 200, 30);

    let mut sjis_dialog = Dialog::file_info(&entry);
    if let rwf_lib::model::dialog::DialogContent::FileInfo(d) = &mut sjis_dialog.content {
        d.detected_type = Some("Unknown / plain text".to_string());
        d.header_bytes = Some(vec![0x92]);
        d.header_hex_mode = true;
        d.header_encoding = Some(rwf_lib::model::viewer::TextEncoding::ShiftJis);
    }
    let sjis_rendered = super::render_dialog_to_string(&sjis_dialog, &state, 200, 30);

    assert_ne!(
        win1252_rendered, sjis_rendered,
        "cycling header_encoding must visibly change hex mode's ASCII column"
    );
    assert!(
        win1252_rendered.contains('\u{2019}'),
        "Windows-1252 decode of 0x92 must show the actual character: {win1252_rendered}"
    );
}

/// Phase 7.3b, Task 15: reproduces the user's live-testing screenshot bug —
/// a full 16-byte hex row's ASCII column got cut off at the dialog's right
/// border on a wide terminal, even though there was plenty of unused screen
/// width (the background pane's own content was visibly peeking out past the
/// dialog). Root cause: the dialog-width match had no dedicated
/// `DialogContent::FileInfo` arm, so it fell through to the generic 60%
/// catch-all (`default_dialog_width`), which doesn't know about the hex
/// row's fixed 80-column content requirement.
///
/// At 100 columns: the OLD 60% formula gives 60 (clips well before the
/// ASCII column even starts); the NEW content-driven formula
/// (`file_info_dialog_width`, capped at 90%/floored at 40) gives exactly 80,
/// which is precisely enough to show a full hex row with no wasted stretch.
#[test]
fn file_info_hex_mode_full_row_not_clipped_at_100_columns() {
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
        // A full 16-byte row so the ASCII column runs the full width;
        // bytes chosen so every position decodes to a printable ASCII char
        // under the Utf8 fallback (0x20..=0x2B), matching the reported
        // screenshot's row shape (offset + hex + trailing ASCII run).
        d.header_bytes = Some(vec![
            0x00, 0x05, 0x81, 0x44, 0x20, 0x20, 0x21, 0x22, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29,
            0x2A, 0x2B,
        ]);
        d.header_hex_mode = true;
    }

    let rendered = super::render_dialog_to_string(&dialog, &state, 100, 24);

    // The ASCII column's trailing run for these bytes: 0x24 '$' through 0x2B
    // '+' are all printable ASCII, so they decode verbatim under the Utf8
    // fallback. If the dialog is too narrow, this trailing slice of the
    // ASCII column gets cut off (as in the user's screenshot) instead of
    // appearing in full.
    assert!(
        rendered.contains("$%&'()*+"),
        "expected the full trailing ASCII column to be visible, not clipped: {rendered}"
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
