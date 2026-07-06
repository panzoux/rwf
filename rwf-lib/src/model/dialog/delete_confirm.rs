//! Delete confirmation dialog content.

#[derive(Debug, Clone)]
pub struct DeleteConfirmDialog {
    /// (location, is_dir) pairs to be deleted
    pub targets: Vec<(crate::model::Location, bool)>,
    pub scroll_offset: usize,
}

impl DeleteConfirmDialog {
    pub fn new(targets: Vec<(crate::model::Location, bool)>) -> Self {
        Self {
            targets,
            scroll_offset: 0,
        }
    }
}
