//! Jump to File dialog content.

use crate::job::JobId;

#[derive(Debug, Clone)]
pub struct JumpToFileDialog {
    /// Current search query
    pub query: String,
    /// Cursor position in the query (in character count, not bytes)
    pub cursor_pos: usize,
    /// Horizontal scroll position for the query field
    pub scroll_pos: usize,
    /// Fast candidates shown immediately; async job appends disk-walk results
    pub candidates: Vec<String>,
    /// Current AND-filtered subset of candidates
    pub suggestions: Vec<String>,
    /// Currently selected suggestion index
    pub selected_index: usize,
    /// Root path for recursive search and relative-path fallback
    pub search_root: String,
    /// Background job collecting more candidates; None when done
    pub loading_job_id: Option<JobId>,
}

impl JumpToFileDialog {
    pub fn new(search_root: String, candidates: Vec<String>) -> Self {
        let suggestions = candidates.clone();
        Self {
            query: String::new(),
            cursor_pos: 0,
            scroll_pos: 0,
            candidates,
            suggestions,
            selected_index: 0,
            search_root,
            loading_job_id: None,
        }
    }
}
