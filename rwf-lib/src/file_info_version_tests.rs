//! Integration tests for file information and version display

use crate::model::{Dialog, DialogContent, FileEntry, Location};
use crate::state::{update_state, Transition};
use crate::AppState;
use crate::config::AppConfig;
use std::path::PathBuf;
use std::time::SystemTime;

#[test]
fn test_show_file_info_transition() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add a file entry to the active pane
    let entry = FileEntry {
        name: "test.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/test.txt")),
        size: 1024,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::now(),
        marked: false,
        calculated_size: None,
    };
    
    state.current_tab_mut().left_pane.entries = vec![entry];
    state.current_tab_mut().left_pane.cursor = 0;
    
    // Trigger ShowFileInfo transition
    let result = update_state(&mut state, Transition::ShowFileInfo);
    
    // Verify UI changed
    assert!(result.ui_changed);
    
    // Verify dialog was created
    assert!(!state.dialogs.is_empty());
    
    let dialog = state.dialogs.current().unwrap();
    assert_eq!(dialog.title, "File Information");
    
    // Verify dialog content
    match &dialog.content {
        DialogContent::FileInfo { file_name, file_path, size, is_dir, .. } => {
            assert_eq!(file_name, "test.txt");
            assert!(file_path.contains("test.txt"));
            assert_eq!(*size, 1024);
            assert!(!is_dir);
        }
        _ => panic!("Expected FileInfo dialog content"),
    }
}

#[test]
fn test_show_file_info_no_entry() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // No entries in the pane
    state.current_tab_mut().left_pane.entries = vec![];
    
    // Trigger ShowFileInfo transition
    let result = update_state(&mut state, Transition::ShowFileInfo);
    
    // Verify no dialog was created
    assert!(!result.ui_changed);
    assert!(state.dialogs.is_empty());
}

#[test]
fn test_show_file_info_directory() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add a directory entry to the active pane
    let entry = FileEntry {
        name: "test_dir".to_string(),
        location: Location::Local(PathBuf::from("/test/test_dir")),
        size: 0,
        is_dir: true,
        is_hidden: false,
        modified: SystemTime::now(),
        marked: false,
        calculated_size: Some(4096),
    };
    
    state.current_tab_mut().left_pane.entries = vec![entry];
    state.current_tab_mut().left_pane.cursor = 0;
    
    // Trigger ShowFileInfo transition
    let result = update_state(&mut state, Transition::ShowFileInfo);
    
    // Verify UI changed
    assert!(result.ui_changed);
    
    // Verify dialog was created
    assert!(!state.dialogs.is_empty());
    
    let dialog = state.dialogs.current().unwrap();
    assert_eq!(dialog.title, "File Information");
    
    // Verify dialog content
    match &dialog.content {
        DialogContent::FileInfo { file_name, is_dir, size, .. } => {
            assert_eq!(file_name, "test_dir");
            assert!(*is_dir);
            // Should use calculated_size if available
            assert_eq!(*size, 4096);
        }
        _ => panic!("Expected FileInfo dialog content"),
    }
}

#[test]
fn test_show_version_transition() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Trigger ShowVersion transition
    let result = update_state(&mut state, Transition::ShowVersion);
    
    // Verify UI changed
    assert!(result.ui_changed);
    
    // Verify dialog was created
    assert!(!state.dialogs.is_empty());
    
    let dialog = state.dialogs.current().unwrap();
    assert_eq!(dialog.title, "Version Information");
    
    // Verify dialog content
    match &dialog.content {
        DialogContent::Version { version, build_date, copyright } => {
            assert!(!version.is_empty());
            assert!(!build_date.is_empty());
            assert!(!copyright.is_empty());
            assert!(copyright.contains("Copyright"));
        }
        _ => panic!("Expected Version dialog content"),
    }
}

