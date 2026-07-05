//! Integration tests for file filtering functionality
//!
//! Tests Requirements 13.1-13.6:
//! - File mask dialog display
//! - Wildcard pattern filtering
//! - Separate masks per pane
//! - Filter clearing
//! - Status bar display

#[cfg(test)]
mod tests {
    use crate::model::{ActivePane, FileEntry, Location};
    use crate::state::{update_state, AppConfig, AppState, Transition};
    use std::path::PathBuf;
    use std::time::SystemTime;

    /// Helper to create a test file entry
    fn create_test_entry(name: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(format!("/test/{}", name))),
            size: 100,
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        }
    }

    #[test]
    fn test_set_file_mask() {
        // Requirement 13.1: Display filter input dialog
        // Requirement 13.2: Apply file mask pattern
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Set file mask
        let result = update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Left,
                mask: Some("*.txt".to_string()),
            },
        );

        assert!(result.ui_changed);
        assert_eq!(
            state.current_tab().left_pane.file_mask,
            Some("*.txt".to_string())
        );
    }

    #[test]
    fn test_clear_file_mask() {
        // Requirement 13.5: Clear filter when mask is empty
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Set file mask first
        update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Left,
                mask: Some("*.txt".to_string()),
            },
        );

        // Clear file mask
        let result = update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Left,
                mask: None,
            },
        );

        assert!(result.ui_changed);
        assert!(state.current_tab().left_pane.file_mask.is_none());
    }

    #[test]
    fn test_separate_masks_per_pane() {
        // Requirement 13.4: Maintain separate File_Mask settings for each pane
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Set different masks for each pane
        update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Left,
                mask: Some("*.txt".to_string()),
            },
        );

        update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Right,
                mask: Some("*.rs".to_string()),
            },
        );

        // Verify each pane has its own mask
        assert_eq!(
            state.current_tab().left_pane.file_mask,
            Some("*.txt".to_string())
        );
        assert_eq!(
            state.current_tab().right_pane.file_mask,
            Some("*.rs".to_string())
        );
    }

    #[test]
    fn test_wildcard_filtering_star() {
        // Requirement 13.3: Support wildcard patterns (* and ?)
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            create_test_entry("file1.txt", false),
            create_test_entry("file2.rs", false),
            create_test_entry("file3.txt", false),
            create_test_entry("document.md", false),
            create_test_entry("dir1", true),
        ];

        state.current_tab_mut().left_pane.entries = entries;

        // Apply filter using the pane's apply_filter method
        state.current_tab_mut().left_pane.apply_filter("*.txt");

        // Should keep directories and .txt files
        let filtered = &state.current_tab().left_pane.entries;
        assert_eq!(filtered.len(), 3); // 2 .txt files + 1 directory

        // Verify the correct files are kept
        assert!(filtered.iter().any(|e| e.name == "file1.txt"));
        assert!(filtered.iter().any(|e| e.name == "file3.txt"));
        assert!(filtered.iter().any(|e| e.name == "dir1"));
        assert!(!filtered.iter().any(|e| e.name == "file2.rs"));
        assert!(!filtered.iter().any(|e| e.name == "document.md"));
    }

    #[test]
    fn test_wildcard_filtering_question_mark() {
        // Requirement 13.3: Support wildcard patterns (* and ?)
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            create_test_entry("a.txt", false),
            create_test_entry("ab.txt", false),
            create_test_entry("abc.txt", false),
            create_test_entry("test.txt", false),
            create_test_entry("dir1", true),
        ];

        state.current_tab_mut().left_pane.entries = entries;

        // Apply filter for single character followed by .txt
        state.current_tab_mut().left_pane.apply_filter("?.txt");

        // Should keep directories and single-char .txt files
        let filtered = &state.current_tab().left_pane.entries;
        assert_eq!(filtered.len(), 2); // 1 matching file + 1 directory

        assert!(filtered.iter().any(|e| e.name == "a.txt"));
        assert!(filtered.iter().any(|e| e.name == "dir1"));
        assert!(!filtered.iter().any(|e| e.name == "ab.txt"));
        assert!(!filtered.iter().any(|e| e.name == "abc.txt"));
    }

    #[test]
    fn test_wildcard_filtering_combined() {
        // Requirement 13.3: Support wildcard patterns (* and ?)
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            create_test_entry("test_1.txt", false),
            create_test_entry("test_12.txt", false),
            create_test_entry("test_a.txt", false),
            create_test_entry("file_1.txt", false),
            create_test_entry("dir1", true),
        ];

        state.current_tab_mut().left_pane.entries = entries;

        // Apply filter: test_?.txt (test_ followed by single char and .txt)
        state.current_tab_mut().left_pane.apply_filter("test_?.txt");

        // Should keep directories and matching files
        let filtered = &state.current_tab().left_pane.entries;
        assert_eq!(filtered.len(), 3); // 2 matching files + 1 directory

        assert!(filtered.iter().any(|e| e.name == "test_1.txt"));
        assert!(filtered.iter().any(|e| e.name == "test_a.txt"));
        assert!(filtered.iter().any(|e| e.name == "dir1"));
        assert!(!filtered.iter().any(|e| e.name == "test_12.txt"));
        assert!(!filtered.iter().any(|e| e.name == "file_1.txt"));
    }

    #[test]
    fn test_filter_always_shows_directories() {
        // Requirement 13.2: Directories should always be visible regardless of filter
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            create_test_entry("file1.txt", false),
            create_test_entry("file2.rs", false),
            create_test_entry("documents", true),
            create_test_entry("projects", true),
        ];

        state.current_tab_mut().left_pane.entries = entries;

        // Apply filter that doesn't match directory names
        state.current_tab_mut().left_pane.apply_filter("*.txt");

        // Should keep all directories and .txt files
        let filtered = &state.current_tab().left_pane.entries;
        assert_eq!(filtered.len(), 3); // 1 .txt file + 2 directories

        assert!(filtered.iter().any(|e| e.name == "file1.txt"));
        assert!(filtered.iter().any(|e| e.name == "documents"));
        assert!(filtered.iter().any(|e| e.name == "projects"));
        assert!(!filtered.iter().any(|e| e.name == "file2.rs"));
    }

    #[test]
    fn test_get_filtered_entries_with_mask() {
        // Test the get_filtered_entries method
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            create_test_entry("file1.txt", false),
            create_test_entry("file2.rs", false),
            create_test_entry("file3.txt", false),
            create_test_entry("dir1", true),
        ];

        state.current_tab_mut().left_pane.entries = entries;
        state.current_tab_mut().left_pane.file_mask = Some("*.txt".to_string());

        // Get filtered entries without modifying the original list
        let filtered = state.current_tab().left_pane.get_filtered_entries();

        // Should return 3 entries (2 .txt files + 1 directory)
        assert_eq!(filtered.len(), 3);
        assert!(filtered.iter().any(|e| e.name == "file1.txt"));
        assert!(filtered.iter().any(|e| e.name == "file3.txt"));
        assert!(filtered.iter().any(|e| e.name == "dir1"));

        // Original entries should be unchanged
        assert_eq!(state.current_tab().left_pane.entries.len(), 4);
    }

    #[test]
    fn test_get_filtered_entries_without_mask() {
        // Test that get_filtered_entries returns all entries when no mask is set
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            create_test_entry("file1.txt", false),
            create_test_entry("file2.rs", false),
            create_test_entry("dir1", true),
        ];

        state.current_tab_mut().left_pane.entries = entries;
        // No file mask set

        let filtered = state.current_tab().left_pane.get_filtered_entries();

        // Should return all entries
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_apply_current_filter() {
        // Test the apply_current_filter method
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            create_test_entry("file1.txt", false),
            create_test_entry("file2.rs", false),
            create_test_entry("file3.txt", false),
            create_test_entry("dir1", true),
        ];

        state.current_tab_mut().left_pane.entries = entries;
        state.current_tab_mut().left_pane.file_mask = Some("*.txt".to_string());

        // Apply the current filter
        state.current_tab_mut().left_pane.apply_current_filter();

        // Entries should be filtered in place
        assert_eq!(state.current_tab().left_pane.entries.len(), 3);
        assert!(state
            .current_tab()
            .left_pane
            .entries
            .iter()
            .any(|e| e.name == "file1.txt"));
        assert!(state
            .current_tab()
            .left_pane
            .entries
            .iter()
            .any(|e| e.name == "file3.txt"));
        assert!(state
            .current_tab()
            .left_pane
            .entries
            .iter()
            .any(|e| e.name == "dir1"));
    }

    #[test]
    fn test_filter_with_special_regex_characters() {
        // Test that special regex characters are properly escaped
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries with special characters
        let entries = vec![
            create_test_entry("file.txt", false),
            create_test_entry("file+txt", false),
            create_test_entry("file[1].txt", false),
            create_test_entry("dir1", true),
        ];

        state.current_tab_mut().left_pane.entries = entries;

        // Apply filter with literal dot
        state.current_tab_mut().left_pane.apply_filter("file.txt");

        // Should only match exact filename
        let filtered = &state.current_tab().left_pane.entries;
        assert_eq!(filtered.len(), 2); // 1 matching file + 1 directory
        assert!(filtered.iter().any(|e| e.name == "file.txt"));
        assert!(filtered.iter().any(|e| e.name == "dir1"));
    }

    #[test]
    fn test_filter_empty_pattern() {
        // Test that empty pattern doesn't filter anything
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            create_test_entry("file1.txt", false),
            create_test_entry("file2.rs", false),
            create_test_entry("dir1", true),
        ];

        state.current_tab_mut().left_pane.entries = entries.clone();

        // Apply empty filter
        state.current_tab_mut().left_pane.apply_filter("");

        // All entries should remain
        assert_eq!(state.current_tab().left_pane.entries.len(), 3);
    }

    #[test]
    fn test_filter_persists_across_pane_switch() {
        // Requirement 13.4: Maintain separate File_Mask settings for each pane
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Set filter on left pane
        update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Left,
                mask: Some("*.txt".to_string()),
            },
        );

        // Switch to right pane
        update_state(&mut state, Transition::SwitchPane);

        // Set different filter on right pane
        update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Right,
                mask: Some("*.rs".to_string()),
            },
        );

        // Switch back to left pane
        update_state(&mut state, Transition::SwitchPane);

        // Verify left pane still has its original filter
        assert_eq!(
            state.current_tab().left_pane.file_mask,
            Some("*.txt".to_string())
        );
        assert_eq!(
            state.current_tab().right_pane.file_mask,
            Some("*.rs".to_string())
        );
    }

    #[test]
    fn test_filter_dialog_workflow() {
        // Requirement 13.1: Display filter input dialog
        // Requirement 13.6: Display active File_Mask in Status_Bar
        use crate::model::Dialog;

        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Show filter dialog
        let dialog = Dialog::input(
            "File Mask Filter",
            "Enter file mask pattern (* and ? wildcards):",
            "",
        );

        update_state(&mut state, Transition::ShowDialog { dialog });

        // Verify dialog is shown
        assert!(!state.dialogs.is_empty());
        assert_eq!(state.dialogs.current().unwrap().title, "File Mask Filter");

        // Simulate user input
        update_state(
            &mut state,
            Transition::UpdateDialogInput {
                input: "*.txt".to_string(),
            },
        );

        // Confirm dialog (this should set the file mask)
        update_state(&mut state, Transition::ConfirmDialog);

        // Verify file mask is set
        assert_eq!(
            state.current_tab().left_pane.file_mask,
            Some("*.txt".to_string())
        );

        // Verify dialog is closed
        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn test_clear_filter_dialog_workflow() {
        // Requirement 13.5: Clear filter when mask is empty
        use crate::model::Dialog;

        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Set a filter first
        update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Left,
                mask: Some("*.txt".to_string()),
            },
        );

        // Show filter dialog with existing mask
        let dialog = Dialog::input(
            "File Mask Filter",
            "Enter file mask pattern (* and ? wildcards):",
            "*.txt",
        );

        update_state(&mut state, Transition::ShowDialog { dialog });

        // Clear the input
        update_state(
            &mut state,
            Transition::UpdateDialogInput {
                input: String::new(),
            },
        );

        // Confirm dialog
        update_state(&mut state, Transition::ConfirmDialog);

        // Verify file mask is cleared
        assert!(state.current_tab().left_pane.file_mask.is_none());
    }
}
