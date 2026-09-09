//! Integration tests for file viewer functionality
//!
//! Tests the complete viewer workflow including:
//! - Opening text and hex viewers
//! - Loading file contents as jobs
//! - Encoding switching
//! - Navigation (Home/End, F5/F6, scrolling)
//! - Search functionality (F4, F3, Shift+F3)

use crate::job::JobKind;
use crate::model::viewer::{FileBytes, LineIndex, SeekableFile, ViewerBuffer};
use crate::model::{Location, TextEncoding, UIMode, ViewerMode};
use crate::state::{update_state, Transition};
use crate::test_utils::test_state;
use std::path::PathBuf;

#[test]
fn test_open_text_viewer() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.txt"));

    // Open text viewer
    let result = update_state(
        &mut state,
        Transition::OpenTextViewer {
            location: location.clone(),
        },
    );

    // Should create a job to load the file
    assert_eq!(result.jobs_to_start.len(), 1);
    assert!(matches!(
        result.jobs_to_start[0].kind,
        JobKind::LoadFileForViewer { .. }
    ));

    // Should change UI mode to viewer
    assert_eq!(state.ui.mode, UIMode::Viewer);

    // Should create viewer state
    assert!(state.viewer.is_some());
    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.location, location);
    assert_eq!(viewer.mode, ViewerMode::Text);
    assert_eq!(viewer.encoding, TextEncoding::Utf8);
}

#[test]
fn test_open_hex_viewer() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.bin"));

    // Open hex viewer
    let result = update_state(
        &mut state,
        Transition::OpenHexViewer {
            location: location.clone(),
        },
    );

    // Should create a job to load the file
    assert_eq!(result.jobs_to_start.len(), 1);

    // Should change UI mode to viewer
    assert_eq!(state.ui.mode, UIMode::Viewer);

    // Should create viewer state in hex mode
    assert!(state.viewer.is_some());
    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.mode, ViewerMode::Hex);
}

#[test]
fn test_viewer_load_complete() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.txt"));

    // Open text viewer
    update_state(
        &mut state,
        Transition::OpenTextViewer {
            location: location.clone(),
        },
    );

    // Simulate file load completion
    let contents = b"Hello World\nLine 2\nLine 3".to_vec();
    update_state(
        &mut state,
        Transition::ViewerLoadComplete {
            contents: contents.clone(),
        },
    );

    // Verify contents are loaded (InMemory path via set_contents)
    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.line_count(), 3);
    assert_eq!(viewer.text(), Some("Hello World\nLine 2\nLine 3"));
}

#[test]
fn test_viewer_encoding_cycle() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.txt"));

    // Open text viewer and load contents
    update_state(&mut state, Transition::OpenTextViewer { location });
    update_state(
        &mut state,
        Transition::ViewerLoadComplete {
            contents: b"Test".to_vec(),
        },
    );

    // Initial encoding should be UTF-8
    assert_eq!(state.viewer.as_ref().unwrap().encoding, TextEncoding::Utf8);

    // Cycle encoding
    update_state(&mut state, Transition::ViewerCycleEncoding);
    assert_eq!(
        state.viewer.as_ref().unwrap().encoding,
        TextEncoding::Utf16Le
    );

    update_state(&mut state, Transition::ViewerCycleEncoding);
    assert_eq!(
        state.viewer.as_ref().unwrap().encoding,
        TextEncoding::Utf16Be
    );

    update_state(&mut state, Transition::ViewerCycleEncoding);
    assert_eq!(
        state.viewer.as_ref().unwrap().encoding,
        TextEncoding::ShiftJis
    );
}

#[test]
fn test_viewer_navigation_home_end() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.txt"));

    // Open viewer and load contents
    update_state(&mut state, Transition::OpenTextViewer { location });
    update_state(
        &mut state,
        Transition::ViewerLoadComplete {
            contents: b"Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_vec(),
        },
    );

    // Set line offset to middle
    state.viewer.as_mut().unwrap().line_offset = 2;
    state.viewer.as_mut().unwrap().column_offset = 5;

    // Test Home (move to line start)
    update_state(&mut state, Transition::ViewerMoveToLineStart);
    assert_eq!(state.viewer.as_ref().unwrap().column_offset, 0);

    // Test End (move to line end)
    update_state(
        &mut state,
        Transition::ViewerMoveToLineEnd { viewport_width: 80 },
    );
    // Column offset should still be 0 since line is short
    assert_eq!(state.viewer.as_ref().unwrap().column_offset, 0);
}

