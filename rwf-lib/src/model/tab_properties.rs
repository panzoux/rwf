//! Property-based tests for TabManager
//!
//! **Validates: Requirements 27.1**

use super::tab::TabManager;
use super::pane::PaneModel;
use super::location::Location;
use super::file_entry::FileEntry;
use proptest::prelude::*;
use std::path::PathBuf;
use std::time::SystemTime;

/// Represents a modification to a pane's state
#[derive(Debug, Clone)]
enum PaneModification {
    MoveCursor(usize),
    SetScrollOffset(usize),
    MarkFile(usize),
    UnmarkFile(usize),
    ChangeLocation(String),
}

impl PaneModification {
    /// Apply this modification to a pane
    fn apply(&self, pane: &mut PaneModel) {
        match self {
            PaneModification::MoveCursor(pos) => {
                if !pane.entries.is_empty() {
                    pane.cursor = (*pos).min(pane.entries.len() - 1);
                }
            }
            PaneModification::SetScrollOffset(offset) => {
                pane.scroll_offset = *offset;
            }
            PaneModification::MarkFile(index) => {
                if let Some(entry) = pane.entries.get_mut(*index) {
                    entry.marked = true;
                }
            }
            PaneModification::UnmarkFile(index) => {
                if let Some(entry) = pane.entries.get_mut(*index) {
                    entry.marked = false;
                }
            }
            PaneModification::ChangeLocation(path) => {
                pane.current_location = Location::Local(PathBuf::from(path));
            }
        }
    }
}

/// Captures the state of a pane for comparison
#[derive(Debug, Clone, PartialEq)]
struct PaneSnapshot {
    cursor: usize,
    scroll_offset: usize,
    marked_count: usize,
    location_path: String,
}

impl PaneSnapshot {
    fn from_pane(pane: &PaneModel) -> Self {
        Self {
            cursor: pane.cursor,
            scroll_offset: pane.scroll_offset,
            marked_count: pane.entries.iter().filter(|e| e.marked).count(),
            location_path: pane.current_location.display_path(),
        }
    }
}

// Strategy for generating a TabManager with multiple tabs
fn tab_manager_with_tabs(min_tabs: usize, max_tabs: usize) -> impl Strategy<Value = TabManager> {
    (min_tabs..=max_tabs).prop_flat_map(|num_tabs| {
        Just(num_tabs).prop_map(|n| {
            let mut manager = TabManager::new();
            // Create additional tabs
            for _ in 1..n {
                manager.create_tab();
            }
            manager
        })
    })
}

// Strategy for generating pane modifications
fn pane_modification() -> impl Strategy<Value = PaneModification> {
    prop_oneof![
        (0usize..20).prop_map(PaneModification::MoveCursor),
        (0usize..50).prop_map(PaneModification::SetScrollOffset),
        (0usize..10).prop_map(PaneModification::MarkFile),
        (0usize..10).prop_map(PaneModification::UnmarkFile),
        "[a-z]{3,10}".prop_map(|s| PaneModification::ChangeLocation(format!("/tmp/{}", s))),
    ]
}

