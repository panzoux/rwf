//! Integration tests for registered folders
//!
//! Tests:
//! - Folder registration
//! - Navigation to registered folder
//! - Environment variable expansion
//! - Moving marked files to registered folder

#[cfg(test)]
mod tests {
    use crate::state::{AppState, update_state, Transition, AppConfig};
    use crate::model::{Location, RegisteredFolder, FileEntry};
    use std::path::PathBuf;
    use std::time::SystemTime;
    use std::env;

    #[test]
    fn test_register_current_folder() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Set current location
        let location = Location::Local(PathBuf::from("/test/folder"));
        state.current_tab_mut().left_pane.current_location = location.clone();
        
        // Register the folder
        let result = update_state(&mut state, Transition::RegisterCurrentFolder {
            name: "Test Folder".to_string(),
        });
        
        assert!(result.ui_changed);
        assert_eq!(state.registered_folders.folders.len(), 1);
        assert_eq!(state.registered_folders.folders[0].name, "Test Folder");
        assert_eq!(state.registered_folders.folders[0].path, "/test/folder");
    }

    #[test]
    fn test_show_registered_folder_dialog() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders and add test folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        state.registered_folders.add(RegisteredFolder::new("Home", "/home/user"));
        state.registered_folders.add(RegisteredFolder::new("Work", "/work/project"));
        
        // Show the dialog
        let result = update_state(&mut state, Transition::ShowRegisteredFolderDialog);
        
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());
        
        // Verify dialog content
        if let Some(dialog) = state.dialogs.current() {
            match &dialog.content {
                crate::model::DialogContent::RegisteredFolderSelector { folders, filter, selected_index } => {
                    assert_eq!(folders.len(), 2);
                    assert_eq!(filter, "");
                    assert_eq!(*selected_index, 0);
                }
                _ => panic!("Expected RegisteredFolderSelector dialog"),
            }
        } else {
            panic!("Expected dialog to be shown");
        }
    }

    #[test]
    fn test_navigate_to_registered_folder() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Add a registered folder
        state.registered_folders.add(RegisteredFolder::new("Test", "/test/path"));
        
        // Navigate to the folder
        let result = update_state(&mut state, Transition::NavigateToRegisteredFolder {
            folder_index: 0,
        });
        
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify the location changed
        let expected_location = Location::Local(PathBuf::from("/test/path"));
        assert_eq!(state.current_tab().left_pane.current_location, expected_location);
    }

    #[test]
    fn test_navigate_to_invalid_folder_index() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Try to navigate to a non-existent folder
        let result = update_state(&mut state, Transition::NavigateToRegisteredFolder {
            folder_index: 99,
        });
        
        assert!(!result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 0);
    }

    #[test]
    fn test_environment_variable_expansion_in_navigation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Set an environment variable
        env::set_var("TEST_HOME", "/home/testuser");
        
        // Add a registered folder with environment variable
        state.registered_folders.add(RegisteredFolder::new("Home", "$TEST_HOME/documents"));
        
        // Navigate to the folder
        let result = update_state(&mut state, Transition::NavigateToRegisteredFolder {
            folder_index: 0,
        });
        
        assert!(result.ui_changed);
        
        // Verify the environment variable was expanded
        let expected_location = Location::Local(PathBuf::from("/home/testuser/documents"));
        assert_eq!(state.current_tab().left_pane.current_location, expected_location);
        
        // Clean up
        env::remove_var("TEST_HOME");
    }

    #[test]
    fn test_environment_variable_expansion_unix_braces() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Set an environment variable with unique name for this test
        env::set_var("TEST_VAR_INTEGRATION_UNIX_BRACES", "value");
        
        // Add a registered folder with braced environment variable
        state.registered_folders.add(RegisteredFolder::new("Test", "${TEST_VAR_INTEGRATION_UNIX_BRACES}/path"));
        
        // Navigate to the folder
        let result = update_state(&mut state, Transition::NavigateToRegisteredFolder {
            folder_index: 0,
        });
        
        assert!(result.ui_changed);
        
        // Verify the environment variable was expanded
        let expected_location = Location::Local(PathBuf::from("value/path"));
        assert_eq!(state.current_tab().left_pane.current_location, expected_location);
        
        // Clean up
        env::remove_var("TEST_VAR_INTEGRATION_UNIX_BRACES");
    }

    #[test]
    fn test_environment_variable_expansion_powershell() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Set an environment variable with unique name for this test
        env::set_var("TEST_VAR_INTEGRATION_POWERSHELL", "psvalue");
        
        // Add a registered folder with PowerShell-style environment variable
        state.registered_folders.add(RegisteredFolder::new("Test", "$env:TEST_VAR_INTEGRATION_POWERSHELL/path"));
        
        // Navigate to the folder
        let result = update_state(&mut state, Transition::NavigateToRegisteredFolder {
            folder_index: 0,
        });
        
        assert!(result.ui_changed);
        
        // Verify the environment variable was expanded
        let expected_location = Location::Local(PathBuf::from("psvalue/path"));
        assert_eq!(state.current_tab().left_pane.current_location, expected_location);
        
        // Clean up
        env::remove_var("TEST_VAR_INTEGRATION_POWERSHELL");
    }

    #[test]
    fn test_move_marked_files_to_registered_folder() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Add some files to the active pane
        let entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Local(PathBuf::from("/source/file1.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "file2.txt".to_string(),
                location: Location::Local(PathBuf::from("/source/file2.txt")),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        
        state.current_tab_mut().left_pane.entries = entries;
        
        // Mark the files
        state.marking.mark(Location::Local(PathBuf::from("/source/file1.txt")));
        state.marking.mark(Location::Local(PathBuf::from("/source/file2.txt")));
        
        // Add a registered folder
        state.registered_folders.add(RegisteredFolder::new("Destination", "/dest/folder"));
        
        // Move marked files to the registered folder
        let result = update_state(&mut state, Transition::MoveToRegisteredFolder {
            folder_index: 0,
        });
        
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify a move job was created
        if let Some(job_spec) = result.jobs_to_start.first() {
            match &job_spec.kind {
                crate::job::JobKind::Move { sources, dest } => {
                    assert_eq!(sources.len(), 2);
                    assert_eq!(*dest, Location::Local(PathBuf::from("/dest/folder")));
                }
                _ => panic!("Expected Move job"),
            }
        } else {
            panic!("Expected a job to be created");
        }
    }

    #[test]
    fn test_move_to_registered_folder_with_no_marked_files() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Add a registered folder
        state.registered_folders.add(RegisteredFolder::new("Destination", "/dest/folder"));
        
        // Try to move with no marked files
        let result = update_state(&mut state, Transition::MoveToRegisteredFolder {
            folder_index: 0,
        });
        
        assert!(!result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 0);
    }

    #[test]
    fn test_move_to_invalid_registered_folder() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Mark a file
        state.marking.mark(Location::Local(PathBuf::from("/source/file.txt")));
        
        // Try to move to a non-existent folder
        let result = update_state(&mut state, Transition::MoveToRegisteredFolder {
            folder_index: 99,
        });
        
        assert!(!result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 0);
    }

    #[test]
    fn test_multiple_environment_variables_in_path() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Set multiple environment variables
        env::set_var("VAR1", "first");
        env::set_var("VAR2", "second");
        
        // Add a registered folder with multiple environment variables
        state.registered_folders.add(RegisteredFolder::new("Test", "$VAR1/${VAR2}/path"));
        
        // Navigate to the folder
        let result = update_state(&mut state, Transition::NavigateToRegisteredFolder {
            folder_index: 0,
        });
        
        assert!(result.ui_changed);
        
        // Verify both environment variables were expanded
        let expected_location = Location::Local(PathBuf::from("first/second/path"));
        assert_eq!(state.current_tab().left_pane.current_location, expected_location);
        
        // Clean up
        env::remove_var("VAR1");
        env::remove_var("VAR2");
    }

    #[test]
    fn test_nonexistent_environment_variable_unchanged() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Ensure the variable doesn't exist
        env::remove_var("NONEXISTENT_VAR");
        
        // Add a registered folder with a non-existent environment variable
        state.registered_folders.add(RegisteredFolder::new("Test", "$NONEXISTENT_VAR/path"));
        
        // Navigate to the folder
        let result = update_state(&mut state, Transition::NavigateToRegisteredFolder {
            folder_index: 0,
        });
        
        assert!(result.ui_changed);
        
        // The path should contain the unexpanded variable or be left as-is
        let location = &state.current_tab().left_pane.current_location;
        match location {
            Location::Local(path) => {
                let path_str = path.to_string_lossy();
                // The variable should either remain unexpanded or the path should be as-is
                assert!(path_str.contains("NONEXISTENT_VAR") || path_str == "$NONEXISTENT_VAR/path");
            }
            _ => panic!("Expected Local location"),
        }
    }

    #[test]
    fn test_register_folder_dialog_confirmation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Set current location
        let location = Location::Local(PathBuf::from("/test/folder"));
        state.current_tab_mut().left_pane.current_location = location.clone();
        
        // Show the register folder dialog
        state.dialogs.push(crate::model::Dialog::input(
            "Register Folder",
            "Enter folder name:",
            "",
        ));
        
        // Set the input buffer
        state.dialogs.input_buffer = "My Folder".to_string();
        
        // Confirm the dialog
        let result = update_state(&mut state, Transition::ConfirmDialog);
        
        assert!(result.ui_changed);
        assert_eq!(state.registered_folders.folders.len(), 1);
        assert_eq!(state.registered_folders.folders[0].name, "My Folder");
        assert_eq!(state.registered_folders.folders[0].path, "/test/folder");
    }

    #[test]
    fn test_registered_folder_selector_dialog_confirmation_for_navigation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Add registered folders
        state.registered_folders.add(RegisteredFolder::new("Folder1", "/path1"));
        state.registered_folders.add(RegisteredFolder::new("Folder2", "/path2"));
        
        // Show the selector dialog
        let folders = state.registered_folders.folders.clone();
        state.dialogs.push(crate::model::Dialog::registered_folder_selector(folders));
        
        // Confirm the dialog (should navigate to the first folder)
        let result = update_state(&mut state, Transition::ConfirmDialog);
        
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify navigation to the first folder
        let expected_location = Location::Local(PathBuf::from("/path1"));
        assert_eq!(state.current_tab().left_pane.current_location, expected_location);
    }

    #[test]
    fn test_registered_folder_selector_dialog_confirmation_for_move() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Clear any pre-loaded folders
        state.registered_folders = crate::model::RegisteredFolderManager::new();
        
        // Add a file entry to the pane
        let file_entry = FileEntry {
            name: "file.txt".to_string(),
            location: Location::Local(PathBuf::from("/source/file.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        state.current_tab_mut().left_pane.entries = vec![file_entry];
        
        // Mark the file
        state.marking.mark(Location::Local(PathBuf::from("/source/file.txt")));
        
        // Add registered folders
        state.registered_folders.add(RegisteredFolder::new("Destination", "/dest"));
        
        // Show the selector dialog
        let folders = state.registered_folders.folders.clone();
        state.dialogs.push(crate::model::Dialog::registered_folder_selector(folders));
        
        // Confirm the dialog (should create a move job)
        let result = update_state(&mut state, Transition::ConfirmDialog);
        
        assert!(result.ui_changed);
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify a move job was created
        if let Some(job_spec) = result.jobs_to_start.first() {
            match &job_spec.kind {
                crate::job::JobKind::Move { sources, dest } => {
                    assert_eq!(sources.len(), 1);
                    assert_eq!(*dest, Location::Local(PathBuf::from("/dest")));
                }
                _ => panic!("Expected Move job"),
            }
        }
    }
}