#[test]
fn test_viewer_navigation_top_bottom() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.txt"));

    // Open viewer and load contents
    update_state(&mut state, Transition::OpenTextViewer { location });
    update_state(
        &mut state,
        Transition::ViewerLoadComplete {
            contents: b"Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_vec(),
        },
    );

    // Set line offset to middle
    state.viewer.as_mut().unwrap().line_offset = 2;

    // Test F5 (jump to top)
    update_state(&mut state, Transition::ViewerJumpToTop);
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 0);

    // Test F6 (jump to bottom)
    update_state(
        &mut state,
        Transition::ViewerJumpToBottom {
            viewport_height: 20,
        },
    );
    // With 5 lines and viewport height of 20, line_offset should be 0
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 0);
}

#[test]
fn test_viewer_scrolling() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.txt"));

    // Open viewer and load contents with many lines
    update_state(&mut state, Transition::OpenTextViewer { location });
    let mut contents = String::new();
    for i in 0..100 {
        contents.push_str(&format!("Line {}\n", i));
    }
    update_state(
        &mut state,
        Transition::ViewerLoadComplete {
            contents: contents.into_bytes(),
        },
    );

    // Test scroll down
    update_state(
        &mut state,
        Transition::ViewerScrollDown {
            viewport_height: 20,
        },
    );
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 1);

    update_state(
        &mut state,
        Transition::ViewerScrollDown {
            viewport_height: 20,
        },
    );
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 2);

    // Test scroll up
    update_state(&mut state, Transition::ViewerScrollUp);
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 1);

    // Test page down
    update_state(
        &mut state,
        Transition::ViewerPageDown {
            viewport_height: 20,
        },
    );
    assert!(state.viewer.as_ref().unwrap().line_offset > 1);

    let offset_after_page_down = state.viewer.as_ref().unwrap().line_offset;

    // Test page up
    update_state(
        &mut state,
        Transition::ViewerPageUp {
            viewport_height: 20,
        },
    );
    assert!(state.viewer.as_ref().unwrap().line_offset < offset_after_page_down);
}

#[test]
fn test_viewer_search() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.txt"));

    // Open viewer and load contents
    update_state(&mut state, Transition::OpenTextViewer { location });
    update_state(
        &mut state,
        Transition::ViewerLoadComplete {
            contents: b"Hello World\nHello Rust\nGoodbye World".to_vec(),
        },
    );

    // Start search for "Hello". Search runs as a background job (see
    // feat(viewer): background async viewer search with cancellation);
    // starting it only queues the job and marks is_searching, it does not
    // populate search_matches synchronously.
    let start_result = update_state(
        &mut state,
        Transition::ViewerStartSearch {
            query: "Hello".to_string(),
        },
    );
    assert_eq!(start_result.jobs_to_start.len(), 1);
    let job_id = start_result.jobs_to_start[0].id;

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.search_query, Some("Hello".to_string()));
    assert!(viewer.is_searching);

    // Simulate the background job completing with matches for "Hello" on
    // line 0 ("Hello World") and line 1 ("Hello Rust").
    update_state(
        &mut state,
        Transition::ViewerSearchComplete {
            job_id,
            matches: vec![(0, 0, 5), (1, 0, 5)],
        },
    );

    let viewer = state.viewer.as_ref().unwrap();
    assert!(!viewer.is_searching);
    assert_eq!(viewer.search_matches.len(), 2);
    assert_eq!(viewer.search_match_index, Some(0));

    // Find next
    update_state(&mut state, Transition::ViewerFindNext);
    assert_eq!(state.viewer.as_ref().unwrap().search_match_index, Some(1));

    // Find previous
    update_state(&mut state, Transition::ViewerFindPrev);
    assert_eq!(state.viewer.as_ref().unwrap().search_match_index, Some(0));

    // Clear search
    update_state(&mut state, Transition::ViewerClearSearch);
    assert_eq!(state.viewer.as_ref().unwrap().search_query, None);
    assert_eq!(state.viewer.as_ref().unwrap().search_matches.len(), 0);
}

