//! Simple rename dialog content.

use super::DialogUiState;

#[derive(Debug, Clone)]
pub struct SimpleRenameDialog {
    pub input: String,
    pub ui: DialogUiState,
}

impl SimpleRenameDialog {
    /// New dialog prefilled with the current filename (cursor placed at the end).
    pub fn new(current_name: String) -> Self {
        let cursor_pos = current_name.chars().count();
        Self {
            input: current_name,
            ui: DialogUiState::new(cursor_pos),
        }
    }
}
