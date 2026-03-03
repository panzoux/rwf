//! Property-based tests for PaneModel
//!
//! **Validates: Requirements 2.2, 2.3, 2.4, 2.5, 2.6, 2.7**

use super::pane::PaneModel;
use super::location::Location;
use super::file_entry::FileEntry;
use proptest::prelude::*;
use std::path::PathBuf;
use std::time::SystemTime;

/// Cursor movement direction
#[derive(Debug, Clone, Copy)]
enum CursorMove {
    Up,
    Down,
    Home,
    End,
    PageUp(usize),   // page size
    PageDown(usize), // page size
}

impl CursorMove {
    /// Apply cursor movement to a pane, respecting bounds
    fn apply(&self, pane: &mut PaneModel) {
        let len = pane.entries.len();
        if len == 0 {
            pane.cursor = 0;
            return;
        }

        match self {
            CursorMove::Up => {
                if pane.cursor > 0 {
                    pane.cursor -= 1;
                }
            }
            CursorMove::Down => {
                if pane.cursor + 1 < len {
                    pane.cursor += 1;
                }
            }
            CursorMove::Home => {
                pane.cursor = 0;
            }
            CursorMove::End => {
                pane.cursor = len.saturating_sub(1);
            }
            CursorMove::PageUp(page_size) => {
                pane.cursor = pane.cursor.saturating_sub(*page_size);
            }
            CursorMove::PageDown(page_size) => {
                let new_pos = pane.cursor.saturating_add(*page_size);
                pane.cursor = new_pos.min(len.saturating_sub(1));
            }
        }
    }
}

// Strategy for generating valid file names
fn file_name() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,20}\\.[a-z]{2,4}"
        .prop_map(|s| s.to_string())
}

// Strategy for generating FileEntry
fn file_entry() -> impl Strategy<Value = FileEntry> {
    (file_name(), any::<bool>(), any::<bool>(), 0u64..1_000_000_000u64)
        .prop_map(|(name, is_dir, is_hidden, size)| {
            FileEntry {
                name,
                location: Location::Local(PathBuf::from("/tmp/test")),
                size,
                is_dir,
                is_hidden,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            }
        })
}

// Strategy for generating a PaneModel with N entries
fn pane_with_entries(min_entries: usize, max_entries: usize) -> impl Strategy<Value = PaneModel> {
    prop::collection::vec(file_entry(), min_entries..=max_entries)
        .prop_map(|entries| {
            let mut pane = PaneModel::new(Location::Local(PathBuf::from("/tmp")));
            pane.entries = entries;
            pane.cursor = 0;
            pane
        })
}

// Strategy for generating cursor movement operations
fn cursor_move_op() -> impl Strategy<Value = CursorMove> {
    prop_oneof![
        Just(CursorMove::Up),
        Just(CursorMove::Down),
        Just(CursorMove::Home),
        Just(CursorMove::End),
        (1usize..20).prop_map(CursorMove::PageUp),
        (1usize..20).prop_map(CursorMove::PageDown),
    ]
}

