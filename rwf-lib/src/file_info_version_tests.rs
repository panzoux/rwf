//! Integration tests for file information and version display

use crate::model::{Dialog, DialogContent};
use crate::state::{update_state, Transition};
use crate::test_utils::{test_state, FileEntryBuilder};

#[test]
fn test_show_file_info_transition() {
    let mut state = test_state();

    // Add a file entry to the active pane
    let entry = FileEntryBuilder::new("test.txt").size(1024).build();

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
        DialogContent::FileInfo {
            file_name,
            file_path,
            size,
            is_dir,
            ..
        } => {
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
    let mut state = test_state();

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
    let mut state = test_state();

    // Add a directory entry to the active pane
    let entry = FileEntryBuilder::new("test_dir")
        .size(0)
        .dir(true)
        .calculated_size(Some(4096))
        .build();

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
        DialogContent::FileInfo {
            file_name,
            is_dir,
            size,
            ..
        } => {
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
    let mut state = test_state();

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
        DialogContent::Version(v) => {
            assert!(!v.version.is_empty());
            assert!(!v.build_date.is_empty());
            assert!(!v.copyright.is_empty());
            assert!(v.copyright.contains("Copyright"));
        }
        _ => panic!("Expected Version dialog content"),
    }
}

#[test]
fn test_file_info_dialog_creation() {
    // Create a test file entry
    let entry = FileEntryBuilder::new("example.txt")
        .path("/home/user/example.txt")
        .size(2048)
        .build();

    // Create file info dialog
    let dialog = Dialog::file_info(&entry);

    assert_eq!(dialog.title, "File Information");

    match dialog.content {
        DialogContent::FileInfo {
            file_name,
            file_path,
            size,
            is_dir,
            ..
        } => {
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
        DialogContent::Version(v) => {
            // Version should be from CARGO_PKG_VERSION
            assert!(!v.version.is_empty());
            // Build date may be "Unknown" in test environment
            assert!(!v.build_date.is_empty());
            // Copyright should contain expected text
            assert!(v.copyright.contains("Copyright"));
            assert!(v.copyright.contains("RWF Contributors"));
        }
        _ => panic!("Expected Version dialog content"),
    }
}

#[test]
fn test_file_info_dialog_dismissal() {
    let mut state = test_state();

    // Add a file entry and show file info
    let entry = FileEntryBuilder::new("test.txt").size(1024).build();

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
    let mut state = test_state();

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
    let mut state = test_state();

    // Add a directory entry with calculated size
    let entry = FileEntryBuilder::new("large_dir")
        .size(0)
        .dir(true)
        .calculated_size(Some(1048576)) // 1 MB
        .build();

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
    let entry = FileEntryBuilder::new("test.txt").size(1024).build();

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

#[cfg(test)]
mod link_info_tests {
    use crate::model::{Dialog, DialogContent, FileEntry, LinkKind, Location};
    use std::path::PathBuf;
    use std::time::SystemTime;

    #[test]
    fn test_file_info_symlink_fields_populated() {
        let entry = FileEntry {
            name: "_vimrc".to_string(),
            location: Location::Local(PathBuf::from("/home/user/_vimrc")),
            size: 0,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: true,
            link_target: Some(PathBuf::from("./.vimrc")),
            link_kind: Some(LinkKind::Symlink),
        };

        let dialog = Dialog::file_info(&entry);

        match dialog.content {
            DialogContent::FileInfo {
                link_target,
                link_kind,
                ..
            } => {
                assert_eq!(link_target, Some("./.vimrc".to_string()));
                assert!(matches!(link_kind, Some(LinkKind::Symlink)));
            }
            _ => panic!("Expected FileInfo dialog content"),
        }
    }

    #[test]
    fn test_file_info_regular_file_no_link_fields() {
        let entry = FileEntry {
            name: "file.txt".to_string(),
            location: Location::Local(PathBuf::from("/home/user/file.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        };

        let dialog = Dialog::file_info(&entry);

        match dialog.content {
            DialogContent::FileInfo {
                link_target,
                link_kind,
                ..
            } => {
                assert_eq!(link_target, None);
                assert_eq!(link_kind, None);
            }
            _ => panic!("Expected FileInfo dialog content"),
        }
    }

    #[test]
    fn test_file_info_junction_strips_nt_prefix() {
        let entry = FileEntry {
            name: "Application Data".to_string(),
            location: Location::Local(PathBuf::from(r"C:\Users\user\Application Data")),
            size: 0,
            is_dir: true,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: true,
            link_target: Some(PathBuf::from(r"\??\C:\ProgramData")),
            link_kind: Some(LinkKind::Junction),
        };

        let dialog = Dialog::file_info(&entry);

        match dialog.content {
            DialogContent::FileInfo { link_target, .. } => {
                assert_eq!(link_target, Some(r"C:\ProgramData".to_string()));
            }
            _ => panic!("Expected FileInfo dialog content"),
        }
    }
}
