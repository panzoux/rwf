//! Integration tests for file viewer functionality
//!
//! Tests the complete viewer workflow including:
//! - Opening text and hex viewers
//! - Loading file contents as jobs
//! - Encoding switching
//! - Navigation (Home/End, F5/F6, scrolling)
//! - Search functionality (F4, F3, Shift+F3)

use crate::config::AppConfig;
use crate::job::JobKind;
use crate::model::{Location, ViewerMode, TextEncoding, UIMode};
use crate::model::viewer::{FileBytes, LineIndex, SeekableFile, ViewerBuffer};
use crate::state::{AppState, Transition, update_state};
use std::path::PathBuf;

#[test]
fn test_open_text_viewer() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.txt"));
    
    // Open text viewer
    let result = update_state(&mut state, Transition::OpenTextViewer {
        location: location.clone(),
    });
    
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
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.bin"));
    
    // Open hex viewer
    let result = update_state(&mut state, Transition::OpenHexViewer {
        location: location.clone(),
    });
    
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
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.txt"));
    
    // Open text viewer
    update_state(&mut state, Transition::OpenTextViewer {
        location: location.clone(),
    });
    
    // Simulate file load completion
    let contents = b"Hello World\nLine 2\nLine 3".to_vec();
    update_state(&mut state, Transition::ViewerLoadComplete {
        contents: contents.clone(),
    });
    
    // Verify contents are loaded (InMemory path via set_contents)
    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.line_count(), 3);
    assert_eq!(viewer.text(), Some("Hello World\nLine 2\nLine 3"));
}

#[test]
fn test_viewer_encoding_cycle() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.txt"));
    
    // Open text viewer and load contents
    update_state(&mut state, Transition::OpenTextViewer { location });
    update_state(&mut state, Transition::ViewerLoadComplete {
        contents: b"Test".to_vec(),
    });
    
    // Initial encoding should be UTF-8
    assert_eq!(state.viewer.as_ref().unwrap().encoding, TextEncoding::Utf8);
    
    // Cycle encoding
    update_state(&mut state, Transition::ViewerCycleEncoding);
    assert_eq!(state.viewer.as_ref().unwrap().encoding, TextEncoding::Utf16Le);
    
    update_state(&mut state, Transition::ViewerCycleEncoding);
    assert_eq!(state.viewer.as_ref().unwrap().encoding, TextEncoding::Utf16Be);
    
    update_state(&mut state, Transition::ViewerCycleEncoding);
    assert_eq!(state.viewer.as_ref().unwrap().encoding, TextEncoding::ShiftJis);
}

#[test]
fn test_viewer_navigation_home_end() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.txt"));
    
    // Open viewer and load contents
    update_state(&mut state, Transition::OpenTextViewer { location });
    update_state(&mut state, Transition::ViewerLoadComplete {
        contents: b"Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_vec(),
    });
    
    // Set line offset to middle
    state.viewer.as_mut().unwrap().line_offset = 2;
    state.viewer.as_mut().unwrap().column_offset = 5;
    
    // Test Home (move to line start)
    update_state(&mut state, Transition::ViewerMoveToLineStart);
    assert_eq!(state.viewer.as_ref().unwrap().column_offset, 0);
    
    // Test End (move to line end)
    update_state(&mut state, Transition::ViewerMoveToLineEnd { viewport_width: 80 });
    // Column offset should still be 0 since line is short
    assert_eq!(state.viewer.as_ref().unwrap().column_offset, 0);
}

#[test]
fn test_viewer_navigation_top_bottom() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.txt"));
    
    // Open viewer and load contents
    update_state(&mut state, Transition::OpenTextViewer { location });
    update_state(&mut state, Transition::ViewerLoadComplete {
        contents: b"Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_vec(),
    });
    
    // Set line offset to middle
    state.viewer.as_mut().unwrap().line_offset = 2;
    
    // Test F5 (jump to top)
    update_state(&mut state, Transition::ViewerJumpToTop);
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 0);
    
    // Test F6 (jump to bottom)
    update_state(&mut state, Transition::ViewerJumpToBottom { viewport_height: 20 });
    // With 5 lines and viewport height of 20, line_offset should be 0
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 0);
}

