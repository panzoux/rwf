//! Directory caching for fast navigation
//!
//! This module implements a cache for directory contents with TTL, checksum validation, and LRU eviction.

use super::{Location, FileEntry};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Maximum number of cached directories (LRU eviction)
const MAX_CACHE_SIZE: usize = 100;

/// Cache for recently visited directories with LRU eviction
#[derive(Debug)]
pub struct DirectoryCache {
    entries: HashMap<Location, CachedDirectory>,
    /// LRU tracking: most recently accessed locations
    access_order: Vec<Location>,
    ttl: Duration,
    /// Statistics for optimization
    hits: u64,
    misses: u64,
}

impl DirectoryCache {
    /// Create a new directory cache with the specified TTL
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            access_order: Vec::new(),
            ttl,
            hits: 0,
            misses: 0,
        }
    }
    
    /// Peek at cached file/folder counts without updating LRU order.
    /// Returns `Some((file_count, folder_count))` if the location is cached and not expired.
    pub fn peek_counts(&self, location: &Location) -> Option<(usize, usize)> {
        if let Some(cached) = self.entries.get(location) {
            if cached.timestamp.elapsed() < self.ttl {
                let files   = cached.entries.iter().filter(|e| !e.is_dir).count();
                let folders = cached.entries.iter().filter(|e| e.is_dir && e.name != "..").count();
                return Some((files, folders));
            }
        }
        None
    }

    /// Get cached directory entries if available and not expired
    pub fn get(&mut self, location: &Location) -> Option<Vec<FileEntry>> {
        if let Some(cached) = self.entries.get(location) {
            if cached.timestamp.elapsed() < self.ttl {
                // Clone entries before updating access order
                let entries = cached.entries.clone();
                
                // Update LRU: move to end (most recently used)
                self.update_access_order(location);
                self.hits += 1;
                return Some(entries);
            }
        }
        self.misses += 1;
        None
    }
    
    /// Get cached directory with checksum for validation
    pub fn get_with_checksum(&mut self, location: &Location) -> Option<(Vec<FileEntry>, u64)> {
        if let Some(cached) = self.entries.get(location) {
            if cached.timestamp.elapsed() < self.ttl {
                // Clone data before updating access order
                let entries = cached.entries.clone();
                let checksum = cached.checksum;
                
                // Update LRU: move to end (most recently used)
                self.update_access_order(location);
                self.hits += 1;
                return Some((entries, checksum));
            }
        }
        self.misses += 1;
        None
    }
    
    /// Update LRU access order
    fn update_access_order(&mut self, location: &Location) {
        // Remove from current position
        if let Some(pos) = self.access_order.iter().position(|l| l == location) {
            self.access_order.remove(pos);
        }
        // Add to end (most recently used)
        self.access_order.push(location.clone());
    }
    
    /// Evict least recently used entry if cache is full
    fn evict_lru_if_needed(&mut self) {
        if self.entries.len() >= MAX_CACHE_SIZE && !self.access_order.is_empty() {
            // Remove least recently used (first in access_order)
            let lru_location = self.access_order.remove(0);
            self.entries.remove(&lru_location);
        }
    }
    
    /// Insert directory entries into cache with checksum
    pub fn insert(&mut self, location: Location, entries: Vec<FileEntry>) {
        // Evict LRU if cache is full
        self.evict_lru_if_needed();
        
        // Optimized checksum calculation
        let checksum = Self::calculate_checksum_fast(&entries);
        let cached = CachedDirectory {
            entries,
            timestamp: Instant::now(),
            checksum,
        };
        self.entries.insert(location.clone(), cached);
        
        // Update LRU
        self.update_access_order(&location);
    }
    
    /// Invalidate cache entry for a specific location
    pub fn invalidate(&mut self, location: &Location) {
        self.entries.remove(location);
        // Remove from access order
        if let Some(pos) = self.access_order.iter().position(|l| l == location) {
            self.access_order.remove(pos);
        }
    }
    
    /// Invalidate all cache entries
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }
    
    /// Remove expired cache entries
    pub fn cleanup_expired(&mut self) {
        let expired_locations: Vec<Location> = self.entries
            .iter()
            .filter(|(_, cached)| cached.timestamp.elapsed() >= self.ttl)
            .map(|(loc, _)| loc.clone())
            .collect();
        
        for location in expired_locations {
            self.invalidate(&location);
        }
    }
    
    /// Calculate checksum for directory entries (optimized version)
    /// Uses name, size, and is_dir to detect changes
    pub fn calculate_checksum_fast(entries: &[FileEntry]) -> u64 {
        let mut hasher = DefaultHasher::new();
        
        // Hash count first for quick differentiation
        entries.len().hash(&mut hasher);
        
        // Hash only essential fields for performance
        for entry in entries {
            entry.name.hash(&mut hasher);
            entry.size.hash(&mut hasher);
            entry.is_dir.hash(&mut hasher);
        }
        hasher.finish()
    }
    
    /// Calculate checksum for directory entries (legacy method for compatibility)
    pub fn calculate_checksum(&self, entries: &[FileEntry]) -> u64 {
        Self::calculate_checksum_fast(entries)
    }
    
    /// Verify if cached directory matches current checksum
    pub fn verify_checksum(&self, location: &Location, current_entries: &[FileEntry]) -> bool {
        if let Some(cached) = self.entries.get(location) {
            let current_checksum = Self::calculate_checksum_fast(current_entries);
            return cached.checksum == current_checksum;
        }
        false
    }
    
    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.entries.len(),
            max_size: MAX_CACHE_SIZE,
            hits: self.hits,
            misses: self.misses,
            hit_rate: if self.hits + self.misses > 0 {
                self.hits as f64 / (self.hits + self.misses) as f64
            } else {
                0.0
            },
        }
    }
    
    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }
}

