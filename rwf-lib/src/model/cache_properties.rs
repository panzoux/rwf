//! Property-based tests for DirectoryCache
//!
//! This module contains property tests that verify the correctness of the
//! directory caching implementation.

#[cfg(test)]
mod tests {
    use super::super::{DirectoryCache, FileEntry, Location};
    use proptest::prelude::*;
    use std::time::{Duration, SystemTime};
    use std::path::PathBuf;

    // Strategy for generating FileEntry
    fn arb_file_entry() -> impl Strategy<Value = FileEntry> {
        (
            "[a-z]{1,10}\\.(txt|rs|md)",
            0u64..10000,
            any::<bool>(),
        ).prop_map(|(name, size, is_dir)| {
            FileEntry {
                name,
                location: Location::Local(PathBuf::from("/test")),
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
        })
    }

    // Strategy for generating a vector of FileEntry
    fn arb_entries() -> impl Strategy<Value = Vec<FileEntry>> {
        prop::collection::vec(arb_file_entry(), 0..20)
    }

    /// **Property 21: Cache Checksum Validation**
    ///
    /// *For any* cached directory, if the checksum of the current directory contents
    /// matches the cached checksum, the cached entries should be used; otherwise,
    /// a new read should be initiated.
    ///
    /// **Validates: Requirements 22.5**
    #[test]
    fn prop_cache_checksum_validation() {
        proptest!(|(entries in arb_entries())| {
            let mut cache = DirectoryCache::new(Duration::from_secs(30));
            let location = Location::Local(PathBuf::from("/test"));
            
            // Insert entries into cache
            cache.insert(location.clone(), entries.clone());
            
            // Verify with same entries - should return true
            prop_assert!(cache.verify_checksum(&location, &entries));
            
            // Modify entries (if not empty)
            if !entries.is_empty() {
                let mut modified_entries = entries.clone();
                modified_entries[0].size += 1; // Change size of first entry
                
                // Verify with modified entries - should return false
                prop_assert!(!cache.verify_checksum(&location, &modified_entries));
            }
        });
    }

    /// Test that cache returns entries when checksum matches
    #[test]
    fn prop_cache_returns_entries_on_checksum_match() {
        proptest!(|(entries in arb_entries())| {
            let mut cache = DirectoryCache::new(Duration::from_secs(30));
            let location = Location::Local(PathBuf::from("/test"));
            
            // Insert entries
            cache.insert(location.clone(), entries.clone());
            
            // Get cached entries
            let cached = cache.get(&location);
            prop_assert!(cached.is_some());
            
            // Verify checksum matches
            prop_assert!(cache.verify_checksum(&location, &entries));
            
            // Cached entries should match original
            let cached_entries = cached.unwrap();
            prop_assert_eq!(cached_entries.len(), entries.len());
        });
    }

    /// Test that cache invalidation removes entries
    #[test]
    fn prop_cache_invalidation_removes_entries() {
        proptest!(|(entries in arb_entries())| {
            let mut cache = DirectoryCache::new(Duration::from_secs(30));
            let location = Location::Local(PathBuf::from("/test"));
            
            // Insert entries
            cache.insert(location.clone(), entries);
            
            // Verify cache has entries
            prop_assert!(cache.get(&location).is_some());
            
            // Invalidate cache
            cache.invalidate(&location);
            
            // Cache should be empty
            prop_assert!(cache.get(&location).is_none());
        });
    }

    /// Test that different entry sets produce different checksums
    #[test]
    fn prop_different_entries_different_checksums() {
        proptest!(|(
            entries1 in arb_entries(),
            entries2 in arb_entries(),
        )| {
            let cache = DirectoryCache::new(Duration::from_secs(30));
            
            // Calculate checksums
            let checksum1 = cache.calculate_checksum(&entries1);
            let checksum2 = cache.calculate_checksum(&entries2);
            
            // If entries have different lengths or different content, checksums should differ
            // We check a simple case: different lengths
            if entries1.len() != entries2.len() {
                prop_assert_ne!(checksum1, checksum2);
            }
        });
    }

    /// Test that cache respects TTL
    #[test]
    fn test_cache_ttl_expiration() {
        let mut cache = DirectoryCache::new(Duration::from_millis(50));
        let location = Location::Local(PathBuf::from("/test"));
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
            }
        ];
        
        // Insert entries
        cache.insert(location.clone(), entries);
        
        // Should be available immediately
        assert!(cache.get(&location).is_some());
        
        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(60));
        
        // Should be expired
        assert!(cache.get(&location).is_none());
    }
}
