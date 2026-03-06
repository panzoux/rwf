//! Integration tests for scrolling behavior
//!
//! This module tests the complete scrolling workflow including:
//! - scroll_offset reset when changing location (Requirement 2A.7)
//! - scroll_offset reset when navigating to registered folders
//! - scroll_offset reset on startup (PaneModel::new)
//! - Cursor and scroll_offset behavior during navigation

#[cfg(test)]
mod tests {
    use crate::state::{AppState, update_state, Transition};
    use crate::config::AppConfig;
    use crate::model::{Location, FileEntry, ActivePane, PaneModel};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_entry(name: &str, size: u64, is_dir: bool, location: &Location) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: location.join(name),
            size,
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        }
    }

    /// Test that scroll_offset is reset to 0 when changing location via ChangeLocation transition
    /// Requirement 2A.7: WHEN scrolling to the beginning of the file list, THE Application SHALL position the first entry at the top of the visible area
    #[test]
    fn test_scroll_offset_reset_on_change_location() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up initial state with some scroll_offset
        let initial_location = Location::Local(PathBuf::from("/test/dir1"));
        state.current_tab_mut().left_pane.current_location = initial_location.clone();
        state.current_tab_mut().left_pane.cursor = 5;
        state.current_tab_mut().left_pane.scroll_offset = 3;
        
        // Populate with entries
        let entries = (0..20)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &initial_location))
            .collect::<Vec<_>>();
        state.current_tab_mut().left_pane.entries = entries;
        
        // Verify initial state
        assert_eq!(state.current_tab().left_pane.cursor, 5);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 3);
        
        // Change location
        let new_location = Location::Local(PathBuf::from("/test/dir2"));
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: new_location.clone(),
        });
        
        // Verify scroll_offset is reset to 0
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
        // Verify cursor is also reset to 0
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        // Verify location changed
        assert_eq!(state.current_tab().left_pane.current_location, new_location);
    }

    /// Test that scroll_offset is reset when navigating to parent directory
    /// This uses ChangeLocation internally via NavigateUp
    #[test]
    fn test_scroll_offset_reset_on_navigate_up() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up initial state in a subdirectory with scroll_offset
        let initial_location = Location::Local(PathBuf::from("/test/dir1/subdir"));
        state.current_tab_mut().left_pane.current_location = initial_location.clone();
        state.current_tab_mut().left_pane.cursor = 10;
        state.current_tab_mut().left_pane.scroll_offset = 5;
        
        // Populate with entries
        let entries = (0..30)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &initial_location))
            .collect::<Vec<_>>();
        state.current_tab_mut().left_pane.entries = entries;
        
        // Navigate up to parent
        let _result = update_state(&mut state, Transition::NavigateUp {
            pane: ActivePane::Left,
        });
        
        // Verify scroll_offset is reset to 0
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
        // Verify cursor is reset to 0
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        // Verify location changed to parent
        assert_eq!(
            state.current_tab().left_pane.current_location,
            Location::Local(PathBuf::from("/test/dir1"))
        );
    }

    /// Test that scroll_offset is 0 when PaneModel is initialized
    /// Requirement 2A.7: On startup, scroll_offset should be 0
    #[test]
    fn test_scroll_offset_zero_on_pane_initialization() {
        let location = Location::Local(PathBuf::from("/test"));
        let pane = PaneModel::new(location.clone());
        
        // Verify scroll_offset is 0 on initialization
        assert_eq!(pane.scroll_offset, 0);
        assert_eq!(pane.cursor, 0);
        assert_eq!(pane.current_location, location);
    }

    /// Test that scroll_offset is reset when navigating to registered folder
    /// Registered folder navigation uses ChangeLocation internally
    #[test]
    fn test_scroll_offset_reset_on_registered_folder_navigation() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up initial state with scroll_offset
        let initial_location = Location::Local(PathBuf::from("/test/dir1"));
        state.current_tab_mut().left_pane.current_location = initial_location.clone();
        state.current_tab_mut().left_pane.cursor = 8;
        state.current_tab_mut().left_pane.scroll_offset = 4;
        
        // Populate with entries
        let entries = (0..25)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &initial_location))
            .collect::<Vec<_>>();
        state.current_tab_mut().left_pane.entries = entries;
        
        // Navigate to registered folder (simulated by ChangeLocation)
        let registered_location = Location::Local(PathBuf::from("/home/user/documents"));
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: registered_location.clone(),
        });
        
        // Verify scroll_offset is reset to 0
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
        // Verify cursor is reset to 0
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        // Verify location changed
        assert_eq!(state.current_tab().left_pane.current_location, registered_location);
    }

    /// Test that scroll_offset is reset in the right pane when changing location
    #[test]
    fn test_scroll_offset_reset_right_pane() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up initial state in right pane with scroll_offset
        let initial_location = Location::Local(PathBuf::from("/test/dir1"));
        state.current_tab_mut().right_pane.current_location = initial_location.clone();
        state.current_tab_mut().right_pane.cursor = 12;
        state.current_tab_mut().right_pane.scroll_offset = 6;
        
        // Populate with entries
        let entries = (0..40)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &initial_location))
            .collect::<Vec<_>>();
        state.current_tab_mut().right_pane.entries = entries;
        
        // Change location in right pane
        let new_location = Location::Local(PathBuf::from("/test/dir2"));
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Right,
            location: new_location.clone(),
        });
        
        // Verify scroll_offset is reset to 0 in right pane
        assert_eq!(state.current_tab().right_pane.scroll_offset, 0);
        // Verify cursor is reset to 0 in right pane
        assert_eq!(state.current_tab().right_pane.cursor, 0);
        // Verify location changed in right pane
        assert_eq!(state.current_tab().right_pane.current_location, new_location);
    }

    /// Test that scroll_offset is reset when entering a directory
    /// This simulates pressing Enter on a directory entry
    #[test]
    fn test_scroll_offset_reset_on_enter_directory() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up initial state with a directory entry
        let initial_location = Location::Local(PathBuf::from("/test"));
        state.current_tab_mut().left_pane.current_location = initial_location.clone();
        state.current_tab_mut().left_pane.cursor = 15;
        state.current_tab_mut().left_pane.scroll_offset = 8;
        
        // Populate with entries including a directory
        let mut entries = vec![
            create_test_entry("subdir", 0, true, &initial_location),
        ];
        entries.extend(
            (0..30)
                .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &initial_location))
        );
        state.current_tab_mut().left_pane.entries = entries;
        
        // Enter the directory
        let subdir_location = initial_location.join("subdir");
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: subdir_location.clone(),
        });
        
        // Verify scroll_offset is reset to 0
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
        // Verify cursor is reset to 0
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        // Verify location changed to subdirectory
        assert_eq!(state.current_tab().left_pane.current_location, subdir_location);
    }

    /// Test that scroll_offset is reset when pane synchronization occurs
    /// Pane sync uses ChangeLocation internally
    #[test]
    fn test_scroll_offset_reset_on_pane_sync() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up left pane with one location
        let left_location = Location::Local(PathBuf::from("/test/left"));
        state.current_tab_mut().left_pane.current_location = left_location.clone();
        
        // Set up right pane with different location and scroll_offset
        let right_location = Location::Local(PathBuf::from("/test/right"));
        state.current_tab_mut().right_pane.current_location = right_location.clone();
        state.current_tab_mut().right_pane.cursor = 9;
        state.current_tab_mut().right_pane.scroll_offset = 4;
        
        // Populate right pane with entries
        let entries = (0..35)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &right_location))
            .collect::<Vec<_>>();
        state.current_tab_mut().right_pane.entries = entries;
        
        // Sync right pane to left pane's location
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Right,
            location: left_location.clone(),
        });
        
        // Verify scroll_offset is reset to 0 in right pane
        assert_eq!(state.current_tab().right_pane.scroll_offset, 0);
        // Verify cursor is reset to 0 in right pane
        assert_eq!(state.current_tab().right_pane.cursor, 0);
        // Verify right pane location matches left pane
        assert_eq!(state.current_tab().right_pane.current_location, left_location);
    }

    /// Test that cursor and scroll position are remembered when returning to a directory
    /// This is the correct TWF behavior - positions are cached per directory
    #[test]
    fn test_navigation_cache_remembers_position() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up initial location with cursor and scroll position
        let dir1 = Location::Local(PathBuf::from("/test/dir1"));
        state.current_tab_mut().left_pane.current_location = dir1.clone();
        state.current_tab_mut().left_pane.cursor = 10;
        state.current_tab_mut().left_pane.scroll_offset = 5;
        
        // Populate with entries
        let entries = (0..30)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &dir1))
            .collect::<Vec<_>>();
        state.current_tab_mut().left_pane.entries = entries.clone();
        
        // Navigate to a different directory
        let dir2 = Location::Local(PathBuf::from("/test/dir2"));
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir2.clone(),
        });
        
        // Verify we're in dir2 with default position (first visit)
        assert_eq!(state.current_tab().left_pane.current_location, dir2);
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
        
        // Navigate back to dir1
        // Need to populate cache with dir1 entries first
        state.cache.insert(dir1.clone(), entries.clone());
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir1.clone(),
        });
        
        // Verify cursor and scroll position are restored
        assert_eq!(state.current_tab().left_pane.current_location, dir1);
        assert_eq!(state.current_tab().left_pane.cursor, 10);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 5);
    }

    /// Test that first visit to a directory starts at cursor=0, scroll=0
    #[test]
    fn test_navigation_cache_first_visit_starts_at_zero() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Navigate to a new directory (first visit)
        let new_dir = Location::Local(PathBuf::from("/test/newdir"));
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: new_dir.clone(),
        });
        
        // Verify cursor and scroll start at 0
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
    }

    /// Test that positions are clamped to valid ranges when entries change
    #[test]
    fn test_navigation_cache_clamps_to_valid_range() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up dir1 with 30 entries, cursor at position 25
        let dir1 = Location::Local(PathBuf::from("/test/dir1"));
        state.current_tab_mut().left_pane.current_location = dir1.clone();
        state.current_tab_mut().left_pane.cursor = 25;
        state.current_tab_mut().left_pane.scroll_offset = 20;
        
        let entries = (0..30)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &dir1))
            .collect::<Vec<_>>();
        state.current_tab_mut().left_pane.entries = entries;
        
        // Navigate away
        let dir2 = Location::Local(PathBuf::from("/test/dir2"));
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir2.clone(),
        });
        
        // Navigate back to dir1, but now it only has 10 entries
        let fewer_entries = (0..10)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &dir1))
            .collect::<Vec<_>>();
        state.cache.insert(dir1.clone(), fewer_entries);
        
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir1.clone(),
        });
        
        // Verify cursor is clamped to valid range (max index is 9)
        assert_eq!(state.current_tab().left_pane.cursor, 9);
        // Scroll offset should be preserved (will be adjusted by scrolling logic)
        assert_eq!(state.current_tab().left_pane.scroll_offset, 20);
    }

    /// Test that navigation cache works independently for left and right panes
    #[test]
    fn test_navigation_cache_independent_panes() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        let dir1 = Location::Local(PathBuf::from("/test/dir1"));
        let dir2 = Location::Local(PathBuf::from("/test/dir2"));
        let dir3 = Location::Local(PathBuf::from("/test/dir3"));
        
        // Set up left pane in dir1 with position 10,5
        state.current_tab_mut().left_pane.current_location = dir1.clone();
        state.current_tab_mut().left_pane.cursor = 10;
        state.current_tab_mut().left_pane.scroll_offset = 5;
        let entries1 = (0..30)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &dir1))
            .collect::<Vec<_>>();
        state.current_tab_mut().left_pane.entries = entries1.clone();
        
        // Set up right pane in dir2 with position 15,8
        state.current_tab_mut().right_pane.current_location = dir2.clone();
        state.current_tab_mut().right_pane.cursor = 15;
        state.current_tab_mut().right_pane.scroll_offset = 8;
        let entries2 = (0..40)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &dir2))
            .collect::<Vec<_>>();
        state.current_tab_mut().right_pane.entries = entries2.clone();
        
        // Navigate left pane to dir2 (this saves dir1's position 10,5)
        state.cache.insert(dir2.clone(), entries2.clone());
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir2.clone(),
        });
        
        // Left pane should start at 0,0 (first visit to dir2 by left pane)
        // Note: The navigation cache is global, but dir2 hasn't been visited yet via navigation
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
        
        // Now set a position in dir2 via left pane
        state.current_tab_mut().left_pane.cursor = 20;
        state.current_tab_mut().left_pane.scroll_offset = 12;
        
        // Navigate left pane to dir3 (this saves dir2's position 20,12)
        let entries3 = (0..50)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &dir3))
            .collect::<Vec<_>>();
        state.cache.insert(dir3.clone(), entries3);
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir3.clone(),
        });
        
        // Navigate left pane back to dir1 (should restore 10,5)
        state.cache.insert(dir1.clone(), entries1.clone());
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir1.clone(),
        });
        
        // Left pane should restore dir1's position (10,5)
        assert_eq!(state.current_tab().left_pane.cursor, 10);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 5);
        
        // Navigate left pane back to dir2 (should restore 20,12)
        state.cache.insert(dir2.clone(), entries2.clone());
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir2.clone(),
        });
        
        // Left pane should restore dir2's position (20,12)
        assert_eq!(state.current_tab().left_pane.cursor, 20);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 12);
        
        // Right pane should still be in dir2 with its original position
        assert_eq!(state.current_tab().right_pane.current_location, dir2);
        assert_eq!(state.current_tab().right_pane.cursor, 15);
        assert_eq!(state.current_tab().right_pane.scroll_offset, 8);
    }

    /// Test that navigation cache handles empty directories correctly
    #[test]
    fn test_navigation_cache_empty_directory() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set up dir1 with entries and position
        let dir1 = Location::Local(PathBuf::from("/test/dir1"));
        state.current_tab_mut().left_pane.current_location = dir1.clone();
        state.current_tab_mut().left_pane.cursor = 5;
        state.current_tab_mut().left_pane.scroll_offset = 3;
        let entries = (0..20)
            .map(|i| create_test_entry(&format!("file{}.txt", i), 100, false, &dir1))
            .collect::<Vec<_>>();
        state.current_tab_mut().left_pane.entries = entries;
        
        // Navigate to empty directory
        let empty_dir = Location::Local(PathBuf::from("/test/empty"));
        state.cache.insert(empty_dir.clone(), vec![]);
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: empty_dir.clone(),
        });
        
        // Empty directory should have cursor and scroll at 0
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
        assert_eq!(state.current_tab().left_pane.entries.len(), 0);
    }

    /// Test that LRU eviction works correctly (cache size limit)
    #[test]
    fn test_navigation_cache_lru_eviction() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Visit many directories to trigger LRU eviction
        // The cache limit is 1000 entries, so we'll visit 1001 directories
        for i in 0..1001 {
            let dir = Location::Local(PathBuf::from(format!("/test/dir{}", i)));
            let entries = vec![create_test_entry("file.txt", 100, false, &dir)];
            state.cache.insert(dir.clone(), entries);
            
            // Set cursor to i for tracking
            state.current_tab_mut().left_pane.cursor = i;
            state.current_tab_mut().left_pane.scroll_offset = i / 2;
            
            let _result = update_state(&mut state, Transition::ChangeLocation {
                pane: ActivePane::Left,
                location: dir,
            });
        }
        
        // The first directory (dir0) should have been evicted
        let dir0 = Location::Local(PathBuf::from("/test/dir0"));
        let entry = vec![create_test_entry("file.txt", 100, false, &dir0)];
        state.cache.insert(dir0.clone(), entry);
        let _result = update_state(&mut state, Transition::ChangeLocation {
            pane: ActivePane::Left,
            location: dir0.clone(),
        });
        
        // Should start at 0,0 (first visit after eviction)
        assert_eq!(state.current_tab().left_pane.cursor, 0);
        assert_eq!(state.current_tab().left_pane.scroll_offset, 0);
    }
}
