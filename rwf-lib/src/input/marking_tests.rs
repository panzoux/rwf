//! Tests for marking key handlers

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action};
    use crate::model::{FileEntry, Location};
    use crate::state::{update_state, AppConfig, AppState, Transition};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_entries(count: usize) -> Vec<FileEntry> {
        (0..count)
            .map(|i| FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/test/file{}.txt", i))),
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
    fn test_mark_all_action() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = create_test_entries(5);
        state.current_tab_mut().left_pane.entries = entries;

        // Execute MarkAll action
        let transitions = action_to_transitions(&state, &Action::MarkAll);
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::MarkAll));

        // Apply transition
        update_state(&mut state, transitions[0].clone());

        // Verify all files are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 5);
    }

    #[test]
    fn test_unmark_all_action() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries and mark them
        let entries = create_test_entries(5);
        state.current_tab_mut().left_pane.entries = entries.clone();
        state.current_tab_mut().left_pane.marking.mark_all(&entries);
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 5);

        // Execute UnmarkAll action
        let transitions = action_to_transitions(&state, &Action::UnmarkAll);
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::UnmarkAll));

        // Apply transition
        update_state(&mut state, transitions[0].clone());

        // Verify all files are unmarked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 0);
    }

    #[test]
    fn test_wildcard_marking_action_shows_dialog() {
        let config = AppConfig::default();
        let state = AppState::new(config);

        // Execute WildcardMarking action
        let transitions = action_to_transitions(&state, &Action::WildcardMarking);
        assert_eq!(transitions.len(), 1);

        // Verify it shows a dialog
        match &transitions[0] {
            Transition::ShowDialog { dialog } => {
                assert_eq!(dialog.title, "Wildcard Marking");
                assert!(matches!(
                    dialog.content,
                    crate::model::DialogContent::WildcardMark { .. }
                ));
            }
            _ => panic!("Expected ShowDialog transition"),
        }
    }

    #[test]
    fn test_wildcard_marking_pattern_matching() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries with different names
        let entries = vec![
            FileEntry {
                name: "test.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/test.txt")),
                size: 100,
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
                name: "test.rs".to_string(),
                location: Location::Local(PathBuf::from("/test/test.rs")),
                size: 100,
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
                name: "other.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/other.txt")),
                size: 100,
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
        state.current_tab_mut().left_pane.entries = entries;

        // Mark files matching "*.txt"
        update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "*.txt".to_string(),
            },
        );

        // Verify only .txt files are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/test.txt"))));
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/other.txt"))));
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/test.rs"))));
    }

    #[test]
    fn test_range_marking_mode_entry() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = create_test_entries(10);
        state.current_tab_mut().left_pane.entries = entries;
        state.current_tab_mut().left_pane.cursor = 3;

        // Execute RangeMarking action (first time - enter mode)
        let transitions = action_to_transitions(&state, &Action::RangeMarking);
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::EnterRangeMarkingMode));

        // Apply transition
        update_state(&mut state, transitions[0].clone());

        // Verify range marking mode is active
        assert_eq!(state.ui.range_marking_start, Some(3));
    }

    #[test]
    fn test_range_marking_completion() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = create_test_entries(10);
        state.current_tab_mut().left_pane.entries = entries;
        state.current_tab_mut().left_pane.cursor = 2;

        // Enter range marking mode
        state.ui.range_marking_start = Some(2);

        // Move cursor to position 5
        state.current_tab_mut().left_pane.cursor = 5;

        // Execute RangeMarking action (second time - complete range)
        let transitions = action_to_transitions(&state, &Action::RangeMarking);
        assert_eq!(transitions.len(), 1);

        match &transitions[0] {
            Transition::MarkRange { start, end } => {
                assert_eq!(*start, 2);
                assert_eq!(*end, 5);
            }
            _ => panic!("Expected MarkRange transition"),
        }

        // Apply transition
        update_state(&mut state, transitions[0].clone());

        // Verify files in range are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 4); // files 2, 3, 4, 5

        // Verify range marking mode is exited
        assert_eq!(state.ui.range_marking_start, None);
    }

    #[test]
    fn test_range_marking_reverse_order() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = create_test_entries(10);
        state.current_tab_mut().left_pane.entries = entries;

        // Start at position 7
        state.ui.range_marking_start = Some(7);
        state.current_tab_mut().left_pane.cursor = 3;

        // Execute RangeMarking action
        let transitions = action_to_transitions(&state, &Action::RangeMarking);
        update_state(&mut state, transitions[0].clone());

        // Verify files in range are marked (should handle reverse order)
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 5); // files 3, 4, 5, 6, 7
    }

    #[test]
    fn test_invert_marks_action() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = create_test_entries(5);
        state.current_tab_mut().left_pane.entries = entries.clone();

        // Mark some files
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(entries[0].location.clone());
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(entries[2].location.clone());
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);

        // Execute InvertMarks action
        let transitions = action_to_transitions(&state, &Action::InvertMarks);
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::InvertMarks));

        // Apply transition
        update_state(&mut state, transitions[0].clone());

        // Verify marks are inverted
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 3);
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&entries[0].location));
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&entries[1].location));
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&entries[2].location));
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&entries[3].location));
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&entries[4].location));
    }

    #[test]
    fn test_wildcard_question_mark() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Add test entries
        let entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file1.txt")),
                size: 100,
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
                size: 100,
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
                name: "file10.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file10.txt")),
                size: 100,
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
        state.current_tab_mut().left_pane.entries = entries;

        // Mark files matching "file?.txt" (single digit)
        update_state(
            &mut state,
            Transition::MarkPattern {
                pattern: "file?.txt".to_string(),
            },
        );

        // Verify only single-digit files are marked
        assert_eq!(state.current_tab_mut().left_pane.marking.count(), 2);
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/file1.txt"))));
        assert!(state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/file2.txt"))));
        assert!(!state
            .current_tab_mut()
            .left_pane
            .marking
            .is_marked(&Location::Local(PathBuf::from("/test/file10.txt"))));
    }
}