#[test]
fn test_file_info_dialog_creation() {
    // Create a test file entry
    let entry = FileEntry {
        name: "example.txt".to_string(),
        location: Location::Local(PathBuf::from("/home/user/example.txt")),
        size: 2048,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::now(),
        marked: false,
        calculated_size: None,
    };
    
    // Create file info dialog
    let dialog = Dialog::file_info(&entry);
    
    assert_eq!(dialog.title, "File Information");
    
    match dialog.content {
        DialogContent::FileInfo { file_name, file_path, size, is_dir, .. } => {
            assert_eq!(file_name, "example.txt");
            assert!(file_path.contains("example.txt"));
            assert_eq!(size, 2048);
            assert!(!is_dir);
        }
        _ => panic!("Expected FileInfo dialog content"),
    }
}

#[test]
fn test_version_dialog_creation() {
    // Create version dialog
    let dialog = Dialog::version();
    
    assert_eq!(dialog.title, "Version Information");
    
    match dialog.content {
        DialogContent::Version { version, build_date, copyright } => {
            // Version should be from CARGO_PKG_VERSION
            assert!(!version.is_empty());
            // Build date may be "Unknown" in test environment
            assert!(!build_date.is_empty());
            // Copyright should contain expected text
            assert!(copyright.contains("Copyright"));
            assert!(copyright.contains("RWF Contributors"));
        }
        _ => panic!("Expected Version dialog content"),
    }
}

#[test]
fn test_file_info_dialog_dismissal() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add a file entry and show file info
    let entry = FileEntry {
        name: "test.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/test.txt")),
        size: 1024,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::now(),
        marked: false,
        calculated_size: None,
    };
    
    state.current_tab_mut().left_pane.entries = vec![entry];
    state.current_tab_mut().left_pane.cursor = 0;
    
    update_state(&mut state, Transition::ShowFileInfo);
    assert!(!state.dialogs.is_empty());
    
    // Close the dialog
    let result = update_state(&mut state, Transition::CloseDialog);
    
    // Verify dialog was closed
    assert!(result.ui_changed);
    assert!(state.dialogs.is_empty());
}

#[test]
fn test_version_dialog_dismissal() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Show version dialog
    update_state(&mut state, Transition::ShowVersion);
    assert!(!state.dialogs.is_empty());
    
    // Close the dialog
    let result = update_state(&mut state, Transition::CloseDialog);
    
    // Verify dialog was closed
    assert!(result.ui_changed);
    assert!(state.dialogs.is_empty());
}

#[test]
fn test_file_info_with_calculated_size() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add a directory entry with calculated size
    let entry = FileEntry {
        name: "large_dir".to_string(),
        location: Location::Local(PathBuf::from("/test/large_dir")),
        size: 0,
        is_dir: true,
        is_hidden: false,
        modified: SystemTime::now(),
        marked: false,
        calculated_size: Some(1048576), // 1 MB
    };
    
    state.current_tab_mut().left_pane.entries = vec![entry];
    state.current_tab_mut().left_pane.cursor = 0;
    
    // Trigger ShowFileInfo transition
    update_state(&mut state, Transition::ShowFileInfo);
    
    let dialog = state.dialogs.current().unwrap();
    
    // Verify calculated size is used
    match &dialog.content {
        DialogContent::FileInfo { size, .. } => {
            assert_eq!(*size, 1048576);
        }
        _ => panic!("Expected FileInfo dialog content"),
    }
}

#[test]
fn test_file_info_dialog_does_not_require_input() {
    let entry = FileEntry {
        name: "test.txt".to_string(),
        location: Location::Local(PathBuf::from("/test/test.txt")),
        size: 1024,
        is_dir: false,
        is_hidden: false,
        modified: SystemTime::now(),
        marked: false,
        calculated_size: None,
    };
    
    let dialog = Dialog::file_info(&entry);
    
    // FileInfo dialog should not require input
    assert!(!dialog.content.requires_input());
}

#[test]
fn test_version_dialog_does_not_require_input() {
    let dialog = Dialog::version();
    
    // Version dialog should not require input
    assert!(!dialog.content.requires_input());
}