/// Cached directory with metadata
#[derive(Debug)]
pub struct CachedDirectory {
    pub entries: Vec<FileEntry>,
    pub timestamp: Instant,
    pub checksum: u64,
}

/// Cache statistics for monitoring performance
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
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
        // Use generous timings to avoid flakiness on Windows (timer granularity ~15ms)
        let mut cache = DirectoryCache::new(Duration::from_millis(100));
        let loc1 = Location::Local(std::path::PathBuf::from("/test1"));
        let loc2 = Location::Local(std::path::PathBuf::from("/test2"));

        cache.insert(loc1.clone(), vec![create_test_entry("file1.txt", 100, false)]);

        // Wait long enough that loc1 will expire after loc2 is inserted
        std::thread::sleep(Duration::from_millis(60));

        cache.insert(loc2.clone(), vec![create_test_entry("file2.txt", 200, false)]);

        // Wait for loc1 to expire (total ~110ms since insert) but NOT loc2 (~50ms since insert)
        std::thread::sleep(Duration::from_millis(60));

        cache.cleanup_expired();

        // First entry should be expired (inserted ~120ms ago, TTL=100ms)
        assert!(cache.get(&loc1).is_none());
        // Second entry should still be valid (inserted ~60ms ago, TTL=100ms)
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
        assert_eq!(checksum, DirectoryCache::calculate_checksum_fast(&entries));
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = DirectoryCache::new(Duration::from_secs(30));
        
        // Fill cache to max capacity
        for i in 0..MAX_CACHE_SIZE {
            let location = Location::Local(std::path::PathBuf::from(format!("/test{}", i)));
            cache.insert(location, vec![create_test_entry(&format!("file{}.txt", i), 100, false)]);
        }
        
        assert_eq!(cache.entries.len(), MAX_CACHE_SIZE);
        
        // Insert one more - should evict LRU
        let new_location = Location::Local(std::path::PathBuf::from("/test_new"));
        cache.insert(new_location.clone(), vec![create_test_entry("new.txt", 100, false)]);
        
        // Cache size should still be at max
        assert_eq!(cache.entries.len(), MAX_CACHE_SIZE);
        
        // First entry should be evicted
        let first_location = Location::Local(std::path::PathBuf::from("/test0"));
        assert!(cache.get(&first_location).is_none());
        
        // New entry should be present
        assert!(cache.get(&new_location).is_some());
    }

    #[test]
    fn test_lru_access_order() {
        let mut cache = DirectoryCache::new(Duration::from_secs(30));
        
        let loc1 = Location::Local(std::path::PathBuf::from("/test1"));
        let loc2 = Location::Local(std::path::PathBuf::from("/test2"));
        let loc3 = Location::Local(std::path::PathBuf::from("/test3"));
        
        cache.insert(loc1.clone(), vec![create_test_entry("file1.txt", 100, false)]);
        cache.insert(loc2.clone(), vec![create_test_entry("file2.txt", 200, false)]);
        cache.insert(loc3.clone(), vec![create_test_entry("file3.txt", 300, false)]);
        
        // Access loc1 to make it most recently used
        cache.get(&loc1);
        
        // Access order should be: loc2, loc3, loc1
        assert_eq!(cache.access_order.len(), 3);
        assert_eq!(cache.access_order[2], loc1);
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = DirectoryCache::new(Duration::from_secs(30));
        let location = Location::Local(std::path::PathBuf::from("/test"));
        let entries = vec![create_test_entry("file1.txt", 100, false)];
        
        cache.insert(location.clone(), entries);
        
        // Hit
        cache.get(&location);
        
        // Miss
        let other_location = Location::Local(std::path::PathBuf::from("/other"));
        cache.get(&other_location);
        
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.5);
        assert_eq!(stats.size, 1);
        assert_eq!(stats.max_size, MAX_CACHE_SIZE);
    }

    #[test]
    fn test_optimized_checksum() {
        let entries = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file2.txt", 200, false),
            create_test_entry("dir1", 0, true),
        ];
        
        // Calculate checksum multiple times - should be consistent
        let checksum1 = DirectoryCache::calculate_checksum_fast(&entries);
        let checksum2 = DirectoryCache::calculate_checksum_fast(&entries);
        assert_eq!(checksum1, checksum2);
        
        // Different entries should have different checksum
        let different_entries = vec![
            create_test_entry("file1.txt", 100, false),
            create_test_entry("file3.txt", 300, false),
        ];
        let checksum3 = DirectoryCache::calculate_checksum_fast(&different_entries);
        assert_ne!(checksum1, checksum3);
    }
}