#[test]
fn test_viewer_hex_mode_navigation() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.bin"));

    // Open hex viewer
    update_state(&mut state, Transition::OpenHexViewer { location });

    // Load binary contents (100 bytes = 7 hex lines)
    update_state(
        &mut state,
        Transition::ViewerLoadComplete {
            contents: vec![0u8; 100],
        },
    );

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.hex_line_count(), 7);

    // Test scrolling in hex mode
    update_state(
        &mut state,
        Transition::ViewerScrollDown {
            viewport_height: 20,
        },
    );
    // With 7 lines and viewport height of 20, we can't scroll
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 0);

    // Test jump to bottom in hex mode
    update_state(
        &mut state,
        Transition::ViewerJumpToBottom {
            viewport_height: 20,
        },
    );
    // With 7 lines and viewport height of 20, line_offset should be 0
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 0);
}

#[test]
fn test_close_viewer() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.txt"));

    // Open viewer
    update_state(&mut state, Transition::OpenTextViewer { location });
    assert!(state.viewer.is_some());
    assert_eq!(state.ui.mode, UIMode::Viewer);

    // Close viewer
    update_state(&mut state, Transition::CloseViewer);
    assert!(state.viewer.is_none());
    assert_eq!(state.ui.mode, UIMode::Normal);
}

#[test]
fn test_viewer_hex_line_formatting() {
    let mut state = test_state();

    let location = Location::Local(PathBuf::from("/test/file.bin"));

    // Open hex viewer
    update_state(&mut state, Transition::OpenHexViewer { location });

    // Load test data: "Hello World!" followed by some binary
    let contents = vec![
        0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x57, 0x6F, // "Hello Wo"
        0x72, 0x6C, 0x64, 0x21, 0x00, 0xFF, 0xAA, 0x55, // "rld!...."
    ];
    update_state(&mut state, Transition::ViewerLoadComplete { contents });

    let viewer = state.viewer.as_ref().unwrap();

    // Get first hex line's raw bytes (what the production renderer reads)
    let (offset, bytes) = viewer.get_hex_bytes_vec(0).unwrap();
    assert_eq!(offset, 0);
    assert_eq!(&bytes[0..4], &[0x48, 0x65, 0x6C, 0x6C]); // "Hell"
    assert_eq!(bytes.len(), 16);
}

// ── Seekable path e2e tests ───────────────────────────────────────────────────

#[test]
fn test_viewer_ready_with_seekable_buffer() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let content = b"Alpha line\nBeta line\nGamma line\n";
    std::fs::write(tmp.path(), content).unwrap();

    let file = std::fs::File::open(tmp.path()).unwrap();
    let sf = SeekableFile::new(file, content.len() as u64);
    let buffer = ViewerBuffer::new(
        FileBytes::Seekable(sf),
        LineIndex {
            offsets: vec![0, 11, 21],
            is_complete: true,
        },
    );

    let mut state = test_state();
    update_state(
        &mut state,
        Transition::OpenTextViewer {
            location: Location::Local(tmp.path().to_path_buf()),
        },
    );
    // Simulate ViewerReady arriving from the executor
    let load_job = state
        .viewer_job_id
        .expect("opening the viewer starts a load");
    update_state(
        &mut state,
        Transition::ViewerReady {
            job_id: load_job,
            buffer,
            encoding: TextEncoding::Utf8,
        },
    );

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.line_count(), 3);
    assert_eq!(viewer.get_line_str(0), Some("Alpha line".to_string()));
    assert_eq!(viewer.get_line_str(1), Some("Beta line".to_string()));
    assert_eq!(viewer.get_line_str(2), Some("Gamma line".to_string()));
    // text() cannot return &str for Seekable files
    assert_eq!(viewer.text(), None);
}

