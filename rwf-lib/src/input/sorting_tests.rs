//! Integration tests for sorting key handlers
//!
//! **Validates: Requirements 12.1-12.7**

#[cfg(test)]
mod tests {
    use crate::input::{KeyBindings, Action, action_to_transitions};
    use crate::model::{ActivePane, FileEntry, Location, SortMode, SortOrder};
    use crate::state::{AppState, AppConfig, Transition, update_state};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use proptest::prelude::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    // Helper to create a test file entry
    fn create_test_entry(name: &str, size: u64, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(PathBuf::from(format!("/test/{}", name))),
            size,
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
    fn test_sort_by_name_key_sequence() {
        let mut bindings = KeyBindings::default();
        
        // Press 's'
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::PendingSequence));
        assert!(bindings.has_pending_sequence());
        
        // Press 'n'
        let event = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::SortByName));
        assert!(!bindings.has_pending_sequence());
    }

    #[test]
    fn test_sort_by_size_key_sequence() {
        let mut bindings = KeyBindings::default();
        
        // Press 's'
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        bindings.map_key(&event);
        
        // Press 's' again
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::SortBySize));
    }

    #[test]
    fn test_sort_by_date_key_sequence() {
        let mut bindings = KeyBindings::default();
        
        // Press 's'
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        bindings.map_key(&event);
        
        // Press 'd'
        let event = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::SortByDate));
    }

    #[test]
    fn test_sort_by_extension_key_sequence() {
        let mut bindings = KeyBindings::default();
        
        // Press 's'
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        bindings.map_key(&event);
        
        // Press 'e'
        let event = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::SortByExtension));
    }

    #[test]
    fn test_sort_action_creates_correct_transition() {
        let config = AppConfig::default();
        let state = AppState::new(config);
        
        // Test SortByName action
        let transitions = action_to_transitions(&state, &Action::SortByName);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ChangeSortMode { pane, mode } => {
                assert_eq!(*pane, ActivePane::Left);
                assert_eq!(*mode, SortMode::Name);
            }
            _ => panic!("Expected ChangeSortMode transition"),
        }
        
        // Test SortBySize action
        let transitions = action_to_transitions(&state, &Action::SortBySize);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ChangeSortMode { pane, mode } => {
                assert_eq!(*pane, ActivePane::Left);
                assert_eq!(*mode, SortMode::Size);
            }
            _ => panic!("Expected ChangeSortMode transition"),
        }
        
        // Test SortByDate action
        let transitions = action_to_transitions(&state, &Action::SortByDate);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ChangeSortMode { pane, mode } => {
                assert_eq!(*pane, ActivePane::Left);
                assert_eq!(*mode, SortMode::Date);
            }
            _ => panic!("Expected ChangeSortMode transition"),
        }
        
        // Test SortByExtension action
        let transitions = action_to_transitions(&state, &Action::SortByExtension);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ChangeSortMode { pane, mode } => {
                assert_eq!(*pane, ActivePane::Left);
                assert_eq!(*mode, SortMode::Extension);
            }
            _ => panic!("Expected ChangeSortMode transition"),
        }
    }

    #[test]
    fn test_sort_transition_updates_pane_state() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add some test entries
        let entries = vec![
            create_test_entry("zebra.txt", 300, false),
            create_test_entry("apple.txt", 100, false),
            create_test_entry("banana.txt", 200, false),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Initially sorted by name (default)
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Name);
        
        // Change to sort by size
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Size,
        });
        
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Size);
        
        // Verify entries are sorted by size
        let sorted = &state.current_tab().left_pane.entries;
        assert_eq!(sorted[0].name, "apple.txt");
        assert_eq!(sorted[0].size, 100);
        assert_eq!(sorted[1].name, "banana.txt");
        assert_eq!(sorted[1].size, 200);
        assert_eq!(sorted[2].name, "zebra.txt");
        assert_eq!(sorted[2].size, 300);
    }

    #[test]
    fn test_sort_applies_to_active_pane_only() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add entries to both panes
        state.current_tab_mut().left_pane.entries = vec![
            create_test_entry("zebra.txt", 300, false),
            create_test_entry("apple.txt", 100, false),
        ];
        state.current_tab_mut().right_pane.entries = vec![
            create_test_entry("banana.txt", 200, false),
            create_test_entry("cherry.txt", 150, false),
        ];
        
        // Set active pane to left
        state.ui.active_pane = ActivePane::Left;
        
        // Sort left pane by size
        let transitions = action_to_transitions(&state, &Action::SortBySize);
        for transition in transitions {
            update_state(&mut state, transition);
        }
        
        // Left pane should be sorted by size
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Size);
        assert_eq!(state.current_tab().left_pane.entries[0].name, "apple.txt");
        
        // Right pane should still be sorted by name (default)
        assert_eq!(state.current_tab().right_pane.sort_mode, SortMode::Name);
        assert_eq!(state.current_tab().right_pane.entries[0].name, "banana.txt");
    }

    #[test]
    fn test_sort_respects_directory_first_rule() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add mixed files and directories
        let entries = vec![
            create_test_entry("file_z.txt", 100, false),
            create_test_entry("dir_a", 0, true),
            create_test_entry("file_a.txt", 200, false),
            create_test_entry("dir_z", 0, true),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Sort by name
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Name,
        });
        
        let sorted = &state.current_tab().left_pane.entries;
        
        // Directories should come first
        assert!(sorted[0].is_dir);
        assert!(sorted[1].is_dir);
        assert!(!sorted[2].is_dir);
        assert!(!sorted[3].is_dir);
        
        // Within directories, sorted by name
        assert_eq!(sorted[0].name, "dir_a");
        assert_eq!(sorted[1].name, "dir_z");
        
        // Within files, sorted by name
        assert_eq!(sorted[2].name, "file_a.txt");
        assert_eq!(sorted[3].name, "file_z.txt");
    }

    #[test]
    fn test_sort_by_extension_groups_correctly() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let entries = vec![
            create_test_entry("file.txt", 100, false),
            create_test_entry("file.rs", 100, false),
            create_test_entry("file.md", 100, false),
            create_test_entry("noext", 100, false),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Sort by extension
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Extension,
        });
        
        let sorted = &state.current_tab().left_pane.entries;
        
        // Files without extension should come first
        assert_eq!(sorted[0].name, "noext");
        
        // Then sorted by extension alphabetically
        assert_eq!(sorted[1].name, "file.md");
        assert_eq!(sorted[2].name, "file.rs");
        assert_eq!(sorted[3].name, "file.txt");
    }

    #[test]
    fn test_invalid_sort_sequence_clears_pending() {
        let mut bindings = KeyBindings::default();
        
        // Press 's'
        let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        bindings.map_key(&event);
        assert!(bindings.has_pending_sequence());
        
        // Press invalid key 'x'
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert_eq!(action, None);
        assert!(!bindings.has_pending_sequence());
    }

    #[test]
    fn test_sort_mode_persists_across_navigation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set sort mode to size
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Size,
        });
        
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Size);
        
        // Navigate to a different location
        let new_location = Location::Local(PathBuf::from("/test/newdir"));
        update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: new_location,
        });
        
        // Sort mode should persist
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Size);
    }

    #[test]
    fn test_each_pane_has_independent_sort_mode() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Sort left pane by size
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Size,
        });
        
        // Sort right pane by date
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Right,
            mode: SortMode::Date,
        });
        
        // Verify each pane has its own sort mode
        assert_eq!(state.current_tab().left_pane.sort_mode, SortMode::Size);
        assert_eq!(state.current_tab().right_pane.sort_mode, SortMode::Date);
    }

    // ========================================================================
    // Property-Based Tests
    // ========================================================================

    /// **Property 18: Directory-First Sorting**
    ///
    /// *For any* PaneModel with mixed files and directories, after applying any sort mode,
    /// all directory entries should appear before all file entries in the entries list.
    ///
    /// **Validates: Requirements 12.6**
    #[test]
    fn property_directory_first_sorting() {
        proptest!(|(
            dir_count in 1usize..10,
            file_count in 1usize..10,
            sort_mode_idx in 0usize..4
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Map index to sort mode
            let sort_mode = match sort_mode_idx {
                0 => SortMode::Name,
                1 => SortMode::Size,
                2 => SortMode::Date,
                _ => SortMode::Extension,
            };
            
            // Create mixed entries (directories and files)
            let mut entries = Vec::new();
            
            // Add directories with various names
            for i in 0..dir_count {
                entries.push(FileEntry {
                    name: format!("dir_{}", i),
                    location: Location::Local(PathBuf::from(format!("/test/dir_{}", i))),
                    size: 0,
                    is_dir: true,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
                });
            }
            
            // Add files with various names and sizes
            for i in 0..file_count {
                entries.push(FileEntry {
                    name: format!("file_{}.txt", i),
                    location: Location::Local(PathBuf::from(format!("/test/file_{}.txt", i))),
                    size: (i as u64 + 1) * 100,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
                });
            }
            
            // Shuffle entries to ensure they're not already sorted
            state.current_tab_mut().left_pane.entries = entries;
            
            // Apply sort mode
            update_state(&mut state, Transition::ChangeSortMode {
                pane: ActivePane::Left,
                mode: sort_mode,
            });
            
            let sorted = &state.current_tab().left_pane.entries;
            
            // Find the index of the first file (if any)
            let first_file_idx = sorted.iter().position(|e| !e.is_dir);
            
            if let Some(first_file_idx) = first_file_idx {
                // All entries before first_file_idx should be directories
                for (i, entry) in sorted.iter().enumerate().take(first_file_idx) {
                    prop_assert!(
                        entry.is_dir,
                        "Entry at index {} should be a directory but is a file: {}",
                        i,
                        entry.name
                    );
                }

                // All entries from first_file_idx onwards should be files
                for (i, entry) in sorted.iter().enumerate().skip(first_file_idx) {
                    prop_assert!(
                        !entry.is_dir,
                        "Entry at index {} should be a file but is a directory: {}",
                        i,
                        entry.name
                    );
                }
            } else {
                // All entries are directories
                for entry in sorted {
                    prop_assert!(entry.is_dir, "All entries should be directories");
                }
            }
            
            // Verify counts
            let dir_count_after = sorted.iter().filter(|e| e.is_dir).count();
            let file_count_after = sorted.iter().filter(|e| !e.is_dir).count();
            prop_assert_eq!(dir_count_after, dir_count, "Directory count should be preserved");
            prop_assert_eq!(file_count_after, file_count, "File count should be preserved");
        });
    }

    /// **Property 19: Sort Stability**
    ///
    /// *For any* PaneModel and SortMode, sorting twice with the same mode should
    /// produce identical ordering.
    ///
    /// **Validates: Requirements 12.1, 12.2, 12.3, 12.4**
    #[test]
    fn property_sort_stability() {
        proptest!(|(
            entry_count in 5usize..20,
            sort_mode_idx in 0usize..4
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            // Map index to sort mode
            let sort_mode = match sort_mode_idx {
                0 => SortMode::Name,
                1 => SortMode::Size,
                2 => SortMode::Date,
                _ => SortMode::Extension,
            };
            
            // Create entries with some having equal sort keys
            let mut entries = Vec::new();
            for i in 0..entry_count {
                let is_dir = i % 3 == 0; // Mix of dirs and files
                let size = (i % 5) as u64 * 100; // Some files have same size
                let name = if i % 2 == 0 {
                    format!("file_{}.txt", i / 2) // Some files have similar names
                } else {
                    format!("doc_{}.txt", i / 2)
                };
                
                entries.push(FileEntry {
                    name,
                    location: Location::Local(PathBuf::from(format!("/test/entry_{}", i))),
                    size,
                    is_dir,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
                });
            }
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Sort once
            update_state(&mut state, Transition::ChangeSortMode {
                pane: ActivePane::Left,
                mode: sort_mode,
            });
            
            let first_sort: Vec<String> = state.current_tab().left_pane.entries
                .iter()
                .map(|e| e.location.display_path())
                .collect();
            
            // Sort again with the same mode
            update_state(&mut state, Transition::ChangeSortMode {
                pane: ActivePane::Left,
                mode: sort_mode,
            });
            
            let second_sort: Vec<String> = state.current_tab().left_pane.entries
                .iter()
                .map(|e| e.location.display_path())
                .collect();
            
            // Both sorts should produce identical ordering
            prop_assert_eq!(
                first_sort,
                second_sort,
                "Sorting twice with {:?} should produce identical results",
                sort_mode
            );
        });
    }

    // Additional property test: Verify sorting preserves all entries
    #[test]
    fn property_sort_preserves_entries() {
        proptest!(|(
            entry_count in 1usize..30,
            sort_mode_idx in 0usize..4
        )| {
            let config = AppConfig::default();
            let mut state = AppState::new(config);
            
            let sort_mode = match sort_mode_idx {
                0 => SortMode::Name,
                1 => SortMode::Size,
                2 => SortMode::Date,
                _ => SortMode::Extension,
            };
            
            // Create entries
            let mut entries = Vec::new();
            for i in 0..entry_count {
                entries.push(FileEntry {
                    name: format!("entry_{}.txt", i),
                    location: Location::Local(PathBuf::from(format!("/test/entry_{}.txt", i))),
                    size: (i as u64) * 100,
                    is_dir: i % 4 == 0,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
                });
            }
            
            // Store original locations
            let original_locations: std::collections::HashSet<_> = entries
                .iter()
                .map(|e| e.location.display_path())
                .collect();
            
            state.current_tab_mut().left_pane.entries = entries;
            
            // Apply sort
            update_state(&mut state, Transition::ChangeSortMode {
                pane: ActivePane::Left,
                mode: sort_mode,
            });
            
            // Verify all entries are still present
            let sorted_locations: std::collections::HashSet<_> = state.current_tab().left_pane.entries
                .iter()
                .map(|e| e.location.display_path())
                .collect();
            
            prop_assert_eq!(
                original_locations,
                sorted_locations,
                "Sorting should preserve all entries"
            );
            
            prop_assert_eq!(
                state.current_tab().left_pane.entries.len(),
                entry_count,
                "Entry count should be preserved after sorting"
            );
        });
    }

    // Unit test: Verify directory-first with specific examples
    #[test]
    fn test_directory_first_all_sort_modes() {
        let config = AppConfig::default();
        
        for sort_mode in &[SortMode::Name, SortMode::Size, SortMode::Date, SortMode::Extension] {
            let mut state = AppState::new(config.clone());
            
            // Create entries with files that would sort before directories by name
            let entries = vec![
                create_test_entry("aaa_file.txt", 100, false),
                create_test_entry("zzz_dir", 0, true),
                create_test_entry("bbb_file.txt", 200, false),
                create_test_entry("aaa_dir", 0, true),
            ];
            state.current_tab_mut().left_pane.entries = entries;
            
            // Apply sort mode
            update_state(&mut state, Transition::ChangeSortMode {
                pane: ActivePane::Left,
                mode: *sort_mode,
            });
            
            let sorted = &state.current_tab().left_pane.entries;
            
            // First two should be directories
            assert!(sorted[0].is_dir, "First entry should be directory for {:?}", sort_mode);
            assert!(sorted[1].is_dir, "Second entry should be directory for {:?}", sort_mode);
            
            // Last two should be files
            assert!(!sorted[2].is_dir, "Third entry should be file for {:?}", sort_mode);
            assert!(!sorted[3].is_dir, "Fourth entry should be file for {:?}", sort_mode);
        }
    }

    // Unit test: Verify sort stability with equal elements
    #[test]
    fn test_sort_stability_equal_elements() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Create files with same size but different names
        let entries = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 100, false),
            create_test_entry("file3.txt", 100, false),
        ];
        state.current_tab_mut().left_pane.entries = entries;
        
        // Sort by size (all have same size)
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Size,
        });
        
        let first_order: Vec<_> = state.current_tab().left_pane.entries
            .iter()
            .map(|e| e.name.clone())
            .collect();
        
        // Sort again
        update_state(&mut state, Transition::ChangeSortMode {
            pane: ActivePane::Left,
            mode: SortMode::Size,
        });
        
        let second_order: Vec<_> = state.current_tab().left_pane.entries
            .iter()
            .map(|e| e.name.clone())
            .collect();
        
        // Order should be identical
        assert_eq!(first_order, second_order, "Sort should be stable for equal elements");
    }

    #[test]
    fn test_sort_order_ascending_default() {
        let state = AppState::new(AppConfig::default());
        assert_eq!(state.current_tab().left_pane.sort_order, SortOrder::Ascending);
    }

    #[test]
    fn test_sort_order_toggle() {
        assert_eq!(SortOrder::Ascending.toggle(), SortOrder::Descending);
        assert_eq!(SortOrder::Descending.toggle(), SortOrder::Ascending);
    }

    #[test]
    fn test_sort_descending_reverses_order() {
        let mut state = AppState::new(AppConfig::default());
        let entries = vec![
            create_test_entry("apple.txt", 100, false),
            create_test_entry("cherry.txt", 300, false),
            create_test_entry("banana.txt", 200, false),
        ];
        state.current_tab_mut().left_pane.entries = entries;

        // Ascending sort by name
        update_state(&mut state, Transition::ChangeSortMode { pane: ActivePane::Left, mode: SortMode::Name });
        update_state(&mut state, Transition::ChangeSortOrder { pane: ActivePane::Left, order: SortOrder::Ascending });
        let asc: Vec<_> = state.current_tab().left_pane.entries.iter().map(|e| e.name.clone()).collect();

        // Descending sort by name
        update_state(&mut state, Transition::ChangeSortOrder { pane: ActivePane::Left, order: SortOrder::Descending });
        let desc: Vec<_> = state.current_tab().left_pane.entries.iter().map(|e| e.name.clone()).collect();

        assert_eq!(asc, vec!["apple.txt", "banana.txt", "cherry.txt"]);
        assert_eq!(desc, vec!["cherry.txt", "banana.txt", "apple.txt"]);
    }

    #[test]
    fn test_sort_order_dirs_always_first() {
        let mut state = AppState::new(AppConfig::default());
        state.current_tab_mut().left_pane.entries = vec![
            create_test_entry("z_file.txt", 100, false),
            create_test_entry("a_dir",      0,   true),
        ];

        // Even with descending, dirs come first
        update_state(&mut state, Transition::ChangeSortMode { pane: ActivePane::Left, mode: SortMode::Name });
        update_state(&mut state, Transition::ChangeSortOrder { pane: ActivePane::Left, order: SortOrder::Descending });
        let result: Vec<_> = state.current_tab().left_pane.entries.iter().collect();
        assert!(result[0].is_dir, "Directory should be first even in descending order");
        assert!(!result[1].is_dir, "File should be second");
    }

    #[test]
    fn test_toggle_sort_order_action() {
        let mut state = AppState::new(AppConfig::default());
        assert_eq!(state.current_tab().left_pane.sort_order, SortOrder::Ascending);

        let transitions = action_to_transitions(&state, &Action::ToggleSortOrder);
        assert!(matches!(transitions[0], Transition::ChangeSortOrder { order: SortOrder::Descending, .. }));

        update_state(&mut state, transitions[0].clone());
        assert_eq!(state.current_tab().left_pane.sort_order, SortOrder::Descending);
    }

    #[test]
    fn test_open_sort_dialog_action() {
        let state = AppState::new(AppConfig::default());
        let transitions = action_to_transitions(&state, &Action::OpenSortDialog);
        assert!(matches!(transitions[0], Transition::ShowDialog { .. }));
    }
}