proptest! {
    /// **Property 4: Cursor Bounds Invariant**
    ///
    /// For any PaneModel with N entries, the cursor position should always be in the range [0, N-1],
    /// and cursor movement transitions should never violate this bound.
    ///
    /// **Validates: Requirements 2.2, 2.3, 2.4, 2.5, 2.6, 2.7**
    #[test]
    fn prop_cursor_always_within_bounds(pane in pane_with_entries(1, 100)) {
        let len = pane.entries.len();
        
        // Initial cursor should be within bounds
        prop_assert!(
            pane.cursor < len,
            "Initial cursor {} should be < len {}",
            pane.cursor,
            len
        );
        
        // Test that cursor stays within bounds after initialization
        prop_assert!(
            pane.cursor <= len.saturating_sub(1),
            "Cursor {} should be <= {}",
            pane.cursor,
            len.saturating_sub(1)
        );
    }

    /// **Property 4: Cursor Bounds Invariant (After Up Movement)**
    ///
    /// Moving cursor up should never make it negative (stays at 0 or above).
    ///
    /// **Validates: Requirements 2.2**
    #[test]
    fn prop_cursor_up_respects_lower_bound(mut pane in pane_with_entries(1, 100), moves in 1usize..50) {
        let initial_cursor = pane.cursor;
        
        // Apply multiple up movements
        for _ in 0..moves {
            CursorMove::Up.apply(&mut pane);
        }
        
        // Cursor should never go below 0
        prop_assert_eq!(
            pane.cursor,
            0,
            "After {} up moves from {}, cursor should be at 0",
            moves,
            initial_cursor
        );
    }

    /// **Property 4: Cursor Bounds Invariant (After Down Movement)**
    ///
    /// Moving cursor down should never exceed entries.len() - 1.
    ///
    /// **Validates: Requirements 2.3**
    #[test]
    fn prop_cursor_down_respects_upper_bound(mut pane in pane_with_entries(1, 100), moves in 1usize..200) {
        let len = pane.entries.len();
        
        // Apply multiple down movements
        for _ in 0..moves {
            CursorMove::Down.apply(&mut pane);
        }
        
        // Cursor should never exceed len - 1
        prop_assert!(
            pane.cursor < len,
            "After {} down moves, cursor {} should be < len {}",
            moves,
            pane.cursor,
            len
        );
        
        prop_assert!(
            pane.cursor <= len - 1,
            "After {} down moves, cursor {} should be <= {}",
            moves,
            pane.cursor,
            len - 1
        );
        
        // After enough moves, cursor should be at the last position
        if moves >= len {
            prop_assert_eq!(
                pane.cursor,
                len - 1,
                "After {} down moves (>= len), cursor should be at {} (len-1)",
                moves,
                len - 1
            );
        }
    }

    /// **Property 4: Cursor Bounds Invariant (Home Movement)**
    ///
    /// Home key should always move cursor to position 0.
    ///
    /// **Validates: Requirements 2.4**
    #[test]
    fn prop_cursor_home_moves_to_first(mut pane in pane_with_entries(1, 100)) {
        // Set cursor to some arbitrary position
        pane.cursor = pane.entries.len() / 2;
        
        // Apply Home movement
        CursorMove::Home.apply(&mut pane);
        
        // Cursor should be at 0
        prop_assert_eq!(
            pane.cursor,
            0,
            "Home should move cursor to 0"
        );
    }

    /// **Property 4: Cursor Bounds Invariant (End Movement)**
    ///
    /// End key should always move cursor to the last entry (len - 1).
    ///
    /// **Validates: Requirements 2.5**
    #[test]
    fn prop_cursor_end_moves_to_last(mut pane in pane_with_entries(1, 100)) {
        let len = pane.entries.len();
        
        // Apply End movement
        CursorMove::End.apply(&mut pane);
        
        // Cursor should be at len - 1
        prop_assert_eq!(
            pane.cursor,
            len - 1,
            "End should move cursor to {} (len-1)",
            len - 1
        );
        
        prop_assert!(
            pane.cursor < len,
            "Cursor {} should be < len {}",
            pane.cursor,
            len
        );
    }

    /// **Property 4: Cursor Bounds Invariant (Page Up Movement)**
    ///
    /// Page Up should move cursor up by page size but never below 0.
    ///
    /// **Validates: Requirements 2.6**
    #[test]
    fn prop_cursor_page_up_respects_bounds(mut pane in pane_with_entries(1, 100), page_size in 1usize..30) {
        // Set cursor to middle of list
        pane.cursor = pane.entries.len() / 2;
        let initial_cursor = pane.cursor;
        
        // Apply Page Up
        CursorMove::PageUp(page_size).apply(&mut pane);
        
        // Cursor should be within bounds
        prop_assert!(
            pane.cursor < pane.entries.len(),
            "Cursor {} should be < len {}",
            pane.cursor,
            pane.entries.len()
        );
        
        // Cursor should have moved up (or stayed at 0)
        prop_assert!(
            pane.cursor <= initial_cursor,
            "Cursor {} should be <= initial {}",
            pane.cursor,
            initial_cursor
        );
        
        // If page_size >= initial_cursor, cursor should be at 0
        if page_size >= initial_cursor {
            prop_assert_eq!(
                pane.cursor,
                0,
                "Page up by {} from {} should reach 0",
                page_size,
                initial_cursor
            );
        }
    }

    /// **Property 4: Cursor Bounds Invariant (Page Down Movement)**
    ///
    /// Page Down should move cursor down by page size but never exceed len - 1.
    ///
    /// **Validates: Requirements 2.7**
    #[test]
    fn prop_cursor_page_down_respects_bounds(mut pane in pane_with_entries(1, 100), page_size in 1usize..30) {
        let len = pane.entries.len();
        // Set cursor to middle of list
        pane.cursor = len / 2;
        let initial_cursor = pane.cursor;
        
        // Apply Page Down
        CursorMove::PageDown(page_size).apply(&mut pane);
        
        // Cursor should be within bounds
        prop_assert!(
            pane.cursor < len,
            "Cursor {} should be < len {}",
            pane.cursor,
            len
        );
        
        prop_assert!(
            pane.cursor <= len - 1,
            "Cursor {} should be <= {}",
            pane.cursor,
            len - 1
        );
        
        // Cursor should have moved down (or stayed at len-1)
        prop_assert!(
            pane.cursor >= initial_cursor,
            "Cursor {} should be >= initial {}",
            pane.cursor,
            initial_cursor
        );
    }

    /// **Property 4: Cursor Bounds Invariant (Sequence of Random Movements)**
    ///
    /// Any sequence of cursor movements should maintain the invariant that
    /// 0 <= cursor < entries.len() (or cursor == 0 if entries is empty).
    ///
    /// **Validates: Requirements 2.2, 2.3, 2.4, 2.5, 2.6, 2.7**
    #[test]
    fn prop_cursor_bounds_maintained_through_movement_sequence(
        mut pane in pane_with_entries(1, 100),
        movements in prop::collection::vec(cursor_move_op(), 1..50)
    ) {
        let len = pane.entries.len();
        
        // Apply each movement in sequence
        for movement in movements.iter() {
            movement.apply(&mut pane);
            
            // After each movement, cursor should be within bounds
            prop_assert!(
                pane.cursor < len,
                "After movement {:?}, cursor {} should be < len {}",
                movement,
                pane.cursor,
                len
            );
            
            prop_assert!(
                pane.cursor <= len.saturating_sub(1),
                "After movement {:?}, cursor {} should be <= {}",
                movement,
                pane.cursor,
                len.saturating_sub(1)
            );
        }
    }

    /// **Property 4: Cursor Bounds Invariant (Empty Pane)**
    ///
    /// For a pane with no entries, cursor should always be 0.
    ///
    /// **Validates: Requirements 2.2, 2.3, 2.4, 2.5, 2.6, 2.7**
    #[test]
    fn prop_cursor_zero_for_empty_pane(
        movements in prop::collection::vec(cursor_move_op(), 1..20)
    ) {
        let mut pane = PaneModel::new(Location::Local(PathBuf::from("/tmp")));
        // Pane has no entries
        prop_assert_eq!(pane.entries.len(), 0);
        prop_assert_eq!(pane.cursor, 0);
        
        // Apply movements
        for movement in movements.iter() {
            movement.apply(&mut pane);
            
            // Cursor should remain at 0
            prop_assert_eq!(
                pane.cursor,
                0,
                "After movement {:?} on empty pane, cursor should be 0",
                movement
            );
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_cursor_up_at_boundary() {
        let mut pane = PaneModel::new(Location::Local(PathBuf::from("/tmp")));
        pane.entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Local(PathBuf::from("/tmp/file1.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        pane.cursor = 0;

        CursorMove::Up.apply(&mut pane);
        assert_eq!(pane.cursor, 0, "Cursor should stay at 0 when already at top");
    }

    #[test]
    fn test_cursor_down_at_boundary() {
        let mut pane = PaneModel::new(Location::Local(PathBuf::from("/tmp")));
        pane.entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Local(PathBuf::from("/tmp/file1.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        pane.cursor = 0;

        CursorMove::Down.apply(&mut pane);
        assert_eq!(pane.cursor, 0, "Cursor should stay at 0 when only one entry");
    }

    #[test]
    fn test_empty_pane_cursor() {
        let mut pane = PaneModel::new(Location::Local(PathBuf::from("/tmp")));
        assert_eq!(pane.entries.len(), 0);
        assert_eq!(pane.cursor, 0);

        CursorMove::Down.apply(&mut pane);
        assert_eq!(pane.cursor, 0, "Cursor should stay at 0 for empty pane");

        CursorMove::End.apply(&mut pane);
        assert_eq!(pane.cursor, 0, "Cursor should stay at 0 for empty pane");
    }
}
