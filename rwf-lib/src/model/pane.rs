//! Pane model and display modes

use super::{Location, FileEntry, MarkingModel};
use std::path::Path;
use regex;

/// Represents the state of a single pane
#[derive(Debug)]
pub struct PaneModel {
    pub current_location: Location,
    /// Canonical sorted unfiltered directory contents; set on every ReadDirectory completion.
    /// Empty until the first ReadDirectory result arrives.
    pub raw_entries: Vec<FileEntry>,
    pub entries: Vec<FileEntry>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub sort_mode: SortMode,
    pub sort_order: SortOrder,
    pub display_mode: DisplayMode,
    pub file_mask: Option<String>,
    pub is_loading: bool,
    pub active_job_id: Option<crate::job::JobId>,
    pub marking: MarkingModel,
}

impl PaneModel {
    pub fn new(location: Location) -> Self {
        Self {
            current_location: location,
            raw_entries: Vec::new(),
            entries: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            sort_mode: SortMode::Name,
            sort_order: SortOrder::Ascending,
            display_mode: DisplayMode::Detailed,
            file_mask: None,
            is_loading: false,
            active_job_id: None,
            marking: MarkingModel::new(),
        }
    }

    pub fn set_loading(&mut self, loading: bool) {
        if self.is_loading != loading {
            tracing::info!("[PaneModel] is_loading changed from {} to {} for pane at {:p}", self.is_loading, loading, self as *const _);
        }
        self.is_loading = loading;
    }
    
    /// Get the current entry under cursor
    pub fn current_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.cursor)
    }
    
    /// Get all marked entries
    pub fn marked_entries(&self) -> Vec<&FileEntry> {
        self.entries.iter().filter(|e| e.marked).collect()
    }
    
    /// Apply current sort mode and order to entries (and raw_entries if populated)
    pub fn apply_sort(&mut self) {
        let mode = self.sort_mode;
        let order = self.sort_order;
        self.entries.sort_by(|a, b| cmp_entries(a, b, mode, order));
        if !self.raw_entries.is_empty() {
            self.raw_entries.sort_by(|a, b| cmp_entries(a, b, mode, order));
        }
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
    
    /// Apply the current file mask filter to entries (modifies entries in place).
    /// Restores from raw_entries first so re-applying a mask never double-filters.
    pub fn apply_current_filter(&mut self) {
        if !self.raw_entries.is_empty() {
            self.entries = self.raw_entries.clone();
        }
        if let Some(mask) = self.file_mask.clone() {
            if !mask.is_empty() && mask != "*" {
                self.apply_filter(&mask);
            }
        }
    }

    /// Update scroll offset based on cursor position and visible height
    pub fn update_scroll(&mut self, visible_height: usize, scroll_margin: usize) {
        if self.entries.is_empty() {
            self.scroll_offset = 0;
            return;
        }

        // Clamp cursor to entry bounds
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));

        // If all entries fit in visible area, no scrolling needed
        if self.entries.len() <= visible_height {
            self.scroll_offset = 0;
            return;
        }

        // 1. Ensure cursor is at least visible (handle large jumps)
        if self.cursor < self.scroll_offset {
            // Cursor is above visible area
            self.scroll_offset = self.cursor.saturating_sub(scroll_margin);
        } else if self.cursor >= self.scroll_offset + visible_height {
            // Cursor is below visible area
            let max_offset = self.entries.len().saturating_sub(visible_height);
            let desired_offset = self.cursor + scroll_margin + 1 - visible_height;
            self.scroll_offset = desired_offset.min(max_offset);
        }

        // 2. Apply smooth scrolling logic (maintain margin)
        let cursor_in_view = self.cursor.saturating_sub(self.scroll_offset);
        
        // Scroll UP if cursor too close to top
        if cursor_in_view < scroll_margin && self.cursor > 0 {
            self.scroll_offset = self.cursor.saturating_sub(scroll_margin);
        }
        // Scroll DOWN if cursor too close to bottom
        else if visible_height > scroll_margin {
            let bottom_trigger = visible_height.saturating_sub(scroll_margin + 1);
            let max_offset = self.entries.len().saturating_sub(visible_height);
            
            // Check if we're in the "end zone" where scroll_margin can't be maintained
            let end_zone_start = self.entries.len().saturating_sub(scroll_margin + 1);
            
            if self.cursor >= end_zone_start {
                // Near the end - just set scroll to max_offset to avoid blank lines
                self.scroll_offset = max_offset;
            } else if cursor_in_view > bottom_trigger {
                // Normal scrolling - maintain scroll_margin
                let desired_offset = self.cursor.saturating_sub(bottom_trigger);
                self.scroll_offset = desired_offset.min(max_offset);
            }
        }
    }
}

fn cmp_entries(a: &FileEntry, b: &FileEntry, mode: SortMode, order: SortOrder) -> std::cmp::Ordering {
    match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => {
            let base = match mode {
                SortMode::Name => a.name.cmp(&b.name),
                SortMode::Size => a.size.cmp(&b.size),
                SortMode::Date => a.modified.cmp(&b.modified),
                SortMode::Extension => {
                    let ext_a = Path::new(&a.name).extension().and_then(|s| s.to_str()).unwrap_or("");
                    let ext_b = Path::new(&b.name).extension().and_then(|s| s.to_str()).unwrap_or("");
                    ext_a.cmp(ext_b)
                }
            };
            if order == SortOrder::Descending { base.reverse() } else { base }
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

/// Sort order (ascending or descending)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggle(self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }
}

/// Display mode for pane
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayMode {
    Columns(u8), // 1-8 columns
    Detailed,    // Full metadata view
}
