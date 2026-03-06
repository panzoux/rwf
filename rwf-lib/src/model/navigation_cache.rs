//! Navigation state cache for cursor and scroll position memory
//!
//! This module implements TWF's navigation state caching behavior, where the application
//! remembers the cursor position and scroll offset for each directory visited.

use super::Location;
use std::collections::HashMap;

/// Maximum number of entries to keep in the cache (LRU eviction)
const MAX_CACHE_SIZE: usize = 1000;

/// Navigation state cache that remembers cursor and scroll positions per location
#[derive(Debug)]
pub struct NavigationStateCache {
    /// Map from location to (cursor_position, scroll_offset)
    cache: HashMap<Location, (usize, usize)>,
    /// LRU tracking: list of locations in order of access (most recent last)
    access_order: Vec<Location>,
}

impl NavigationStateCache {
    /// Create a new empty navigation state cache
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            access_order: Vec::new(),
        }
    }

    /// Save the current cursor and scroll position for a location
    pub fn save(&mut self, location: Location, cursor: usize, scroll_offset: usize) {
        // Update access order for LRU
        if let Some(pos) = self.access_order.iter().position(|l| l == &location) {
            self.access_order.remove(pos);
        }
        self.access_order.push(location.clone());

        // Insert or update the cache entry
        self.cache.insert(location, (cursor, scroll_offset));

        // Evict oldest entry if cache is full
        if self.cache.len() > MAX_CACHE_SIZE {
            if let Some(oldest) = self.access_order.first().cloned() {
                self.cache.remove(&oldest);
                self.access_order.remove(0);
            }
        }
    }

    /// Restore the cursor and scroll position for a location
    /// Returns None if the location is not in the cache (first visit)
    pub fn restore(&mut self, location: &Location) -> Option<(usize, usize)> {
        if let Some(&state) = self.cache.get(location) {
            // Update access order for LRU
            if let Some(pos) = self.access_order.iter().position(|l| l == location) {
                self.access_order.remove(pos);
            }
            self.access_order.push(location.clone());

            Some(state)
        } else {
            None
        }
    }

    /// Get the number of entries in the cache
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear all entries from the cache
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_save_and_restore() {
        let mut cache = NavigationStateCache::new();
        let loc = Location::Local(PathBuf::from("/test"));

        // Save position
        cache.save(loc.clone(), 5, 10);

        // Restore position
        let restored = cache.restore(&loc);
        assert_eq!(restored, Some((5, 10)));
    }

    #[test]
    fn test_restore_nonexistent() {
        let mut cache = NavigationStateCache::new();
        let loc = Location::Local(PathBuf::from("/test"));

        // Try to restore from empty cache
        let restored = cache.restore(&loc);
        assert_eq!(restored, None);
    }

    #[test]
    fn test_update_existing() {
        let mut cache = NavigationStateCache::new();
        let loc = Location::Local(PathBuf::from("/test"));

        // Save initial position
        cache.save(loc.clone(), 5, 10);

        // Update position
        cache.save(loc.clone(), 15, 20);

        // Restore should get updated position
        let restored = cache.restore(&loc);
        assert_eq!(restored, Some((15, 20)));
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = NavigationStateCache::new();

        // Fill cache beyond MAX_CACHE_SIZE
        for i in 0..=MAX_CACHE_SIZE {
            let loc = Location::Local(PathBuf::from(format!("/test{}", i)));
            cache.save(loc, i, i * 2);
        }

        // Cache should be at max size
        assert_eq!(cache.len(), MAX_CACHE_SIZE);

        // First entry should have been evicted
        let first_loc = Location::Local(PathBuf::from("/test0"));
        assert_eq!(cache.restore(&first_loc), None);

        // Last entry should still be there
        let last_loc = Location::Local(PathBuf::from(format!("/test{}", MAX_CACHE_SIZE)));
        assert_eq!(cache.restore(&last_loc), Some((MAX_CACHE_SIZE, MAX_CACHE_SIZE * 2)));
    }

    #[test]
    fn test_lru_access_updates_order() {
        let mut cache = NavigationStateCache::new();

        // Add three entries
        let loc1 = Location::Local(PathBuf::from("/test1"));
        let loc2 = Location::Local(PathBuf::from("/test2"));
        let loc3 = Location::Local(PathBuf::from("/test3"));

        cache.save(loc1.clone(), 1, 1);
        cache.save(loc2.clone(), 2, 2);
        cache.save(loc3.clone(), 3, 3);

        // Access loc1 to make it most recent
        cache.restore(&loc1);

        // Fill cache to trigger eviction
        for i in 4..=MAX_CACHE_SIZE + 1 {
            let loc = Location::Local(PathBuf::from(format!("/test{}", i)));
            cache.save(loc, i, i * 2);
        }

        // loc1 should still be there (was accessed recently)
        assert!(cache.restore(&loc1).is_some());

        // loc2 should have been evicted (oldest unaccessed)
        assert_eq!(cache.restore(&loc2), None);
    }

    #[test]
    fn test_multiple_locations() {
        let mut cache = NavigationStateCache::new();

        let loc1 = Location::Local(PathBuf::from("/dir1"));
        let loc2 = Location::Local(PathBuf::from("/dir2"));
        let loc3 = Location::Local(PathBuf::from("/dir3"));

        cache.save(loc1.clone(), 10, 5);
        cache.save(loc2.clone(), 20, 15);
        cache.save(loc3.clone(), 30, 25);

        assert_eq!(cache.restore(&loc1), Some((10, 5)));
        assert_eq!(cache.restore(&loc2), Some((20, 15)));
        assert_eq!(cache.restore(&loc3), Some((30, 25)));
    }
}
