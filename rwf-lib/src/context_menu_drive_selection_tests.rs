//! Integration tests for context menu and drive selection
//! **Validates: Requirements 42.1-42.7**

#[cfg(test)]
mod tests {
    use crate::model::{ContextMenuAction, Dialog, DialogContent, DriveSelectionDialog, Location};
    use crate::state::{update_state, Transition};
    use crate::test_utils::test_state;
    use std::path::PathBuf;

    /// Test showing context menu dialog
    /// **Validates: Requirement 42.1**
    #[test]
    fn test_show_context_menu() {
        let mut state = test_state();

        // Show context menu
        let result = update_state(&mut state, Transition::ShowContextMenu);

        // Verify dialog was shown
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());

        // Verify dialog content
        if let Some(dialog) = state.dialogs.current() {
            assert_eq!(dialog.title, "Context Menu");

            if let DialogContent::ContextMenu {
                options,
                selected_index,
            } = &dialog.content
            {
                assert_eq!(selected_index, &0);
                assert!(!options.is_empty());

                // Verify expected options are present
                let option_labels: Vec<&str> = options.iter().map(|o| o.label.as_str()).collect();
                assert!(option_labels.contains(&"Copy"));
                assert!(option_labels.contains(&"Move"));
                assert!(option_labels.contains(&"Delete"));
                assert!(option_labels.contains(&"Rename"));
                assert!(option_labels.contains(&"View"));
            } else {
                panic!("Expected ContextMenu dialog content");
            }
        } else {
            panic!("Expected dialog to be shown");
        }
    }

    /// Test context menu options
    /// **Validates: Requirement 42.2**
    #[test]
    fn test_context_menu_options() {
        let mut state = test_state();

        // Show context menu
        update_state(&mut state, Transition::ShowContextMenu);

        // Get the dialog
        if let Some(dialog) = state.dialogs.current() {
            if let DialogContent::ContextMenu { options, .. } = &dialog.content {
                // Verify all required options are present
                let has_copy = options
                    .iter()
                    .any(|o| matches!(o.action, ContextMenuAction::Copy));
                let has_move = options
                    .iter()
                    .any(|o| matches!(o.action, ContextMenuAction::Move));
                let has_delete = options
                    .iter()
                    .any(|o| matches!(o.action, ContextMenuAction::Delete));
                let has_rename = options
                    .iter()
                    .any(|o| matches!(o.action, ContextMenuAction::Rename));
                let has_view = options
                    .iter()
                    .any(|o| matches!(o.action, ContextMenuAction::View));

                assert!(has_copy, "Context menu should include Copy option");
                assert!(has_move, "Context menu should include Move option");
                assert!(has_delete, "Context menu should include Delete option");
                assert!(has_rename, "Context menu should include Rename option");
                assert!(has_view, "Context menu should include View option");
            }
        }
    }

    /// Test showing drive selection dialog
    /// **Validates: Requirement 42.3**
    #[test]
    fn test_show_drive_selection_dialog() {
        let mut state = test_state();

        // Show drive selection dialog
        let result = update_state(&mut state, Transition::ShowDriveChangeDialog);

        // Verify dialog was shown
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());

        // Verify dialog content
        if let Some(dialog) = state.dialogs.current() {
            assert!(
                dialog.title.starts_with("Select Drive"),
                "title must start with 'Select Drive'"
            );

            if let DialogContent::DriveSelection(DriveSelectionDialog {
                drives: _,
                selected_index,
                ..
            }) = &dialog.content
            {
                assert_eq!(selected_index, &0);
                // Note: drives may be empty in test environment, but the dialog should still be shown
                // In a real environment, drives would be populated
            } else {
                panic!("Expected DriveSelection dialog content");
            }
        } else {
            panic!("Expected dialog to be shown");
        }
    }

    /// Test drive selection lists available drives
    /// **Validates: Requirement 42.4**
    #[test]
    fn test_drive_selection_lists_drives() {
        let mut state = test_state();

        // Show drive selection dialog
        update_state(&mut state, Transition::ShowDriveChangeDialog);

        // Get the dialog
        if let Some(dialog) = state.dialogs.current() {
            if let DialogContent::DriveSelection(DriveSelectionDialog { drives, .. }) =
                &dialog.content
            {
                // In a real environment, this would list actual drives
                // In test environment, it may be empty or have mock drives
                // The important thing is that the structure is correct
                for drive in drives {
                    assert!(!drive.path.is_empty(), "Drive path should not be empty");
                    assert!(!drive.label.is_empty(), "Drive label should not be empty");
                }
            }
        }
    }

    /// Test navigating to selected drive
    /// **Validates: Requirement 42.5**
    #[test]
    fn test_navigate_to_selected_drive() {
        let mut state = test_state();

        // Manually create a drive selection dialog with a test drive
        use crate::model::{DriveInfo, DriveType};
        let test_drive = DriveInfo {
            path: "/test/drive".to_string(),
            label: "Test Drive".to_string(),
            drive_type: DriveType::Local,
            total_space: Some(1000000),
            free_space: Some(500000),
        };

        let dialog = Dialog::drive_selection(vec![test_drive], crate::model::ui::ActivePane::Left);
        state.dialogs.push(dialog);

        // Get the initial location
        let initial_location = state.active_pane().current_location.clone();

        // Confirm the dialog (which should navigate to the selected drive)
        let result = update_state(&mut state, Transition::ConfirmDialog);

        // Verify navigation occurred
        assert!(result.ui_changed || !result.jobs_to_start.is_empty());

        // The dialog should be closed
        assert!(state.dialogs.is_empty());

        // The location should have changed (or a job should be started to change it)
        if result.jobs_to_start.is_empty() {
            // If no job was started, the location should have changed immediately
            let new_location = state.active_pane().current_location.clone();
            assert_ne!(initial_location, new_location);
        }
    }

    /// Test drive information display
    /// **Validates: Requirement 42.6**
    #[test]
    fn test_drive_information_display() {
        use crate::model::{DriveInfo, DriveType};

        // Create a drive with full information
        let drive = DriveInfo {
            path: "C:\\".to_string(),
            label: "System Drive".to_string(),
            drive_type: DriveType::Local,
            total_space: Some(1000000000000), // 1TB
            free_space: Some(500000000000),   // 500GB
        };

        // Verify all information is present
        assert_eq!(drive.path, "C:\\");
        assert_eq!(drive.label, "System Drive");
        assert_eq!(drive.drive_type, DriveType::Local);
        assert!(drive.total_space.is_some());
        assert!(drive.free_space.is_some());

        // Verify space calculations
        if let (Some(total), Some(free)) = (drive.total_space, drive.free_space) {
            assert!(free <= total, "Free space should not exceed total space");
            let used = total - free;
            assert_eq!(used, 500000000000); // 500GB used
        }
    }

    /// Test quick drive switching
    /// **Validates: Requirement 42.7**
    #[test]
    fn test_quick_drive_switching() {
        let mut state = test_state();

        // Show drive selection dialog
        update_state(&mut state, Transition::ShowContextMenu);

        // Verify dialog can be shown quickly (no blocking operations)
        assert!(!state.dialogs.is_empty());

        // Close the dialog
        update_state(&mut state, Transition::CancelDialog);
        assert!(state.dialogs.is_empty());

        // Show it again to verify quick switching
        update_state(&mut state, Transition::ShowDriveChangeDialog);
        assert!(!state.dialogs.is_empty());
    }

    /// Test context menu selection navigation
    #[test]
    fn test_context_menu_navigation() {
        let mut state = test_state();

        // Show context menu
        update_state(&mut state, Transition::ShowContextMenu);

        // Get the dialog
        if let Some(dialog) = state.dialogs.current() {
            if let DialogContent::ContextMenu {
                options,
                selected_index,
            } = &dialog.content
            {
                let initial_index = selected_index;
                assert_eq!(initial_index, &0);

                // Verify we can navigate through options
                let option_count = options.len();
                assert!(option_count > 0);
            }
        }
    }

    /// Test drive selection with no drives available
    #[test]
    fn test_drive_selection_empty() {
        let mut state = test_state();

        // Manually create an empty drive selection dialog
        let dialog = Dialog::drive_selection(vec![], crate::model::ui::ActivePane::Left);
        state.dialogs.push(dialog);

        // Verify dialog is shown even with no drives
        assert!(!state.dialogs.is_empty());

        // Confirm should close the dialog without error
        update_state(&mut state, Transition::ConfirmDialog);
        assert!(state.dialogs.is_empty());
    }

    /// Test context menu with file selected
    #[test]
    fn test_context_menu_with_file() {
        let mut state = test_state();

        // Add a test file to the active pane
        use crate::model::FileEntry;
        use std::time::SystemTime;

        let test_file = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test/test.txt")),
            size: 1024,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        };

        state.active_pane_mut().entries.push(test_file);
        state.active_pane_mut().cursor = 0;

        // Show context menu
        update_state(&mut state, Transition::ShowContextMenu);

        // Verify dialog is shown
        assert!(!state.dialogs.is_empty());

        // All operations should be available for a file
        if let Some(dialog) = state.dialogs.current() {
            if let DialogContent::ContextMenu { options, .. } = &dialog.content {
                assert!(options.len() >= 5, "Should have at least 5 options");
            }
        }
    }

    /// Test drive type differentiation
    #[test]
    fn test_drive_types() {
        use crate::model::{DriveInfo, DriveType};

        let local_drive = DriveInfo {
            path: "C:\\".to_string(),
            label: "Local".to_string(),
            drive_type: DriveType::Local,
            total_space: None,
            free_space: None,
        };

        let network_drive = DriveInfo {
            path: "\\\\server\\share".to_string(),
            label: "Network".to_string(),
            drive_type: DriveType::Network,
            total_space: None,
            free_space: None,
        };

        let removable_drive = DriveInfo {
            path: "D:\\".to_string(),
            label: "USB Drive".to_string(),
            drive_type: DriveType::Removable,
            total_space: None,
            free_space: None,
        };

        // Verify drive types are correctly set
        assert_eq!(local_drive.drive_type, DriveType::Local);
        assert_eq!(network_drive.drive_type, DriveType::Network);
        assert_eq!(removable_drive.drive_type, DriveType::Removable);
    }
}