#[test]
fn test_viewer_scrolling() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.txt"));
    
    // Open viewer and load contents with many lines
    update_state(&mut state, Transition::OpenTextViewer { location });
    let mut contents = String::new();
    for i in 0..100 {
        contents.push_str(&format!("Line {}\n", i));
    }
    update_state(&mut state, Transition::ViewerLoadComplete {
        contents: contents.into_bytes(),
    });
    
    // Test scroll down
    update_state(&mut state, Transition::ViewerScrollDown { viewport_height: 20 });
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 1);
    
    update_state(&mut state, Transition::ViewerScrollDown { viewport_height: 20 });
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 2);
    
    // Test scroll up
    update_state(&mut state, Transition::ViewerScrollUp);
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 1);
    
    // Test page down
    update_state(&mut state, Transition::ViewerPageDown { viewport_height: 20 });
    assert!(state.viewer.as_ref().unwrap().line_offset > 1);
    
    let offset_after_page_down = state.viewer.as_ref().unwrap().line_offset;
    
    // Test page up
    update_state(&mut state, Transition::ViewerPageUp { viewport_height: 20 });
    assert!(state.viewer.as_ref().unwrap().line_offset < offset_after_page_down);
}

#[test]
fn test_viewer_search() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.txt"));
    
    // Open viewer and load contents
    update_state(&mut state, Transition::OpenTextViewer { location });
    update_state(&mut state, Transition::ViewerLoadComplete {
        contents: b"Hello World\nHello Rust\nGoodbye World".to_vec(),
    });
    
    // Start search for "Hello". Search runs as a background job (see
    // feat(viewer): background async viewer search with cancellation);
    // starting it only queues the job and marks is_searching, it does not
    // populate search_matches synchronously.
    let start_result = update_state(&mut state, Transition::ViewerStartSearch {
        query: "Hello".to_string(),
    });
    assert_eq!(start_result.jobs_to_start.len(), 1);
    let job_id = start_result.jobs_to_start[0].id;

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.search_query, Some("Hello".to_string()));
    assert!(viewer.is_searching);

    // Simulate the background job completing with matches for "Hello" on
    // line 0 ("Hello World") and line 1 ("Hello Rust").
    update_state(&mut state, Transition::ViewerSearchComplete {
        job_id,
        matches: vec![(0, 0, 5), (1, 0, 5)],
    });

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
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    let location = Location::Local(PathBuf::from("/test/file.bin"));
    
    // Open hex viewer
    update_state(&mut state, Transition::OpenHexViewer { location });
    
    // Load binary contents (100 bytes = 7 hex lines)
    update_state(&mut state, Transition::ViewerLoadComplete {
        contents: vec![0u8; 100],
    });
    
    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.hex_line_count(), 7);
    
    // Test scrolling in hex mode
    update_state(&mut state, Transition::ViewerScrollDown { viewport_height: 20 });
    // With 7 lines and viewport height of 20, we can't scroll
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 0);
    
    // Test jump to bottom in hex mode
    update_state(&mut state, Transition::ViewerJumpToBottom { viewport_height: 20 });
    // With 7 lines and viewport height of 20, line_offset should be 0
    assert_eq!(state.viewer.as_ref().unwrap().line_offset, 0);
}

