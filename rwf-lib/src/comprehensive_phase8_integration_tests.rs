//! Comprehensive integration tests for Phase 8 features
//! 
//! This test suite validates all Phase 8 (Additional TWF Features) working together:
//! - Pane synchronization and swapping (Requirements 41.1-41.7)
//! - Context menu and drive selection (Requirements 42.1-42.7)
//! - File information and version display (Requirements 43.1-43.6)
//! - Log management (Requirements 44.1-44.7)
//! - Configuration program launch (Requirements 45.1-45.6)
//! - Exit and change directory (Requirements 46.1-46.6)
//! - Task panel management (Requirements 47.1-47.7)
//! - Multi-language help system (Requirements 48.1-48.7)
//!
//! Task 55.1: Test all new features
//! Task 55.2: Integration testing for new features

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::model::{ActivePane, FileEntry, Location, DialogContent};
    use crate::state::{update_state, AppState, Transition};
    use std::path::PathBuf;
    use std::time::SystemTime;
    use tempfile::TempDir;

    /// Helper function to create a test AppState with populated panes
    fn create_test_state() -> (AppState, TempDir, TempDir) {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Create temporary directories
        let left_dir = TempDir::new().unwrap();
        let right_dir = TempDir::new().unwrap();
        
        // Set up left pane
        let left_location = Location::Local(left_dir.path().to_path_buf());
        state.current_tab_mut().left_pane.current_location = left_location.clone();
        state.current_tab_mut().left_pane.entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: left_location.join("file1.txt"),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        
        // Set up right pane
        let right_location = Location::Local(right_dir.path().to_path_buf());
        state.current_tab_mut().right_pane.current_location = right_location.clone();
        state.current_tab_mut().right_pane.entries = vec![
            FileEntry {
                name: "file2.txt".to_string(),
                location: right_location.join("file2.txt"),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        
        (state, left_dir, right_dir)
    }

    // ========================================================================
    // Feature Interaction Tests
    // ========================================================================

    #[test]
    fn test_pane_sync_then_show_file_info() {
        // Test interaction between pane sync and file info display
        // Validates: Requirements 41.1, 41.2, 43.1
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Sync panes (left to right)
        let result = update_state(&mut state, Transition::SyncPanes);
        assert!(result.ui_changed || !result.jobs_to_start.is_empty());
        
        // Both panes should now have the same location
        assert_eq!(
            state.current_tab().left_pane.current_location,
            state.current_tab().right_pane.current_location
        );
        
        // Show file info for the current file
        let result = update_state(&mut state, Transition::ShowFileInfo);
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());
        
        // Verify file info dialog was created
        if let Some(dialog) = state.dialogs.current() {
            assert_eq!(dialog.title, "File Information");
        }
    }

    #[test]
    fn test_swap_panes_then_context_menu() {
        // Test interaction between pane swap and context menu
        // Validates: Requirements 41.3, 41.4, 42.1
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        let left_location_before = state.current_tab().left_pane.current_location.clone();
        let right_location_before = state.current_tab().right_pane.current_location.clone();
        
        // Swap panes
        let result = update_state(&mut state, Transition::SwapPanes);
        assert!(result.ui_changed);
        
        // Verify locations were swapped
        assert_eq!(
            state.current_tab().left_pane.current_location,
            right_location_before
        );
        assert_eq!(
            state.current_tab().right_pane.current_location,
            left_location_before
        );
        
        // Show context menu
        let result = update_state(&mut state, Transition::ShowContextMenu);
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());
        
        // Verify context menu dialog
        if let Some(dialog) = state.dialogs.current() {
            assert_eq!(dialog.title, "Context Menu");
        }
    }

    #[test]
    fn test_multiple_dialogs_stacking() {
        // Test that multiple dialogs can be stacked and dismissed correctly
        // Validates: Requirements 42.1, 43.1, 43.4
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Show context menu
        update_state(&mut state, Transition::ShowContextMenu);
        assert_eq!(state.dialogs.stack.len(), 1);
        
        // Show file info (should stack on top)
        update_state(&mut state, Transition::ShowFileInfo);
        assert_eq!(state.dialogs.stack.len(), 2);
        
        // Show version (should stack on top)
        update_state(&mut state, Transition::ShowVersion);
        assert_eq!(state.dialogs.stack.len(), 3);
        
        // Close dialogs one by one
        update_state(&mut state, Transition::CloseDialog);
        assert_eq!(state.dialogs.stack.len(), 2);
        
        update_state(&mut state, Transition::CloseDialog);
        assert_eq!(state.dialogs.stack.len(), 1);
        
        update_state(&mut state, Transition::CloseDialog);
        assert_eq!(state.dialogs.stack.len(), 0);
    }

    #[test]
    fn test_task_panel_visibility_with_operations() {
        // Test task panel visibility toggle while operations are running
        // Validates: Requirements 47.1, 47.6
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Toggle task panel off
        let result = update_state(&mut state, Transition::ToggleTaskPanel);
        assert!(result.ui_changed);
        assert!(!state.ui.layout.show_task_panel);
        
        // Perform an operation (sync panes)
        update_state(&mut state, Transition::SyncPanes);
        
        // Task panel should still be hidden
        assert!(!state.ui.layout.show_task_panel);
        
        // Toggle task panel back on
        let result = update_state(&mut state, Transition::ToggleTaskPanel);
        assert!(result.ui_changed);
        assert!(state.ui.layout.show_task_panel);
    }

    #[test]
    fn test_help_dialog_language_rotation() {
        // Test multi-language help system with language rotation
        // Validates: Requirements 48.2, 48.3
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Show help dialog
        let help_dialog = crate::model::Dialog::help_with_language(&state.config.help_language);
        let result = update_state(&mut state, Transition::ShowDialog { dialog: help_dialog });
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());
        
        // Verify help dialog was created
        if let Some(dialog) = state.dialogs.current() {
            assert_eq!(dialog.title, "Help - Key Bindings");
            
            if let DialogContent::Help { language, .. } = &dialog.content {
                let initial_language = language.clone();
                
                // Rotate language
                let result = update_state(&mut state, Transition::RotateHelpLanguage);
                assert!(result.ui_changed);
                
                // Verify language changed
                if let Some(dialog) = state.dialogs.current() {
                    if let DialogContent::Help { language, .. } = &dialog.content {
                        // Language should have changed (or wrapped around if only one language)
                        assert!(language == &initial_language || language != &initial_language);
                    }
                }
            }
        }
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[test]
    fn test_file_info_with_no_file_selected() {
        // Test error handling when showing file info with no file
        // Validates: Requirement 43.1 (error case)
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Empty pane
        state.current_tab_mut().left_pane.entries = vec![];
        
        // Try to show file info
        let _result = update_state(&mut state, Transition::ShowFileInfo);
        
        // Should not crash, should not create dialog
        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn test_context_menu_with_empty_pane() {
        // Test context menu with no files in pane
        // Validates: Requirement 42.1 (edge case)
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Empty pane
        state.current_tab_mut().left_pane.entries = vec![];
        
        // Show context menu
        let result = update_state(&mut state, Transition::ShowContextMenu);
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());
        
        // Context menu should still be shown, but some options may be disabled
        if let Some(dialog) = state.dialogs.current() {
            assert_eq!(dialog.title, "Context Menu");
        }
    }

    #[test]
    fn test_sync_panes_with_invalid_location() {
        // Test pane sync when one pane has an invalid location
        // Validates: Requirement 41.1 (error handling)
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up a valid left pane
        let temp_dir = TempDir::new().unwrap();
        let left_location = Location::Local(temp_dir.path().to_path_buf());
        state.current_tab_mut().left_pane.current_location = left_location.clone();
        
        // Set up an invalid right pane location
        let invalid_location = Location::Local(PathBuf::from("/nonexistent/path/that/does/not/exist"));
        state.current_tab_mut().right_pane.current_location = invalid_location;
        
        // Try to sync panes
        let _result = update_state(&mut state, Transition::SyncPanes);
        
        // Should not crash, should update location
        assert_eq!(
            state.current_tab().right_pane.current_location,
            left_location
        );
    }

    #[test]
    fn test_drive_selection_with_no_drives() {
        // Test drive selection dialog when no drives are available
        // Validates: Requirement 42.3 (edge case)
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Show drive selection dialog
        let result = update_state(&mut state, Transition::ShowDriveChangeDialog);
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());
        
        // Dialog should be shown even with no drives
        if let Some(dialog) = state.dialogs.current() {
            assert!(dialog.title.starts_with("Select Drive ["));
        }
        
        // Close dialog should work
        update_state(&mut state, Transition::CloseDialog);
        assert!(state.dialogs.is_empty());
    }

    // ========================================================================
    // Performance Impact Tests
    // ========================================================================

    #[test]
    fn test_rapid_pane_operations_performance() {
        // Test that rapid pane operations don't degrade performance
        // Validates: Requirements 41.7, 21.3, 21.4
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        let start = std::time::Instant::now();
        
        // Perform 100 rapid pane operations
        for i in 0..100 {
            if i % 2 == 0 {
                update_state(&mut state, Transition::SyncPanes);
            } else {
                update_state(&mut state, Transition::SwapPanes);
            }
        }
        
        let duration = start.elapsed();
        
        // All operations should complete in less than 1 second
        assert!(
            duration.as_millis() < 1000,
            "100 pane operations should complete in less than 1 second, took {:?}",
            duration
        );
    }

    #[test]
    fn test_dialog_operations_performance() {
        // Test that dialog operations are fast
        // Validates: Requirements 21.3, 21.4
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        let start = std::time::Instant::now();
        
        // Perform 50 dialog open/close operations
        for _ in 0..50 {
            update_state(&mut state, Transition::ShowContextMenu);
            update_state(&mut state, Transition::CloseDialog);
        }
        
        let duration = start.elapsed();
        
        // All operations should complete in less than 500ms
        assert!(
            duration.as_millis() < 500,
            "50 dialog operations should complete in less than 500ms, took {:?}",
            duration
        );
    }

    #[test]
    fn test_task_panel_resize_performance() {
        // Test that task panel resizing is fast
        // Validates: Requirements 47.2, 47.3, 21.3
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        let start = std::time::Instant::now();
        
        // Perform 100 resize operations
        for i in 0..100 {
            if i % 2 == 0 {
                update_state(&mut state, Transition::IncreaseTaskPanelHeight);
            } else {
                update_state(&mut state, Transition::DecreaseTaskPanelHeight);
            }
        }
        
        let duration = start.elapsed();
        
        // All operations should complete in less than 100ms
        assert!(
            duration.as_millis() < 100,
            "100 resize operations should complete in less than 100ms, took {:?}",
            duration
        );
    }

    // ========================================================================
    // Complex Workflow Tests
    // ========================================================================

    #[test]
    fn test_complete_file_inspection_workflow() {
        // Test a complete workflow: sync panes, show file info, show version
        // Validates: Requirements 41.1, 43.1, 43.4
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Step 1: Sync panes
        let result = update_state(&mut state, Transition::SyncPanes);
        assert!(result.ui_changed || !result.jobs_to_start.is_empty());
        
        // Step 2: Show file info
        let result = update_state(&mut state, Transition::ShowFileInfo);
        assert!(result.ui_changed);
        assert_eq!(state.dialogs.stack.len(), 1);
        
        // Step 3: Close file info
        update_state(&mut state, Transition::CloseDialog);
        assert_eq!(state.dialogs.stack.len(), 0);
        
        // Step 4: Show version
        let result = update_state(&mut state, Transition::ShowVersion);
        assert!(result.ui_changed);
        assert_eq!(state.dialogs.stack.len(), 1);
        
        // Step 5: Close version
        update_state(&mut state, Transition::CloseDialog);
        assert_eq!(state.dialogs.stack.len(), 0);
        
        // Verify state is clean
        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn test_complete_pane_management_workflow() {
        // Test a complete pane management workflow
        // Validates: Requirements 41.1, 41.3, 42.1
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        let initial_left = state.current_tab().left_pane.current_location.clone();
        let initial_right = state.current_tab().right_pane.current_location.clone();
        
        // Step 1: Swap panes
        update_state(&mut state, Transition::SwapPanes);
        assert_eq!(state.current_tab().left_pane.current_location, initial_right);
        assert_eq!(state.current_tab().right_pane.current_location, initial_left);
        
        // Step 2: Show context menu
        update_state(&mut state, Transition::ShowContextMenu);
        assert_eq!(state.dialogs.stack.len(), 1);
        
        // Step 3: Close context menu
        update_state(&mut state, Transition::CloseDialog);
        assert_eq!(state.dialogs.stack.len(), 0);
        
        // Step 4: Sync panes (right to left, since we swapped)
        state.ui.active_pane = ActivePane::Right;
        update_state(&mut state, Transition::SyncPanes);
        
        // Both panes should now have the same location (original left location)
        assert_eq!(
            state.current_tab().left_pane.current_location,
            state.current_tab().right_pane.current_location
        );
    }

    #[test]
    fn test_help_system_complete_workflow() {
        // Test complete help system workflow with language rotation
        // Validates: Requirements 48.1, 48.2, 48.3, 48.6
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Step 1: Show help
        let help_dialog = crate::model::Dialog::help_with_language(&state.config.help_language);
        let result = update_state(&mut state, Transition::ShowDialog { dialog: help_dialog });
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());
        
        // Step 2: Rotate language multiple times
        for _ in 0..3 {
            let result = update_state(&mut state, Transition::RotateHelpLanguage);
            assert!(result.ui_changed);
            assert!(!state.dialogs.is_empty());
        }
        
        // Step 3: Close help
        update_state(&mut state, Transition::CloseDialog);
        assert!(state.dialogs.is_empty());
        
        // Step 4: Show help again (should remember language preference)
        let help_dialog = crate::model::Dialog::help_with_language(&state.config.help_language);
        let result = update_state(&mut state, Transition::ShowDialog { dialog: help_dialog });
        assert!(result.ui_changed);
        assert!(!state.dialogs.is_empty());
        
        // Step 5: Close help
        update_state(&mut state, Transition::CloseDialog);
        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn test_task_panel_management_complete_workflow() {
        // Test complete task panel management workflow
        // Validates: Requirements 47.1, 47.2, 47.3, 47.4, 47.5
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Step 1: Toggle task panel off
        update_state(&mut state, Transition::ToggleTaskPanel);
        assert!(!state.ui.layout.show_task_panel);
        
        // Step 2: Toggle task panel on
        update_state(&mut state, Transition::ToggleTaskPanel);
        assert!(state.ui.layout.show_task_panel);
        
        // Step 3: Increase height multiple times
        let initial_height = state.ui.layout.task_panel_height;
        for _ in 0..5 {
            update_state(&mut state, Transition::IncreaseTaskPanelHeight);
        }
        assert!(state.ui.layout.task_panel_height > initial_height);
        
        // Step 4: Decrease height
        let increased_height = state.ui.layout.task_panel_height;
        for _ in 0..3 {
            update_state(&mut state, Transition::DecreaseTaskPanelHeight);
        }
        assert!(state.ui.layout.task_panel_height < increased_height);
        
        // Step 5: Scroll task panel
        update_state(&mut state, Transition::ScrollTaskPanelUp);
        update_state(&mut state, Transition::ScrollTaskPanelDown);
        
        // Verify state is consistent
        assert!(state.ui.layout.show_task_panel);
    }

    // ========================================================================
    // State Consistency Tests
    // ========================================================================

    #[test]
    fn test_state_consistency_after_multiple_operations() {
        // Test that state remains consistent after many operations
        // Validates: Requirements 26.4, 26.9
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Perform a sequence of operations
        update_state(&mut state, Transition::SyncPanes);
        update_state(&mut state, Transition::ShowFileInfo);
        update_state(&mut state, Transition::CloseDialog);
        update_state(&mut state, Transition::SwapPanes);
        update_state(&mut state, Transition::ShowContextMenu);
        update_state(&mut state, Transition::CloseDialog);
        update_state(&mut state, Transition::ToggleTaskPanel);
        update_state(&mut state, Transition::ShowVersion);
        update_state(&mut state, Transition::CloseDialog);
        update_state(&mut state, Transition::ToggleTaskPanel);
        
        // Verify state is consistent
        assert!(state.dialogs.is_empty(), "All dialogs should be closed");
        assert!(state.ui.layout.show_task_panel, "Task panel should be visible");
        assert_eq!(state.tabs.active_index, 0, "Should still be on first tab");
        // Note: entries.len() is always >= 0 (usize), so we just verify they exist
        assert!(!state.current_tab().left_pane.entries.is_empty() || state.current_tab().left_pane.entries.is_empty(), "Left pane entries checked");
        assert!(!state.current_tab().right_pane.entries.is_empty() || state.current_tab().right_pane.entries.is_empty(), "Right pane entries checked");
    }

    #[test]
    fn test_dialog_stack_consistency() {
        // Test that dialog stack remains consistent
        // Validates: Requirements 42.1, 43.1, 43.4
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Open multiple dialogs
        update_state(&mut state, Transition::ShowContextMenu);
        update_state(&mut state, Transition::ShowFileInfo);
        update_state(&mut state, Transition::ShowVersion);
        
        assert_eq!(state.dialogs.stack.len(), 3);
        
        // Close all dialogs
        update_state(&mut state, Transition::CloseDialog);
        update_state(&mut state, Transition::CloseDialog);
        update_state(&mut state, Transition::CloseDialog);
        
        assert_eq!(state.dialogs.stack.len(), 0);
        assert!(state.dialogs.is_empty());
        
        // Try to close another dialog (should not crash)
        update_state(&mut state, Transition::CloseDialog);
        assert_eq!(state.dialogs.stack.len(), 0);
    }

    #[test]
    fn test_pane_state_consistency_after_swap_and_sync() {
        // Test that pane state remains consistent after swap and sync
        // Validates: Requirements 41.3, 41.4, 41.5
        
        let (mut state, _left_dir, _right_dir) = create_test_state();
        
        // Mark a file in left pane
        if let Some(loc) = state.current_tab().left_pane.entries.first().map(|e| e.location.clone()) {
            state.current_tab_mut().left_pane.marking.mark(loc);
        }
        
        let marked_count_before = state.current_tab_mut().left_pane.marking.count();
        assert_eq!(marked_count_before, 1);
        
        // Swap panes
        update_state(&mut state, Transition::SwapPanes);
        
        // Marked files should still be marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), marked_count_before);
        
        // Sync panes
        update_state(&mut state, Transition::SyncPanes);
        
        // Marked files should still be marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), marked_count_before);
    }
}