#[test]
fn test_viewer_seekable_text_search() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let content = b"Hello World\nHello Rust\nGoodbye World\n";
    std::fs::write(tmp.path(), content).unwrap();

    let file = std::fs::File::open(tmp.path()).unwrap();
    let sf = SeekableFile::new(file, content.len() as u64);
    let buffer = ViewerBuffer::new(
        FileBytes::Seekable(sf),
        LineIndex {
            offsets: vec![0, 12, 23],
            is_complete: true,
        },
    );

    let mut state = test_state();
    update_state(
        &mut state,
        Transition::OpenTextViewer {
            location: Location::Local(tmp.path().to_path_buf()),
        },
    );
    let load_job = state
        .viewer_job_id
        .expect("opening the viewer starts a load");
    update_state(
        &mut state,
        Transition::ViewerReady {
            job_id: load_job,
            buffer,
            encoding: TextEncoding::Utf8,
        },
    );
    // Search runs as a background job; starting it only queues the job.
    let start_result = update_state(
        &mut state,
        Transition::ViewerStartSearch {
            query: "Hello".to_string(),
        },
    );
    assert_eq!(start_result.jobs_to_start.len(), 1);
    let job_id = start_result.jobs_to_start[0].id;

    // Simulate the background job completing with matches for "Hello" on
    // line 0 ("Hello World") and line 1 ("Hello Rust").
    update_state(
        &mut state,
        Transition::ViewerSearchComplete {
            job_id,
            matches: vec![(0, 0, 5), (1, 0, 5)],
        },
    );

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.search_matches.len(), 2);
    assert_eq!(viewer.search_match_index, Some(0));

    update_state(&mut state, Transition::ViewerFindNext);
    assert_eq!(state.viewer.as_ref().unwrap().search_match_index, Some(1));
}

#[test]
fn test_viewer_seekable_hex_mode() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let data: Vec<u8> = (0u8..32).collect();
    std::fs::write(tmp.path(), &data).unwrap();

    let file = std::fs::File::open(tmp.path()).unwrap();
    let sf = SeekableFile::new(file, data.len() as u64);
    let buffer = ViewerBuffer::new(FileBytes::Seekable(sf), LineIndex::new_complete_empty());

    let mut state = test_state();
    update_state(
        &mut state,
        Transition::OpenHexViewer {
            location: Location::Local(tmp.path().to_path_buf()),
        },
    );
    let load_job = state
        .viewer_job_id
        .expect("opening the viewer starts a load");
    update_state(
        &mut state,
        Transition::ViewerReady {
            job_id: load_job,
            buffer,
            encoding: TextEncoding::Utf8,
        },
    );

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.mode, ViewerMode::Hex);
    assert_eq!(viewer.hex_line_count(), 2);

    let (offset, bytes) = viewer.get_hex_bytes_vec(0).unwrap();
    assert_eq!(offset, 0);
    assert_eq!(&bytes[0..4], &[0x00, 0x01, 0x02, 0x03]);
}

#[test]
fn test_viewer_seekable_hex_search() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // Write bytes with a known pattern 0xDE 0xAD 0xBE 0xEF at offset 8
    let mut data = vec![0u8; 32];
    data[8] = 0xDE;
    data[9] = 0xAD;
    data[10] = 0xBE;
    data[11] = 0xEF;
    std::fs::write(tmp.path(), &data).unwrap();

    let file = std::fs::File::open(tmp.path()).unwrap();
    let sf = SeekableFile::new(file, data.len() as u64);
    let buffer = ViewerBuffer::new(FileBytes::Seekable(sf), LineIndex::new_complete_empty());

    let mut state = test_state();
    update_state(
        &mut state,
        Transition::OpenHexViewer {
            location: Location::Local(tmp.path().to_path_buf()),
        },
    );
    let load_job = state
        .viewer_job_id
        .expect("opening the viewer starts a load");
    update_state(
        &mut state,
        Transition::ViewerReady {
            job_id: load_job,
            buffer,
            encoding: TextEncoding::Utf8,
        },
    );
    // Hex byte pattern search runs as a background job; starting it only
    // queues the job (see feat(viewer): background async viewer search).
    let start_result = update_state(
        &mut state,
        Transition::ViewerStartSearch {
            query: "DE AD BE EF".to_string(),
        },
    );
    assert_eq!(start_result.jobs_to_start.len(), 1);
    let job_id = start_result.jobs_to_start[0].id;

    // Simulate the background job completing with the byte-pattern match at
    // offset 8..12 (line = byte_offset / 16 = 0).
    update_state(
        &mut state,
        Transition::ViewerSearchComplete {
            job_id,
            matches: vec![(0, 8, 12)],
        },
    );

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.search_matches.len(), 1);
    assert_eq!(viewer.search_matches[0].1, 8); // byte offset 8
    assert_eq!(viewer.search_matches[0].2, 12); // byte offset 12
}

