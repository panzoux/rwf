//! Integration tests for pattern-based rename functionality

use crate::state::{AppState, update_state, Transition, AppConfig};
use crate::model::{Location, FileEntry, DialogContent};
use crate::job::{JobKind, OpResult, SuccessData};
use std::path::PathBuf;
use std::time::SystemTime;

#[test]
fn test_show_pattern_rename_dialog_with_marked_files() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add some files to the active pane
    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntry {
            name: "file1.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/file1.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
        FileEntry {
            name: "file2.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/file2.txt")),
            size: 200,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
    ];
    
    // Mark the files
    update_state(&mut state, Transition::MarkAll);
    
    // Show pattern rename dialog
    update_state(&mut state, Transition::ShowPatternRenameDialog);
    
    // Verify dialog is shown
    assert!(!state.dialogs.is_empty());
    let dialog = state.dialogs.current().unwrap();
    assert!(matches!(dialog.content, DialogContent::PatternRename { .. }));
}

#[test]
fn test_show_pattern_rename_dialog_with_cursor_file() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add a file to the active pane
    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntry {
            name: "document.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/document.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
    ];
    
    // Show pattern rename dialog
    update_state(&mut state, Transition::ShowPatternRenameDialog);
    
    // Verify dialog is shown
    assert!(!state.dialogs.is_empty());
    let dialog = state.dialogs.current().unwrap();
    assert!(matches!(dialog.content, DialogContent::PatternRename { .. }));
}

#[test]
fn test_update_pattern_rename_pattern() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add files to the active pane
    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntry {
            name: "file1.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/file1.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
        FileEntry {
            name: "file2.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/file2.txt")),
            size: 200,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
    ];
    
    // Mark the files
    update_state(&mut state, Transition::MarkAll);
    
    // Show pattern rename dialog
    update_state(&mut state, Transition::ShowPatternRenameDialog);
    
    // Update the pattern
    let pattern = "*.txt -> backup_[1].txt".to_string();
    update_state(&mut state, Transition::UpdatePatternRenamePattern { pattern: pattern.clone() });
    
    // Verify the pattern and preview are updated
    let dialog = state.dialogs.current().unwrap();
    if let Some((dialog_pattern, preview)) = dialog.content.as_pattern_rename() {
        assert_eq!(dialog_pattern, &pattern);
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0], ("file1.txt".to_string(), "backup_file1.txt".to_string()));
        assert_eq!(preview[1], ("file2.txt".to_string(), "backup_file2.txt".to_string()));
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_execute_pattern_rename_creates_job() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add files to the active pane
    let pane = state.active_pane_mut();
    let file1_location = Location::Local(PathBuf::from("/test/file1.txt"));
    let file2_location = Location::Local(PathBuf::from("/test/file2.txt"));
    pane.entries = vec![
        FileEntry {
            name: "file1.txt".to_string(),
            location: file1_location.clone(),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
        FileEntry {
            name: "file2.txt".to_string(),
            location: file2_location.clone(),
            size: 200,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
    ];
    
    // Mark the files
    update_state(&mut state, Transition::MarkAll);
    
    // Execute pattern rename
    let pattern = "*.txt -> backup_[1].txt".to_string();
    let targets = vec![file1_location, file2_location];
    let result = update_state(&mut state, Transition::ExecutePatternRename {
        pattern: pattern.clone(),
        targets: targets.clone(),
    });
    
    // Verify a job was created
    assert_eq!(result.jobs_to_start.len(), 1);
    let job_spec = &result.jobs_to_start[0];
    
    match &job_spec.kind {
        JobKind::PatternRename { targets: job_targets, pattern: job_pattern } => {
            assert_eq!(job_targets.len(), 2);
            assert_eq!(job_pattern, &pattern);
        }
        _ => panic!("Expected PatternRename job kind"),
    }
    
    // Verify dialog is closed
    assert!(state.dialogs.is_empty());
}

#[test]
fn test_pattern_rename_with_swap() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add files to the active pane
    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntry {
            name: "hello_world.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/hello_world.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
    ];
    
    // Show pattern rename dialog
    update_state(&mut state, Transition::ShowPatternRenameDialog);
    
    // Update the pattern to swap parts
    let pattern = "*_*.txt -> [2]_[1].txt".to_string();
    update_state(&mut state, Transition::UpdatePatternRenamePattern { pattern: pattern.clone() });
    
    // Verify the preview shows the swap
    let dialog = state.dialogs.current().unwrap();
    if let Some((_, preview)) = dialog.content.as_pattern_rename() {
        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0], ("hello_world.txt".to_string(), "world_hello.txt".to_string()));
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_pattern_rename_with_substring_extraction() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add files to the active pane
    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntry {
            name: "document.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/document.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
    ];
    
    // Show pattern rename dialog
    update_state(&mut state, Transition::ShowPatternRenameDialog);
    
    // Update the pattern to extract first 3 characters
    let pattern = "*.txt -> [0:3]_backup.txt".to_string();
    update_state(&mut state, Transition::UpdatePatternRenamePattern { pattern: pattern.clone() });
    
    // Verify the preview shows the substring extraction
    let dialog = state.dialogs.current().unwrap();
    if let Some((_, preview)) = dialog.content.as_pattern_rename() {
        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0], ("document.txt".to_string(), "doc_backup.txt".to_string()));
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_pattern_rename_filters_non_matching_files() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add files with different extensions
    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntry {
            name: "file1.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/file1.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
        FileEntry {
            name: "file2.pdf".to_string(),
            location: Location::Local(PathBuf::from("/test/file2.pdf")),
            size: 200,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
    ];
    
    // Mark all files
    update_state(&mut state, Transition::MarkAll);
    
    // Show pattern rename dialog
    update_state(&mut state, Transition::ShowPatternRenameDialog);
    
    // Update the pattern to only match .txt files
    let pattern = "*.txt -> backup_[1].txt".to_string();
    update_state(&mut state, Transition::UpdatePatternRenamePattern { pattern: pattern.clone() });
    
    // Verify only .txt files are in the preview
    let dialog = state.dialogs.current().unwrap();
    if let Some((_, preview)) = dialog.content.as_pattern_rename() {
        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0], ("file1.txt".to_string(), "backup_file1.txt".to_string()));
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_pattern_rename_job_completion_refreshes_pane() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Add files to the active pane
    let pane = state.active_pane_mut();
    let file_location = Location::Local(PathBuf::from("/test/file1.txt"));
    pane.entries = vec![
        FileEntry {
            name: "file1.txt".to_string(),
            location: file_location.clone(),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        },
    ];
    
    // Execute pattern rename
    let pattern = "*.txt -> backup_[1].txt".to_string();
    let targets = vec![file_location];
    let result = update_state(&mut state, Transition::ExecutePatternRename {
        pattern: pattern.clone(),
        targets: targets.clone(),
    });
    
    // Get the job spec
    let job_spec = result.jobs_to_start[0].clone();
    let job_id = job_spec.id;
    
    // Enqueue and start the job
    update_state(&mut state, Transition::EnqueueJob { spec: job_spec.clone() });
    update_state(&mut state, Transition::StartNextJob);
    state.jobs.start_job(job_spec);
    
    // Complete the job
    let result = update_state(&mut state, Transition::CompleteJob {
        job_id,
        result: OpResult::Success(SuccessData::None),
    });
    
    // Verify pane refresh is requested
    assert_eq!(result.panes_to_refresh.len(), 1);
}
