//! File comparison view dialog content.

#[derive(Debug, Clone)]
pub struct ComparisonViewDialog {
    pub diff: crate::job::FileDiff,
    pub scroll_offset: usize,
}

impl ComparisonViewDialog {
    pub fn new(diff: crate::job::FileDiff) -> Self {
        Self {
            diff,
            scroll_offset: 0,
        }
    }
}
