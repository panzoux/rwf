//! Pane model and display modes

use super::{Location, FileEntry};
use std::path::Path;
use regex;

/// Represents the state of a single pane
#[derive(Debug)]
pub struct PaneModel {
    pub current_location: Location,
    pub entries: Vec<FileEntry>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub sort_mode: SortMode,
    pub display_mode: DisplayMode,
    pub file_mask: Option<String>,
}

impl PaneModel {
    pub fn new(location: Location) -> Self {
        Self {
            current_location: location,
            entries: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            sort_mode: SortMode::Name,
            display_mode: DisplayMode::Detailed,
            file_mask: None,
        }
    }
    
    /// Get the current entry under cursor
    pub fn current_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.cursor)
    }
    
    /// Get all marked entries
    pub fn marked_entries(&self) -> Vec<&FileEntry> {
        self.entries.iter().filter(|e| e.marked).collect()
    }
    
    /// Apply current sort mode to entries
    pub fn apply_sort(&mut self) {
        self.entries.sort_by(|a, b| {
            // Directories always come first
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => match self.sort_mode {
                    SortMode::Name => a.name.cmp(&b.name),
                    SortMode::Size => a.size.cmp(&b.size),
                    SortMode::Date => a.modified.cmp(&b.modified),
                    SortMode::Extension => {
                        let ext_a = Path::new(&a.name).extension().and_then(|s| s.to_str()).unwrap_or("");
                        let ext_b = Path::new(&b.name).extension().and_then(|s| s.to_str()).unwrap_or("");
                        ext_a.cmp(ext_b)
                    }
                }
            }
        });
    }
    
    /// Apply file mask filter to entries
    /// Filters entries based on wildcard pattern (* and ?)
    pub fn apply_filter(&mut self, mask: &str) {
        if mask.is_empty() {
            return;
        }
        
        let pattern = wildcard_to_regex(mask);
        if let Ok(re) = regex::Regex::new(&pattern) {
            self.entries.retain(|entry| {
                // Always show directories
                entry.is_dir || re.is_match(&entry.name)
            });
        }
    }
    
    /// Get filtered entries based on current file mask
    /// Returns all entries if no mask is set, otherwise returns only matching entries
    pub fn get_filtered_entries(&self) -> Vec<&FileEntry> {
        if let Some(ref mask) = self.file_mask {
            if !mask.is_empty() {
                let pattern = wildcard_to_regex(mask);
                if let Ok(re) = regex::Regex::new(&pattern) {
                    return self.entries.iter()
                        .filter(|entry| {
                            // Always show directories
                            entry.is_dir || re.is_match(&entry.name)
                        })
                        .collect();
                }
            }
        }
        
        // No filter or invalid pattern - return all entries
        self.entries.iter().collect()
    }
    
    /// Apply the current file mask filter to entries (modifies entries in place)
    pub fn apply_current_filter(&mut self) {
        if let Some(ref mask) = self.file_mask.clone() {
            self.apply_filter(mask);
        }
    }
}

/// Convert wildcard pattern to regex pattern
fn wildcard_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }
    regex.push('$');
    regex
}

/// Sort mode for file entries
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortMode {
    Name,
    Size,
    Date,
    Extension,
}

/// Display mode for pane
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayMode {
    Columns(u8), // 1-8 columns
    Detailed,    // Full metadata view
}