// ---------------------------------------------------------------------------
// SideBySide anchor pinning
//
// `viewer_anchor_pane` decides which file pane the SideBySide layout renders. If it
// disagrees with `ui.active_pane`, rwf draws the *other* pane: wrong entry list, wrong
// pane-info counts, and a path line with no active marker. The invariant under test is
// that the anchor is re-pinned to `ui.active_pane` at every entry into SideBySide.
// ---------------------------------------------------------------------------

/// Route A: reaching SideBySide via FullScreen ("v" then "V") goes through
/// `ViewerSwitchLayout`, not `OpenSideBySideViewer`. That path used to leave the anchor at
/// whatever it was before — `ActivePane::Left` on a fresh session — so opening the viewer
/// from the right pane rendered the left pane beside it.
#[test]
fn test_switch_layout_to_side_by_side_pins_anchor_to_active_pane() {
    use crate::model::{ActivePane, ViewerLayout};

    let mut state = test_state();
    state.ui.active_pane = ActivePane::Right;
    assert_eq!(
        state.ui.layout.viewer_anchor_pane,
        ActivePane::Left,
        "precondition: a fresh session defaults the anchor to Left"
    );

    update_state(
        &mut state,
        Transition::OpenTextViewer {
            location: Location::Local(PathBuf::from("/test/file.txt")),
        },
    );
    update_state(
        &mut state,
        Transition::ViewerSwitchLayout {
            layout: ViewerLayout::SideBySide,
        },
    );

    assert_eq!(state.ui.layout.viewer_layout, ViewerLayout::SideBySide);
    assert_eq!(state.ui.layout.viewer_anchor_pane, ActivePane::Right);
}

/// `OpenSideBySideViewer` ("V") pins the pane the user is standing on, from either side.
#[test]
fn test_open_side_by_side_viewer_pins_active_pane() {
    use crate::model::{ActivePane, ViewerLayout, ViewerMode};

    for pane in [ActivePane::Left, ActivePane::Right] {
        let mut state = test_state();
        state.ui.active_pane = pane;
        update_state(
            &mut state,
            Transition::OpenSideBySideViewer {
                location: Location::Local(PathBuf::from("/test/file.txt")),
                mode: ViewerMode::Text,
            },
        );
        assert_eq!(state.ui.layout.viewer_layout, ViewerLayout::SideBySide);
        assert_eq!(state.ui.layout.viewer_anchor_pane, pane);
    }
}

/// Leaving SideBySide for FullScreen and coming back re-pins against the pane that is
/// active at that moment, not the one from the previous SideBySide session.
#[test]
fn test_side_by_side_round_trip_repins_anchor() {
    use crate::model::{ActivePane, ViewerLayout, ViewerMode};

    let mut state = test_state();
    state.ui.active_pane = ActivePane::Left;
    update_state(
        &mut state,
        Transition::OpenSideBySideViewer {
            location: Location::Local(PathBuf::from("/test/file.txt")),
            mode: ViewerMode::Text,
        },
    );
    assert_eq!(state.ui.layout.viewer_anchor_pane, ActivePane::Left);

    update_state(
        &mut state,
        Transition::ViewerSwitchLayout {
            layout: ViewerLayout::FullScreen,
        },
    );
    // The user moves to the other pane while the viewer is full-screen.
    state.ui.active_pane = ActivePane::Right;
    update_state(
        &mut state,
        Transition::ViewerSwitchLayout {
            layout: ViewerLayout::SideBySide,
        },
    );

    assert_eq!(state.ui.layout.viewer_anchor_pane, ActivePane::Right);
}

