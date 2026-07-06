//! Context menu dialog content.

use super::ContextMenuOption;

#[derive(Debug, Clone)]
pub struct ContextMenuDialog {
    pub options: Vec<ContextMenuOption>,
    pub selected_index: usize,
}

impl ContextMenuDialog {
    pub fn new(options: Vec<ContextMenuOption>) -> Self {
        Self {
            options,
            selected_index: 0,
        }
    }
}
