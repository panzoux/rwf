//! Directory caching for fast navigation
//!
//! This module implements a cache for directory contents with TTL and checksum validation.

use super::{Location, FileEntry};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Cache for recently visited directories
#[derive(Debug)]
pub struct DirectoryCache {
    entries: HashMap<Location, CachedDirectory>,
    ttl: Duration,
}

impl DirectoryCache {
    /// Create a new directory cache with the specified TTL
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }
    
    /// Get cached directory entries if available and not expired
    pub fn get(&self, location: &Location) -> Option<&Vec<FileEntry>> {
        if let Some(cached) = self.entries.get(location) {
            if cached.timestamp.elapsed() < self.ttl {
                return Some(&cached.entries);
            }
        }
        None
    }
    
    /// Get cached directory with checksum for validation
    pub fn get_with_checksum(&self, location: &Location) -> Option<(&Vec<FileEntry>, u64)> {
        if let Some(cached) = self.entries.get(location) {
            if cached.timestamp.elapsed() < self.ttl {
                return Some((&cached.entries, cached.checksum));
            }
        }
        None
    }
    
    /// Insert directory entries into cache with checksum
    pub fn insert(&mut self, location: Location, entries: Vec<FileEntry>) {
        let checksum = self.calculate_checksum(&entries);
        let cached = CachedDirectory {
            entries,
            timestamp: Instant::now(),
            checksum,
        };
        self.entries.insert(location, cached);
    }
    
    /// Invalidate cache entry for a specific location
    pub fn invalidate(&mut self, location: &Location) {
        self.entries.remove(location);
    }
    
    /// Invalidate all cache entries
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }
    
    /// Remove expired cache entries
    pub fn cleanup_expired(&mut self) {
        self.entries.retain(|_, cached| {
            cached.timestamp.elapsed() < self.ttl
        });
    }
    
    /// Calculate checksum for directory entries
    /// Uses name, size, and is_dir to detect changes
    pub fn calculate_checksum(&self, entries: &[FileEntry]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for entry in entries {
            entry.name.hash(&mut hasher);
            entry.size.hash(&mut hasher);
            entry.is_dir.hash(&mut hasher);
        }
        hasher.finish()
    }
    
    /// Verify if cached directory matches current checksum
    pub fn verify_checksum(&self, location: &Location, current_entries: &[FileEntry]) -> bool {
        if let Some(cached) = self.entries.get(location) {
            let current_checksum = self.calculate_checksum(current_entries);
            return cached.checksum == current_checksum;
        }
        false
    }
}

/// Cached directory with metadata
#[derive(Debug)]
pub struct CachedDirectory {
    pub entries: Vec<FileEntry>,
    pub timestamp: Instant,
    pub checksum: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn create_test_entry(name: &str, size: u64, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            location: Location::Local(std::path::PathBuf::from(name)),
            size,
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = DirectoryCache::new(Duration::from_secs(30));
        let location = Location::Local(std::path::PathBuf::from("/test"));
        let entries = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 200, false),
        ];
        
        cache.insert(location.clone(), entries.clone());
        
        let cached = cache.get(&location);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 2);
    }

    #[test]
    fn test_cache_expiration() {
        let mut cache = DirectoryCache::new(Duration::from_millis(10));
        let location = Location::Local(std::path::PathBuf::from("/test"));
        let entries = vec![create_test_entry("file1.txt", 100, false)];
        
        cache.insert(location.clone(), entries);
        
        // Should be available immediately
        assert!(cache.get(&location).is_some());
        
        // Wait for expiration
        std::thread::sleep(Duration::from_millis(20));
        
        // Should be expired
        assert!(cache.get(&location).is_none());
    }

    #[test]
    fn test_cache_invalidate() {
        let mut cache = DirectoryCache::new(Duration::from_secs(30));
        let location = Location::Local(std::path::PathBuf::from("/test"));
        let entries = vec![create_test_entry("file1.txt", 100, false)];
        
        cache.insert(location.clone(), entries);
        assert!(cache.get(&location).is_some());
        
        cache.invalidate(&location);
        assert!(cache.get(&location).is_none());
    }

    #[test]
    fn test_cache_invalidate_all() {
        let mut cache = DirectoryCache::new(Duration::from_secs(30));
        let loc1 = Location::Local(std::path::PathBuf::from("/test1"));
        let loc2 = Location::Local(std::path::PathBuf::from("/test2"));
        
        cache.insert(loc1.clone(), vec![create_test_entry("file1.txt", 100, false)]);
        cache.insert(loc2.clone(), vec![create_test_entry("file2.txt", 200, false)]);
        
        assert!(cache.get(&loc1).is_some());
        assert!(cache.get(&loc2).is_some());
        
        cache.invalidate_all();
        
        assert!(cache.get(&loc1).is_none());
        assert!(cache.get(&loc2).is_none());
    }

    #[test]
    fn test_cleanup_expired() {
        let mut cache = DirectoryCache::new(Duration::from_millis(30));
        let loc1 = Location::Local(std::path::PathBuf::from("/test1"));
        let loc2 = Location::Local(std::path::PathBuf::from("/test2"));
        
        cache.insert(loc1.clone(), vec![create_test_entry("file1.txt", 100, false)]);
        
        // Wait a bit
        std::thread::sleep(Duration::from_millis(15));
        
        cache.insert(loc2.clone(), vec![create_test_entry("file2.txt", 200, false)]);
        
        // Wait for first entry to expire
        std::thread::sleep(Duration::from_millis(20));
        
        cache.cleanup_expired();
        
        // First entry should be removed, second should remain
        assert!(cache.get(&loc1).is_none());
        assert!(cache.get(&loc2).is_some());
    }

    #[test]
    fn test_checksum_calculation() {
        let cache = DirectoryCache::new(Duration::from_secs(30));
        let entries1 = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 200, false),
        ];
        let entries2 = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 200, false),
        ];
        let entries3 = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 300, false), // Different size
        ];
        
        let checksum1 = cache.calculate_checksum(&entries1);
        let checksum2 = cache.calculate_checksum(&entries2);
        let checksum3 = cache.calculate_checksum(&entries3);
        
        // Same entries should have same checksum
        assert_eq!(checksum1, checksum2);
        
        // Different entries should have different checksum
        assert_ne!(checksum1, checksum3);
    }

    #[test]
    fn test_verify_checksum() {
        let mut cache = DirectoryCache::new(Duration::from_secs(30));
        let location = Location::Local(std::path::PathBuf::from("/test"));
        let entries = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 200, false),
        ];
        
        cache.insert(location.clone(), entries.clone());
        
        // Same entries should verify
        assert!(cache.verify_checksum(&location, &entries));
        
        // Different entries should not verify
        let different_entries = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file3.txt", 300, false),
        ];
        assert!(!cache.verify_checksum(&location, &different_entries));
    }

    #[test]
    fn test_get_with_checksum() {
        let mut cache = DirectoryCache::new(Duration::from_secs(30));
        let location = Location::Local(std::path::PathBuf::from("/test"));
        let entries = vec![create_test_entry("file1.txt", 100, false)];
        
        cache.insert(location.clone(), entries.clone());
        
        let result = cache.get_with_checksum(&location);
        assert!(result.is_some());
        
        let (cached_entries, checksum) = result.unwrap();
        assert_eq!(cached_entries.len(), 1);
        assert_eq!(checksum, cache.calculate_checksum(&entries));
    }
}