/// Route B: the anchor belongs to its tab, not to the session. `ui.active_pane` is
/// global, so on a tab switch it arrives carrying whichever pane the *other* tab was
/// standing on -- re-deriving the anchor from it drags the viewer to the wrong side.
/// The tab's own anchor is restored, and `ui.active_pane` follows it.
#[test]
fn test_tab_switch_restores_the_tabs_own_anchor() {
    use crate::model::{ActivePane, ViewerLayout, ViewerMode};

    let mut state = test_state();
    state.tabs.create_tab(); // tab 1; active_index stays 0

    // Tab 0: SideBySide anchored to the left pane.
    state.ui.active_pane = ActivePane::Left;
    update_state(
        &mut state,
        Transition::OpenSideBySideViewer {
            location: Location::Local(PathBuf::from("/test/file.txt")),
            mode: ViewerMode::Text,
        },
    );
    assert_eq!(state.ui.layout.viewer_anchor_pane, ActivePane::Left);

    // Switch to tab 1 (no viewer there, so pane switching is allowed) and move right.
    update_state(&mut state, Transition::NextTab);
    assert!(state.viewer.is_none(), "tab 1 has no viewer");
    state.ui.active_pane = ActivePane::Right;

    // Back to tab 0: its viewer must come back on the side it was left on, with the
    // active pane moved to the anchored pane -- the only file pane on screen there.
    update_state(&mut state, Transition::PrevTab);
    assert!(state.viewer.is_some(), "tab 0's viewer is restored");
    assert_eq!(state.ui.layout.viewer_layout, ViewerLayout::SideBySide);
    assert_eq!(
        state.ui.layout.viewer_anchor_pane,
        ActivePane::Left,
        "tab 1's pane choice leaked into tab 0's anchor"
    );
    assert_eq!(
        state.ui.active_pane,
        ActivePane::Left,
        "the active pane must follow the restored anchor"
    );
}

/// Diagnostic bundle `20260909-172206`, verbatim: two tabs each holding their own
/// SideBySide viewer on opposite sides. Tab 1 anchors right (viewer on the left), tab 2
/// anchors left (viewer on the right); coming back to tab 1 flipped its viewer to the
/// right and re-pointed it at the other pane's cursor entry (`ReloadViewer README.md`
/// at seq 211). Two tabs is the minimum to reproduce -- with one, the global
/// `ui.active_pane` never disagrees with the anchor.
#[test]
fn test_two_tabs_keep_their_own_side_by_side_sides() {
    use crate::model::{ActivePane, ViewerLayout, ViewerMode};

    let mut state = test_state();
    state.tabs.create_tab();

    // Tab 0 anchors right: file pane on the right, viewer on the left.
    state.ui.active_pane = ActivePane::Right;
    update_state(
        &mut state,
        Transition::OpenSideBySideViewer {
            location: Location::Local(PathBuf::from("/test/tab0.txt")),
            mode: ViewerMode::Text,
        },
    );
    assert_eq!(state.ui.layout.viewer_anchor_pane, ActivePane::Right);

    // Tab 1 anchors left: file pane on the left, viewer on the right.
    update_state(&mut state, Transition::NextTab);
    state.ui.active_pane = ActivePane::Left;
    update_state(
        &mut state,
        Transition::OpenSideBySideViewer {
            location: Location::Local(PathBuf::from("/test/tab1.txt")),
            mode: ViewerMode::Hex,
        },
    );
    assert_eq!(state.ui.layout.viewer_anchor_pane, ActivePane::Left);

    // Back to tab 0 -- the step that broke.
    update_state(&mut state, Transition::PrevTab);
    assert_eq!(state.tabs.active_index, 0);
    assert_eq!(
        state.ui.layout.viewer_anchor_pane,
        ActivePane::Right,
        "tab 0's viewer jumped to the other side"
    );
    assert_eq!(state.ui.active_pane, ActivePane::Right);
    assert_eq!(state.ui.layout.viewer_layout, ViewerLayout::SideBySide);

    // ...and forward again: tab 1 keeps its own side too.
    update_state(&mut state, Transition::NextTab);
    assert_eq!(state.tabs.active_index, 1);
    assert_eq!(
        state.ui.layout.viewer_anchor_pane,
        ActivePane::Left,
        "tab 1's viewer jumped to the other side"
    );
    assert_eq!(state.ui.active_pane, ActivePane::Left);
}

