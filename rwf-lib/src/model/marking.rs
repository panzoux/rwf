//! File marking model

use std::collections::HashSet;
use super::{Location, FileEntry};

/// Manages file marking state
#[derive(Debug)]
pub struct MarkingModel {
    pub marked_locations: HashSet<Location>,
}

impl Default for MarkingModel {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkingModel {
    pub fn new() -> Self {
        Self {
            marked_locations: HashSet::new(),
        }
    }
    
    /// Toggle mark state
    pub fn toggle(&mut self, location: Location) {
        if self.marked_locations.contains(&location) {
            self.marked_locations.remove(&location);
        } else {
            self.marked_locations.insert(location);
        }
    }
    
    /// Mark a location
    pub fn mark(&mut self, location: Location) {
        self.marked_locations.insert(location);
    }
    
    /// Unmark a location
    pub fn unmark(&mut self, location: Location) {
        self.marked_locations.remove(&location);
    }
    
    /// Mark all entries
    pub fn mark_all(&mut self, entries: &[FileEntry]) {
        for entry in entries {
            self.marked_locations.insert(entry.location.clone());
        }
    }
    
    /// Unmark all
    pub fn unmark_all(&mut self) {
        self.marked_locations.clear();
    }
    
    /// Check if location is marked
    pub fn is_marked(&self, location: &Location) -> bool {
        self.marked_locations.contains(location)
    }
    
    /// Get count of marked files
    pub fn count(&self) -> usize {
        self.marked_locations.len()
    }
    
    /// Calculate total size of marked files
    pub fn total_size(&self, entries: &[FileEntry]) -> u64 {
        entries.iter()
            .filter(|e| self.is_marked(&e.location))
            .map(|e| e.size)
            .sum()
    }
    
    /// Mark files matching a wildcard pattern
    pub fn mark_pattern(&mut self, entries: &[FileEntry], pattern: &str) {
        for entry in entries {
            if matches_wildcard(&entry.name, pattern) {
                self.marked_locations.insert(entry.location.clone());
            }
        }
    }
    
    /// Mark a range of entries by index
    pub fn mark_range(&mut self, entries: &[FileEntry], start: usize, end: usize) {
        let start = start.min(entries.len());
        let end = end.min(entries.len());
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        for entry in &entries[start..=end] {
            self.marked_locations.insert(entry.location.clone());
        }
    }
    
    /// Invert marks for entries in the active pane
    pub fn invert_marks(&mut self, entries: &[FileEntry]) {
        for entry in entries {
            if self.marked_locations.contains(&entry.location) {
                self.marked_locations.remove(&entry.location);
            } else {
                self.marked_locations.insert(entry.location.clone());
            }
        }
    }
}

/// Match a filename against a wildcard pattern
/// Supports * (any characters) and ? (single character)
fn matches_wildcard(name: &str, pattern: &str) -> bool {
    let name_chars: Vec<char> = name.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    
    matches_wildcard_impl(&name_chars, &pattern_chars, 0, 0)
}

fn matches_wildcard_impl(name: &[char], pattern: &[char], name_idx: usize, pattern_idx: usize) -> bool {
    // If we've consumed both strings, it's a match
    if pattern_idx >= pattern.len() && name_idx >= name.len() {
        return true;
    }
    
    // If pattern is exhausted but name isn't, no match
    if pattern_idx >= pattern.len() {
        return false;
    }
    
    // If name is exhausted, pattern must be all '*' to match
    if name_idx >= name.len() {
        return pattern[pattern_idx..].iter().all(|&c| c == '*');
    }
    
    match pattern[pattern_idx] {
        '*' => {
            // Try matching zero or more characters
            // First try matching zero characters (skip the *)
            if matches_wildcard_impl(name, pattern, name_idx, pattern_idx + 1) {
                return true;
            }
            // Then try matching one or more characters
            matches_wildcard_impl(name, pattern, name_idx + 1, pattern_idx)
        }
        '?' => {
            // Match any single character
            matches_wildcard_impl(name, pattern, name_idx + 1, pattern_idx + 1)
        }
        c => {
            // Match exact character (case-insensitive)
            if name[name_idx].to_lowercase().eq(c.to_lowercase()) {
                matches_wildcard_impl(name, pattern, name_idx + 1, pattern_idx + 1)
            } else {
                false
            }
        }
    }
}
