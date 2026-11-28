//! File mask filter dialog content.

use super::DialogUiState;

#[derive(Debug, Clone)]
pub struct FileMaskDialog {
    /// Current pattern text being edited
    pub input: String,
    pub ui: DialogUiState,
}

impl FileMaskDialog {
    pub fn new(input: String) -> Self {
        let cursor_pos = input.chars().count();
        Self {
            input,
            ui: DialogUiState::new(cursor_pos),
        }
    }
}