/// `CreateTab` moves to the new tab, so it owes the same viewer save/restore pairing that
/// `NextTab` / `PrevTab` / `SwitchTab` do. Without it the viewer stays live in `AppState`
/// and the brand-new tab renders the previous tab's viewer beside its own file pane —
/// and the next switch away stashes that viewer onto the wrong tab, silently losing it
/// from the tab that actually opened it.
#[test]
fn test_create_tab_does_not_carry_the_viewer_to_the_new_tab() {
    use crate::model::{ActivePane, ViewerLayout, ViewerMode};

    let mut state = test_state();
    state.ui.active_pane = ActivePane::Left;
    update_state(
        &mut state,
        Transition::OpenSideBySideViewer {
            location: Location::Local(PathBuf::from("/test/file.txt")),
            mode: ViewerMode::Text,
        },
    );
    assert!(state.viewer.is_some(), "precondition: tab 0 has a viewer");

    update_state(&mut state, Transition::CreateTab);

    assert_eq!(state.tabs.active_index, 1, "moved to the new tab");
    assert!(
        state.viewer.is_none(),
        "the new tab must start with no viewer"
    );
    assert_eq!(
        state.ui.layout.viewer_layout,
        ViewerLayout::FullScreen,
        "the new tab must not inherit the SideBySide layout"
    );

    // The viewer belongs to tab 0 and must still be there on the way back.
    update_state(&mut state, Transition::PrevTab);
    assert_eq!(state.tabs.active_index, 0);
    assert!(
        state.viewer.is_some(),
        "tab 0's viewer was lost across the CreateTab round trip"
    );
    assert_eq!(state.ui.layout.viewer_layout, ViewerLayout::SideBySide);
}

/// The 300ms `last_tab_created` debounce returns *before* the viewer hand-off. Ordering
/// matters: a save hoisted above that early return would stash the current tab's viewer and
/// then bail, blanking a viewer that is on screen without creating anything.
///
/// Reaching that needs a debounced CreateTab while the current tab holds a live viewer —
/// created here by bouncing back to tab 0 inside the debounce window.
#[test]
fn test_debounced_create_tab_leaves_the_viewer_untouched() {
    use crate::model::{ActivePane, ViewerLayout, ViewerMode};

    let mut state = test_state();
    state.ui.active_pane = ActivePane::Left;
    update_state(
        &mut state,
        Transition::OpenSideBySideViewer {
            location: Location::Local(PathBuf::from("/test/file.txt")),
            mode: ViewerMode::Text,
        },
    );

    update_state(&mut state, Transition::CreateTab);
    assert_eq!(state.tabs.active_index, 1);

    // Straight back to the tab that owns the viewer, still inside the debounce window.
    update_state(&mut state, Transition::PrevTab);
    assert!(state.viewer.is_some(), "tab 0's viewer is back on screen");

    // Swallowed by the debounce — and must leave that viewer exactly where it is.
    update_state(&mut state, Transition::CreateTab);

    assert_eq!(
        state.tabs.tabs.len(),
        2,
        "the second CreateTab was swallowed"
    );
    assert_eq!(state.tabs.active_index, 0, "still on tab 0");
    assert!(
        state.viewer.is_some(),
        "a swallowed CreateTab blanked the visible viewer"
    );
    assert_eq!(state.ui.layout.viewer_layout, ViewerLayout::SideBySide);
}

// ---------------------------------------------------------------------------
// ViewerReady routing
//
// A `LoadFileForViewer` job outlives the tab hand-off: `save_tab_ui_state`
// moves a still-loading viewer into its tab's slot while the job keeps running. The
// `JobEvent::ViewerReady` that eventually arrives carries no tab identity, so the
// buffer has to be routed by job id -- otherwise it lands in whatever viewer happens
// to be live, and the tab that started the load never gets its contents.
// ---------------------------------------------------------------------------

