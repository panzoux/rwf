//! Pane model and display modes

use super::{FileEntry, Location, MarkingModel};
use regex;
use std::collections::HashMap;
use std::path::Path;

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
    /// After JumpToFile navigation: filename to select once ReadDirectory completes.
    pub pending_cursor_name: Option<String>,
    /// Phase 7.5 D12: bumped whenever the listing changes (a read that returned a
    /// different listing, or an in-memory rename/delete). A background read submitted
    /// under an older value must not overwrite what the pane shows now.
    pub listing_generation: u64,
    /// Phase 7.5 D8b: the `listing_generation` at which each calculated directory size
    /// was measured. A size whose generation is behind the pane's is stale.
    pub size_generations: HashMap<Location, u64>,
}

impl PaneModel {
    /// Create a new pane viewing the specified location
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
            pending_cursor_name: None,
            listing_generation: 0,
            size_generations: HashMap::new(),
        }
    }

    /// Set the loading state (emits debug trace when state changes)
    pub fn set_loading(&mut self, loading: bool) {
        if self.is_loading != loading {
            tracing::info!(
                "[PaneModel] is_loading changed from {} to {} for pane at {:p}",
                self.is_loading,
                loading,
                self as *const _
            );
        }
        self.is_loading = loading;
    }

    /// Whether `entry`'s calculated size was measured before the listing last changed.
    pub fn is_size_stale(&self, entry: &FileEntry) -> bool {
        entry.calculated_size.is_some()
            && self
                .size_generations
                .get(&entry.location)
                .is_some_and(|measured| *measured < self.listing_generation)
    }

    /// Record that what the pane lists has changed (Phase 7.5 D12).
    pub fn mark_listing_changed(&mut self) {
        self.listing_generation = self.listing_generation.wrapping_add(1);
    }

    /// Store a calculated directory size on the entry at `location`, measured now.
    /// Returns whether this pane lists that entry.
    pub fn set_calculated_size(&mut self, location: &Location, size: u64) -> bool {
        let mut found = false;
        for entry in self
            .raw_entries
            .iter_mut()
            .chain(self.entries.iter_mut())
            .filter(|e| e.location == *location)
        {
            entry.calculated_size = Some(size);
            found = true;
        }
        if found {
            self.size_generations
                .insert(location.clone(), self.listing_generation);
        }
        found
    }

    /// Apply a completed read of `location` (Phase 7.5 T1). Returns whether what the pane
    /// shows changed.
    ///
    /// Calculated sizes are carried over by `Location` before comparing, so a read that
    /// only lacks them is not a change (D8b). On a change the cursor stays on the file it
    /// was on and keeps its screen row (D7); if that file is gone the old index is kept,
    /// clamped. An explicit `pending_cursor_name` still wins.
    pub fn apply_directory_listing(
        &mut self,
        location: &Location,
        mut incoming: Vec<FileEntry>,
        visible_height: usize,
        scroll_margin: usize,
    ) -> bool {
        let carried: HashMap<&Location, u64> = self
            .raw_entries
            .iter()
            .filter_map(|e| e.calculated_size.map(|size| (&e.location, size)))
            .collect();
        if !carried.is_empty() {
            for entry in incoming.iter_mut() {
                if entry.calculated_size.is_none() {
                    entry.calculated_size = carried.get(&entry.location).copied();
                }
            }
        }
        let (mode, order) = (self.sort_mode, self.sort_order);
        incoming.sort_by(|a, b| cmp_entries(a, b, mode, order));

        self.is_loading = false;
        let changed = self.raw_entries != incoming;
        // Re-entering the directory already shown clears `entries` but not `raw_entries`,
        // so an identical read can still have something to show.
        let needs_refill = self.entries.is_empty() && !incoming.is_empty();
        if !changed && !needs_refill {
            self.pending_cursor_name = None;
            return false;
        }

        // Only a cursor on an entry of the directory just read can be "its file".
        let anchor = self
            .current_entry()
            .filter(|e| e.location.parent().as_ref() == Some(location))
            .map(|e| {
                (
                    e.name.clone(),
                    self.cursor.saturating_sub(self.scroll_offset),
                )
            });

        // `apply_current_filter` leaves `entries` alone when `raw_entries` is empty, so
        // an emptied directory must be copied in explicitly.
        self.entries = incoming.clone();
        self.raw_entries = incoming;
        self.apply_current_filter();
        if changed {
            self.mark_listing_changed();
            let listed: std::collections::HashSet<&Location> =
                self.raw_entries.iter().map(|e| &e.location).collect();
            self.size_generations.retain(|loc, _| listed.contains(loc));
        }

        if let Some(name) = self.pending_cursor_name.take() {
            if let Some(pos) = self.entries.iter().position(|e| e.name == name) {
                self.cursor = pos;
            }
        } else if let Some((name, row)) = anchor {
            if let Some(pos) = self.entries.iter().position(|e| e.name == name) {
                self.cursor = pos;
                let max_offset = self.entries.len().saturating_sub(visible_height);
                self.scroll_offset = pos.saturating_sub(row).min(max_offset);
            }
        }
        self.update_scroll(visible_height, scroll_margin);
        true
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
            self.raw_entries
                .sort_by(|a, b| cmp_entries(a, b, mode, order));
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
                    return self
                        .entries
                        .iter()
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

fn cmp_entries(
    a: &FileEntry,
    b: &FileEntry,
    mode: SortMode,
    order: SortOrder,
) -> std::cmp::Ordering {
    match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => {
            let base = match mode {
                SortMode::Name => a.name.cmp(&b.name),
                SortMode::Size => a.size.cmp(&b.size),
                SortMode::Date => a.modified.cmp(&b.modified),
                SortMode::Extension => {
                    let ext_a = Path::new(&a.name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    let ext_b = Path::new(&b.name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    ext_a.cmp(ext_b)
                }
            };
            if order == SortOrder::Descending {
                base.reverse()
            } else {
                base
            }
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
    /// Reverse the sort order (Ascending ↔ Descending)
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
