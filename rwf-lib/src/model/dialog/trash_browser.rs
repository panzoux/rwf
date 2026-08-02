//! Trash browser dialog content (Phase 7.7 Task 16): lists trashed items
//! (populated by a completed `JobKind::ListTrash`) so the user can select one
//! and restore it to its original location.

#[derive(Debug, Clone)]
pub struct TrashBrowserDialog {
    pub records: Vec<crate::model::TrashRecord>,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

impl TrashBrowserDialog {
    pub fn new(records: Vec<crate::model::TrashRecord>) -> Self {
        Self {
            records,
            selected_index: 0,
            scroll_offset: 0,
        }
    }
}