proptest! {
    /// **Property 23: Tab Independence**
    ///
    /// For any AppState with multiple tabs, modifying the pane state in one tab
    /// should not affect the pane state in any other tab.
    ///
    /// This test verifies that:
    /// 1. Changes to cursor position in one tab don't affect other tabs
    /// 2. Changes to scroll offset in one tab don't affect other tabs
    /// 3. Marking files in one tab doesn't affect other tabs
    /// 4. Changing location in one tab doesn't affect other tabs
    ///
    /// **Validates: Requirements 27.1**
    #[test]
    fn prop_tab_independence_cursor_and_scroll(
        mut manager in tab_manager_with_tabs(2, 5),
        target_tab_index in 0usize..5,
        modifications in prop::collection::vec(pane_modification(), 1..10)
    ) {
        // Ensure target_tab_index is valid
        let target_tab_index = target_tab_index % manager.tabs.len();
        
        // Add some entries to all tabs' panes so modifications can be applied
        for tab in &mut manager.tabs {
            for i in 0..10 {
                let entry = FileEntry {
                    name: format!("file{}.txt", i),
                    location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                    size: 100 * i as u64,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                };
                tab.left_pane.entries.push(entry.clone());
                tab.right_pane.entries.push(entry);
            }
        }
        
        // Take snapshots of all tabs' panes BEFORE modifications
        let snapshots_before: Vec<(PaneSnapshot, PaneSnapshot)> = manager.tabs.iter()
            .map(|tab| (
                PaneSnapshot::from_pane(&tab.left_pane),
                PaneSnapshot::from_pane(&tab.right_pane)
            ))
            .collect();
        
        // Apply modifications to the target tab's left pane
        for modification in &modifications {
            modification.apply(&mut manager.tabs[target_tab_index].left_pane);
        }
        
        // Take snapshots of all tabs' panes AFTER modifications
        let snapshots_after: Vec<(PaneSnapshot, PaneSnapshot)> = manager.tabs.iter()
            .map(|tab| (
                PaneSnapshot::from_pane(&tab.left_pane),
                PaneSnapshot::from_pane(&tab.right_pane)
            ))
            .collect();
        
        // Verify that ONLY the target tab's left pane changed
        for (i, (before, after)) in snapshots_before.iter().zip(snapshots_after.iter()).enumerate() {
            if i == target_tab_index {
                // The target tab's left pane should have changed (or at least we applied modifications)
                // We don't assert it changed because some modifications might be no-ops
                // But we do verify the right pane didn't change
                prop_assert_eq!(
                    &before.1,
                    &after.1,
                    "Tab {}: Right pane should not change when left pane is modified",
                    i
                );
            } else {
                // All other tabs should remain completely unchanged
                prop_assert_eq!(
                    &before.0,
                    &after.0,
                    "Tab {}: Left pane should not change when tab {} is modified",
                    i,
                    target_tab_index
                );
                prop_assert_eq!(
                    &before.1,
                    &after.1,
                    "Tab {}: Right pane should not change when tab {} is modified",
                    i,
                    target_tab_index
                );
            }
        }
    }

    /// **Property 23: Tab Independence (Right Pane Modifications)**
    ///
    /// Verify that modifications to a tab's right pane don't affect other tabs.
    ///
    /// **Validates: Requirements 27.1**
    #[test]
    fn prop_tab_independence_right_pane(
        mut manager in tab_manager_with_tabs(2, 5),
        target_tab_index in 0usize..5,
        modifications in prop::collection::vec(pane_modification(), 1..10)
    ) {
        let target_tab_index = target_tab_index % manager.tabs.len();
        
        // Add entries to all tabs
        for tab in &mut manager.tabs {
            for i in 0..10 {
                let entry = FileEntry {
                    name: format!("file{}.txt", i),
                    location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                    size: 100 * i as u64,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                };
                tab.left_pane.entries.push(entry.clone());
                tab.right_pane.entries.push(entry);
            }
        }
        
        // Snapshots before
        let snapshots_before: Vec<(PaneSnapshot, PaneSnapshot)> = manager.tabs.iter()
            .map(|tab| (
                PaneSnapshot::from_pane(&tab.left_pane),
                PaneSnapshot::from_pane(&tab.right_pane)
            ))
            .collect();
        
        // Apply modifications to the target tab's RIGHT pane
        for modification in &modifications {
            modification.apply(&mut manager.tabs[target_tab_index].right_pane);
        }
        
        // Snapshots after
        let snapshots_after: Vec<(PaneSnapshot, PaneSnapshot)> = manager.tabs.iter()
            .map(|tab| (
                PaneSnapshot::from_pane(&tab.left_pane),
                PaneSnapshot::from_pane(&tab.right_pane)
            ))
            .collect();
        
        // Verify independence
        for (i, (before, after)) in snapshots_before.iter().zip(snapshots_after.iter()).enumerate() {
            if i == target_tab_index {
                // The left pane of the target tab should not change
                prop_assert_eq!(
                    &before.0,
                    &after.0,
                    "Tab {}: Left pane should not change when right pane is modified",
                    i
                );
            } else {
                // All other tabs should remain completely unchanged
                prop_assert_eq!(
                    &before.0,
                    &after.0,
                    "Tab {}: Left pane should not change when tab {} is modified",
                    i,
                    target_tab_index
                );
                prop_assert_eq!(
                    &before.1,
                    &after.1,
                    "Tab {}: Right pane should not change when tab {} is modified",
                    i,
                    target_tab_index
                );
            }
        }
    }

    /// **Property 23: Tab Independence (Multiple Tabs Modified)**
    ///
    /// Verify that modifying multiple tabs independently maintains isolation.
    ///
    /// **Validates: Requirements 27.1**
    #[test]
    fn prop_tab_independence_multiple_modifications(
        mut manager in tab_manager_with_tabs(3, 5),
        tab1_mods in prop::collection::vec(pane_modification(), 1..5),
        tab2_mods in prop::collection::vec(pane_modification(), 1..5)
    ) {
        // We'll modify tab 0 and tab 1, and verify tab 2+ remain unchanged
        if manager.tabs.len() < 3 {
            return Ok(());
        }
        
        // Add entries to all tabs
        for tab in &mut manager.tabs {
            for i in 0..10 {
                let entry = FileEntry {
                    name: format!("file{}.txt", i),
                    location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                    size: 100 * i as u64,
                    is_dir: false,
                    is_hidden: false,
                    modified: SystemTime::now(),
                    marked: false,
                    calculated_size: None,
                };
                tab.left_pane.entries.push(entry.clone());
                tab.right_pane.entries.push(entry);
            }
        }
        
        // Take snapshots of tabs 2+ before any modifications
        let unmodified_snapshots_before: Vec<(PaneSnapshot, PaneSnapshot)> = manager.tabs.iter()
            .skip(2)
            .map(|tab| (
                PaneSnapshot::from_pane(&tab.left_pane),
                PaneSnapshot::from_pane(&tab.right_pane)
            ))
            .collect();
        
        // Modify tab 0's left pane
        for modification in &tab1_mods {
            modification.apply(&mut manager.tabs[0].left_pane);
        }
        
        // Modify tab 1's right pane
        for modification in &tab2_mods {
            modification.apply(&mut manager.tabs[1].right_pane);
        }
        
        // Take snapshots of tabs 2+ after modifications
        let unmodified_snapshots_after: Vec<(PaneSnapshot, PaneSnapshot)> = manager.tabs.iter()
            .skip(2)
            .map(|tab| (
                PaneSnapshot::from_pane(&tab.left_pane),
                PaneSnapshot::from_pane(&tab.right_pane)
            ))
            .collect();
        
        // Verify that tabs 2+ remain completely unchanged
        for (i, (before, after)) in unmodified_snapshots_before.iter()
            .zip(unmodified_snapshots_after.iter())
            .enumerate() 
        {
            let actual_tab_index = i + 2;
            prop_assert_eq!(
                &before.0,
                &after.0,
                "Tab {}: Left pane should not change when other tabs are modified",
                actual_tab_index
            );
            prop_assert_eq!(
                &before.1,
                &after.1,
                "Tab {}: Right pane should not change when other tabs are modified",
                actual_tab_index
            );
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_tab_independence_basic() {
        let mut manager = TabManager::new();
        manager.create_tab();
        
        // Add entries to both tabs
        for i in 0..5 {
            let entry = FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            };
            manager.tabs[0].left_pane.entries.push(entry.clone());
            manager.tabs[1].left_pane.entries.push(entry);
        }
        
        // Modify tab 0's cursor
        manager.tabs[0].left_pane.cursor = 3;
        
        // Verify tab 1's cursor is unchanged
        assert_eq!(manager.tabs[1].left_pane.cursor, 0);
    }

    #[test]
    fn test_tab_independence_marking() {
        let mut manager = TabManager::new();
        manager.create_tab();
        
        // Add entries to both tabs
        for i in 0..5 {
            let entry = FileEntry {
                name: format!("file{}.txt", i),
                location: Location::Local(PathBuf::from(format!("/tmp/file{}.txt", i))),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            };
            manager.tabs[0].left_pane.entries.push(entry.clone());
            manager.tabs[1].left_pane.entries.push(entry);
        }
        
        // Mark a file in tab 0
        manager.tabs[0].left_pane.entries[2].marked = true;
        
        // Verify tab 1's files are not marked
        assert!(!manager.tabs[1].left_pane.entries[2].marked);
    }

    #[test]
    fn test_tab_independence_location() {
        let mut manager = TabManager::new();
        manager.create_tab();
        
        let original_location = manager.tabs[1].left_pane.current_location.clone();
        
        // Change location in tab 0
        manager.tabs[0].left_pane.current_location = Location::Local(PathBuf::from("/different/path"));
        
        // Verify tab 1's location is unchanged
        assert_eq!(manager.tabs[1].left_pane.current_location, original_location);
    }
}