#[test]
fn test_close_viewer() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
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
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
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
    
    // Get first hex line
    let (offset, hex, ascii) = viewer.get_hex_line(0).unwrap();
    assert_eq!(offset, 0);
    assert!(hex.contains("48 65 6C 6C")); // "Hell"
    assert!(ascii.contains("Hello"));
    assert!(ascii.contains("World"));
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
        LineIndex { offsets: vec![0, 11, 21], is_complete: true },
    );

    let config = AppConfig::default();
    let mut state = AppState::new(config);
    update_state(&mut state, Transition::OpenTextViewer {
        location: Location::Local(tmp.path().to_path_buf()),
    });
    // Simulate ViewerReady arriving from the executor
    update_state(&mut state, Transition::ViewerReady {
        buffer,
        encoding: TextEncoding::Utf8,
    });

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
        LineIndex { offsets: vec![0, 12, 23], is_complete: true },
    );

    let config = AppConfig::default();
    let mut state = AppState::new(config);
    update_state(&mut state, Transition::OpenTextViewer {
        location: Location::Local(tmp.path().to_path_buf()),
    });
    update_state(&mut state, Transition::ViewerReady { buffer, encoding: TextEncoding::Utf8 });
    // Search runs as a background job; starting it only queues the job.
    let start_result = update_state(&mut state, Transition::ViewerStartSearch { query: "Hello".to_string() });
    assert_eq!(start_result.jobs_to_start.len(), 1);
    let job_id = start_result.jobs_to_start[0].id;

    // Simulate the background job completing with matches for "Hello" on
    // line 0 ("Hello World") and line 1 ("Hello Rust").
    update_state(&mut state, Transition::ViewerSearchComplete {
        job_id,
        matches: vec![(0, 0, 5), (1, 0, 5)],
    });

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
    let buffer = ViewerBuffer::new(
        FileBytes::Seekable(sf),
        LineIndex::new_complete_empty(),
    );

    let config = AppConfig::default();
    let mut state = AppState::new(config);
    update_state(&mut state, Transition::OpenHexViewer {
        location: Location::Local(tmp.path().to_path_buf()),
    });
    update_state(&mut state, Transition::ViewerReady { buffer, encoding: TextEncoding::Utf8 });

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.mode, ViewerMode::Hex);
    assert_eq!(viewer.hex_line_count(), 2);

    let (offset, hex, _ascii) = viewer.get_hex_line(0).unwrap();
    assert_eq!(offset, 0);
    assert!(hex.contains("00 01 02 03"));
}

#[test]
fn test_viewer_seekable_hex_search() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // Write bytes with a known pattern 0xDE 0xAD 0xBE 0xEF at offset 8
    let mut data = vec![0u8; 32];
    data[8] = 0xDE; data[9] = 0xAD; data[10] = 0xBE; data[11] = 0xEF;
    std::fs::write(tmp.path(), &data).unwrap();

    let file = std::fs::File::open(tmp.path()).unwrap();
    let sf = SeekableFile::new(file, data.len() as u64);
    let buffer = ViewerBuffer::new(FileBytes::Seekable(sf), LineIndex::new_complete_empty());

    let config = AppConfig::default();
    let mut state = AppState::new(config);
    update_state(&mut state, Transition::OpenHexViewer {
        location: Location::Local(tmp.path().to_path_buf()),
    });
    update_state(&mut state, Transition::ViewerReady { buffer, encoding: TextEncoding::Utf8 });
    // Hex byte pattern search runs as a background job; starting it only
    // queues the job (see feat(viewer): background async viewer search).
    let start_result = update_state(&mut state, Transition::ViewerStartSearch { query: "DE AD BE EF".to_string() });
    assert_eq!(start_result.jobs_to_start.len(), 1);
    let job_id = start_result.jobs_to_start[0].id;

    // Simulate the background job completing with the byte-pattern match at
    // offset 8..12 (line = byte_offset / 16 = 0).
    update_state(&mut state, Transition::ViewerSearchComplete {
        job_id,
        matches: vec![(0, 8, 12)],
    });

    let viewer = state.viewer.as_ref().unwrap();
    assert_eq!(viewer.search_matches.len(), 1);
    assert_eq!(viewer.search_matches[0].1, 8);  // byte offset 8
    assert_eq!(viewer.search_matches[0].2, 12); // byte offset 12
}
