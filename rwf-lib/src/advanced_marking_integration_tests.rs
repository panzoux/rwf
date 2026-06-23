//! Integration tests for advanced marking operations (Requirement 36)

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action};
    use crate::model::{Location, FileEntry, DialogContent};
    use crate::state::{AppState, AppConfig, Transition, update_state};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_entries(names: Vec<&str>) -> Vec<FileEntry> {
        names
            .iter()
            .map(|name| FileEntry {
                name: name.to_string(),
                location: Location::Local(PathBuf::from(format!("/test/{}", name))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
            })
            .collect()
    }

    #[test]
    fn test_wildcard_marking_dialog_display() {
        // Requirement 36.1: WHEN the user presses '@', THE Application SHALL display a wildcard marking Dialog
        let config = AppConfig::default();
        let state = AppState::new(config);
        
        // Execute WildcardMarking action
        let transitions = action_to_transitions(&state, &Action::WildcardMarking);
        assert_eq!(transitions.len(), 1);
        
        // Verify dialog is shown
        match &transitions[0] {
            Transition::ShowDialog { dialog } => {
                assert_eq!(dialog.title, "Wildcard Marking");
                assert!(matches!(dialog.content, DialogContent::WildcardMark { .. }));
            }
            _ => panic!("Expected ShowDialog transition"),
        }
    }

    #[test]
    fn test_wildcard_patterns_star_and_question() {
        // Requirement 36.2: THE Application SHALL support wildcard patterns (* and ?) in the marking Dialog
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec![
            "test.txt",
            "test.rs",
            "file1.txt",
            "file2.txt",
            "file10.txt",
            "data.json",
        ]);
        state.current_tab_mut().left_pane.entries = entries;
        
        // Test * wildcard - mark all .txt files
        update_state(&mut state, Transition::MarkPattern { pattern: "*.txt".to_string() });
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 4); // test.txt, file1.txt, file2.txt, file10.txt
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/test.txt"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file1.txt"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file2.txt"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file10.txt"))));
        
        // Clear marks
        state.current_tab_mut().left_pane.marking.unmark_all();
        
        // Test ? wildcard - mark files with single character after "file"
        update_state(&mut state, Transition::MarkPattern { pattern: "file?.txt".to_string() });
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file1.txt"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file2.txt"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/file10.txt"))));
    }

    #[test]
    fn test_wildcard_pattern_execution() {
        // Requirement 36.3: WHEN the user submits a pattern, THE Application SHALL mark all files matching the pattern
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec![
            "document.pdf",
            "report.pdf",
            "image.png",
            "photo.jpg",
            "data.csv",
        ]);
        state.current_tab_mut().left_pane.entries = entries;
        
        // Mark all PDF files
        update_state(&mut state, Transition::MarkPattern { pattern: "*.pdf".to_string() });
        
        // Verify only PDF files are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/document.pdf"))));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/report.pdf"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/image.png"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/photo.jpg"))));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&Location::Local(PathBuf::from("/test/data.csv"))));
    }

    #[test]
    fn test_range_marking_mode_entry() {
        // Requirement 36.4: WHEN the user presses Ctrl+Space, THE Application SHALL enter range marking mode
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec!["file1.txt", "file2.txt", "file3.txt", "file4.txt", "file5.txt"]);
        state.current_tab_mut().left_pane.entries = entries;
        state.current_tab_mut().left_pane.cursor = 2;
        
        // Execute RangeMarking action (first time - enter mode)
        let transitions = action_to_transitions(&state, &Action::RangeMarking);
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::EnterRangeMarkingMode));
        
        // Apply transition
        update_state(&mut state, transitions[0].clone());
        
        // Verify range marking mode is active with cursor position stored
        assert_eq!(state.ui.range_marking_start, Some(2));
    }

    #[test]
    fn test_range_marking_execution() {
        // Requirement 36.5: WHEN in range marking mode, THE Application SHALL mark all files between 
        // the initial Cursor position and the current Cursor position
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec![
            "file0.txt", "file1.txt", "file2.txt", "file3.txt", "file4.txt",
            "file5.txt", "file6.txt", "file7.txt", "file8.txt", "file9.txt",
        ]);
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Enter range marking mode at position 2
        state.ui.range_marking_start = Some(2);
        state.current_tab_mut().left_pane.cursor = 2;
        
        // Move cursor to position 6
        state.current_tab_mut().left_pane.cursor = 6;
        
        // Execute RangeMarking action (second time - complete range)
        let transitions = action_to_transitions(&state, &Action::RangeMarking);
        assert_eq!(transitions.len(), 1);
        
        // Apply transition
        update_state(&mut state, transitions[0].clone());
        
        // Verify files in range [2, 6] are marked (inclusive)
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 5); // files 2, 3, 4, 5, 6
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[3].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[4].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[5].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[6].location));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[1].location));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[7].location));
        
        // Verify range marking mode is exited
        assert_eq!(state.ui.range_marking_start, None);
    }

    #[test]
    fn test_range_marking_reverse_order() {
        // Test that range marking works when cursor moves backward
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec![
            "file0.txt", "file1.txt", "file2.txt", "file3.txt", "file4.txt",
            "file5.txt", "file6.txt", "file7.txt",
        ]);
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Enter range marking mode at position 6
        state.ui.range_marking_start = Some(6);
        state.current_tab_mut().left_pane.cursor = 6;
        
        // Move cursor backward to position 3
        state.current_tab_mut().left_pane.cursor = 3;
        
        // Execute RangeMarking action
        let transitions = action_to_transitions(&state, &Action::RangeMarking);
        update_state(&mut state, transitions[0].clone());
        
        // Verify files in range [3, 6] are marked (should handle reverse order)
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 4); // files 3, 4, 5, 6
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[3].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[4].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[5].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[6].location));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[7].location));
    }

    #[test]
    fn test_invert_marks() {
        // Requirement 36.6: WHEN the user presses Home (with Shift or in marking mode), 
        // THE Application SHALL invert all marks in the Active_Pane
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec![
            "file1.txt", "file2.txt", "file3.txt", "file4.txt", "file5.txt",
        ]);
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Mark some files (1 and 3)
        state.current_tab_mut().left_pane.marking.mark(entries[0].location.clone());
        state.current_tab_mut().left_pane.marking.mark(entries[2].location.clone());
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        
        // Execute InvertMarks action
        let transitions = action_to_transitions(&state, &Action::InvertMarks);
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::InvertMarks));
        
        // Apply transition
        update_state(&mut state, transitions[0].clone());
        
        // Verify marks are inverted
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 3);
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[0].location)); // was marked, now unmarked
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[1].location));  // was unmarked, now marked
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location)); // was marked, now unmarked
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[3].location));  // was unmarked, now marked
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[4].location));  // was unmarked, now marked
    }

    #[test]
    fn test_marking_persistence_across_navigation() {
        // Requirement 36.7: THE Application SHALL maintain marked file state across directory navigation
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec!["file1.txt", "file2.txt", "file3.txt"]);
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Mark some files
        state.current_tab_mut().left_pane.marking.mark(entries[0].location.clone());
        state.current_tab_mut().left_pane.marking.mark(entries[2].location.clone());
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        
        // Navigate to a different directory
        let new_location = Location::Local(PathBuf::from("/other"));
        update_state(&mut state, Transition::ChangeLocation {
            pane: crate::model::ActivePane::Left,
            location: new_location,
        });
        
        // Verify marks are still present
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[0].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location));
        
        // Navigate back
        let original_location = Location::Local(PathBuf::from("/test"));
        update_state(&mut state, Transition::ChangeLocation {
            pane: crate::model::ActivePane::Left,
            location: original_location,
        });
        
        // Verify marks are still present
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[0].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location));
    }

    #[test]
    fn test_marked_file_count_and_size_display() {
        // Requirement 36.8: THE Application SHALL display marked file count and total size in the Status_Bar
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file1.txt")),
                size: 1024,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
            },
            FileEntry {
                name: "file2.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file2.txt")),
                size: 2048,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
            },
            FileEntry {
                name: "file3.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file3.txt")),
                size: 512,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
            },
        ];
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Mark two files
        state.current_tab_mut().left_pane.marking.mark(entries[0].location.clone());
        state.current_tab_mut().left_pane.marking.mark(entries[1].location.clone());
        
        // Verify count
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        
        // Verify total size calculation
        let total_size = state.current_tab_mut().left_pane.marking.total_size(&entries);
        assert_eq!(total_size, 1024 + 2048); // 3072 bytes
    }

    #[test]
    fn test_wildcard_marking_complex_patterns() {
        // Test complex wildcard patterns
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec![
            "test_file_1.txt",
            "test_file_2.txt",
            "test_doc_1.txt",
            "other_file_1.txt",
            "file.txt",
        ]);
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Mark files matching "test_*_*.txt"
        update_state(&mut state, Transition::MarkPattern { pattern: "test_*_*.txt".to_string() });
        
        // Verify only test_* files are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 3);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[0].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[1].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[3].location));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[4].location));
    }

    #[test]
    fn test_wildcard_marking_case_insensitive() {
        // Test that wildcard marking is case-insensitive
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec![
            "Test.TXT",
            "test.txt",
            "TEST.txt",
            "file.rs",
        ]);
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Mark files matching "*.txt" (lowercase pattern)
        update_state(&mut state, Transition::MarkPattern { pattern: "*.txt".to_string() });
        
        // Verify all .txt files are marked regardless of case
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 3);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[0].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[1].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location));
        assert!(!state.current_tab_mut().left_pane.marking.is_marked(&entries[3].location));
    }

    #[test]
    fn test_range_marking_single_file() {
        // Test range marking when start and end are the same
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec!["file1.txt", "file2.txt", "file3.txt"]);
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Enter range marking mode at position 1
        state.ui.range_marking_start = Some(1);
        state.current_tab_mut().left_pane.cursor = 1;
        
        // Execute RangeMarking action (cursor hasn't moved)
        let transitions = action_to_transitions(&state, &Action::RangeMarking);
        update_state(&mut state, transitions[0].clone());
        
        // Verify only one file is marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 1);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[1].location));
    }

    #[test]
    fn test_wildcard_marking_no_matches() {
        // Test wildcard marking when no files match
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec!["file1.txt", "file2.txt", "file3.txt"]);
        state.current_tab_mut().left_pane.entries = entries;
        
        // Mark files matching "*.pdf" (no matches)
        update_state(&mut state, Transition::MarkPattern { pattern: "*.pdf".to_string() });
        
        // Verify no files are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 0);
    }

    #[test]
    fn test_wildcard_marking_all_files() {
        // Test wildcard marking with "*" pattern (all files)
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = create_test_entries(vec!["file1.txt", "file2.rs", "file3.md"]);
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Mark all files with "*"
        update_state(&mut state, Transition::MarkPattern { pattern: "*".to_string() });
        
        // Verify all files are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 3);
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[0].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[1].location));
        assert!(state.current_tab_mut().left_pane.marking.is_marked(&entries[2].location));
    }
}
