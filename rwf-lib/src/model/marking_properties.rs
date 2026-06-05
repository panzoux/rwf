//! Property-based tests for MarkingModel
//!
//! **Validates: Requirements 5.1, 5.2, 5.3, 5.4**

use crate::model::{MarkingModel, Location, FileEntry};
use proptest::prelude::*;
use std::path::PathBuf;
use std::time::SystemTime;

/// Strategy for generating Location values
fn arb_location() -> impl Strategy<Value = Location> {
    prop_oneof![
        "[a-z]{3,10}".prop_map(|s| Location::Local(PathBuf::from(format!("/tmp/{}", s)))),
        "[a-z]{3,10}".prop_map(|s| Location::Local(PathBuf::from(format!("/home/{}", s)))),
        "[a-z]{3,10}".prop_map(|s| Location::Local(PathBuf::from(format!("/var/{}", s)))),
    ]
}

proptest! {
    #[test]
    fn prop_mark_toggle_idempotence(location in arb_location()) {
        let mut model = MarkingModel::new();
        let initially_marked = model.is_marked(&location);
        model.toggle(location.clone());
        model.toggle(location.clone());
        prop_assert_eq!(model.is_marked(&location), initially_marked);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_toggle_idempotence_simple() {
        let mut model = MarkingModel::new();
        let location = Location::Local(PathBuf::from("/test/file.txt"));
        
        // Start unmarked
        assert!(!model.is_marked(&location));
        
        // Toggle twice
        model.toggle(location.clone());
        model.toggle(location.clone());
        
        // Should be unmarked again
        assert!(!model.is_marked(&location));
    }

    #[test]
    fn test_mark_all_completeness_simple() {
        let mut model = MarkingModel::new();
        let entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file1.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "file2.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file2.txt")),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            },
        ];
        
        model.mark_all(&entries);
        
        // All should be marked
        assert!(model.is_marked(&entries[0].location));
        assert!(model.is_marked(&entries[1].location));
        assert_eq!(model.count(), 2);
    }

    #[test]
    fn test_unmark_all_completeness_simple() {
        let mut model = MarkingModel::new();
        let entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: Location::Local(PathBuf::from("/test/file1.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            },
        ];
        
        model.mark_all(&entries);
        assert_eq!(model.count(), 1);
        
        model.unmark_all();
        
        // None should be marked
        assert!(!model.is_marked(&entries[0].location));
        assert_eq!(model.count(), 0);
    }

    #[test]
    fn test_marking_persistence() {
        let mut model = MarkingModel::new();
        
        // Mark some locations
        let loc1 = Location::Local(PathBuf::from("/dir1/file1.txt"));
        let loc2 = Location::Local(PathBuf::from("/dir1/file2.txt"));
        
        model.mark(loc1.clone());
        model.mark(loc2.clone());
        
        assert_eq!(model.count(), 2);
        
        // Simulate navigation - marks should persist
        assert!(model.is_marked(&loc1));
        assert!(model.is_marked(&loc2));
        
        // Even if we check with a different entry list
        let _different_entries = vec![
            FileEntry {
                name: "other.txt".to_string(),
                location: Location::Local(PathBuf::from("/dir2/other.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::UNIX_EPOCH,
                marked: false,
                calculated_size: None,
            },
        ];
        
        // Original marks still exist
        assert!(model.is_marked(&loc1));
        assert!(model.is_marked(&loc2));
        assert_eq!(model.count(), 2);
    }
}