/// A small in-memory buffer whose contents identify which load produced it.
fn in_memory_buffer(text: &str) -> ViewerBuffer {
    let bytes = text.as_bytes().to_vec();
    let mut offsets = vec![0u64];
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' && i + 1 < bytes.len() {
            offsets.push(i as u64 + 1);
        }
    }
    ViewerBuffer::new(
        FileBytes::InMemory(bytes),
        LineIndex {
            offsets,
            is_complete: true,
        },
    )
}

/// `ReloadViewer` cancels the in-flight load and starts a new one, but a cancel is a
/// request -- the old job can still finish and emit its buffer. Without a job-id check
/// that late buffer overwrites the file the user is actually looking at.
#[test]
fn test_viewer_ready_from_a_superseded_job_is_dropped() {
    let mut state = test_state();
    update_state(
        &mut state,
        Transition::OpenTextViewer {
            location: Location::Local(PathBuf::from("/test/wanted.txt")),
        },
    );
    let live_job = state.viewer_job_id.expect("OpenTextViewer starts a load");

    let superseded = crate::job::JobId::new();
    assert_ne!(superseded, live_job);
    update_state(
        &mut state,
        Transition::ViewerReady {
            job_id: superseded,
            buffer: in_memory_buffer("SUPERSEDED"),
            encoding: TextEncoding::Utf8,
        },
    );

    assert!(
        state
            .viewer
            .as_ref()
            .expect("viewer is open")
            .buffer
            .is_none(),
        "a buffer from a superseded load was shown"
    );

    // The buffer the user is waiting for still arrives normally.
    update_state(
        &mut state,
        Transition::ViewerReady {
            job_id: live_job,
            buffer: in_memory_buffer("WANTED"),
            encoding: TextEncoding::Utf8,
        },
    );
    assert_eq!(state.viewer.as_ref().and_then(|v| v.text()), Some("WANTED"));
}

/// `CreateTab` stashes a still-loading viewer into the tab it leaves. The buffer must
/// follow the job to that tab's slot: delivering it to the live viewer shows the
/// previous tab's file, and dropping it leaves the owning tab permanently empty.
#[test]
fn test_viewer_ready_reaches_the_tab_that_started_the_load() {
    use crate::model::ActivePane;

    let mut state = test_state();
    state.ui.active_pane = ActivePane::Left;
    update_state(
        &mut state,
        Transition::OpenSideBySideViewer {
            location: Location::Local(PathBuf::from("/test/tab0.txt")),
            mode: ViewerMode::Text,
        },
    );
    let tab0_job = state.viewer_job_id.expect("tab 0 started a load");

    // Ctrl+T before the load finishes: the viewer moves into tab 0's slot, job and all.
    update_state(&mut state, Transition::CreateTab);
    assert_eq!(state.tabs.active_index, 1);
    assert!(state.viewer.is_none(), "the new tab starts with no viewer");

    // The new tab opens a viewer of its own, so there is a live viewer to leak into.
    update_state(
        &mut state,
        Transition::OpenSideBySideViewer {
            location: Location::Local(PathBuf::from("/test/tab1.txt")),
            mode: ViewerMode::Text,
        },
    );
    let tab1_job = state.viewer_job_id.expect("tab 1 started a load");
    assert_ne!(tab0_job, tab1_job);

    // Tab 0's load finally completes while tab 1 is on screen.
    update_state(
        &mut state,
        Transition::ViewerReady {
            job_id: tab0_job,
            buffer: in_memory_buffer("TAB ZERO"),
            encoding: TextEncoding::Utf8,
        },
    );

    assert!(
        state
            .viewer
            .as_ref()
            .expect("tab 1's viewer is on screen")
            .buffer
            .is_none(),
        "tab 0's buffer leaked into the viewer the user is looking at"
    );

    // ...and it is waiting on tab 0 when the user goes back.
    update_state(&mut state, Transition::PrevTab);
    assert_eq!(state.tabs.active_index, 0);
    assert_eq!(
        state.viewer.as_ref().and_then(|v| v.text()),
        Some("TAB ZERO"),
        "the tab that started the load never received its contents"
    );
}
