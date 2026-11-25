//! Job manager dialog content.

#[derive(Debug, Clone)]
pub struct JobManagerContent {
    pub selected_index: usize,
    /// 0=Job List, 1=Close, 2=Cancel
    pub focused_field: usize,
}

impl JobManagerContent {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            focused_field: 0,
        }
    }
}

impl Default for JobManagerContent {
    fn default() -> Self {
        Self::new()
    }
}
