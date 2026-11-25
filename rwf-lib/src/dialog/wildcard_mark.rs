//! Wildcard marking dialog content.

use super::DialogUiState;

#[derive(Debug, Clone)]
pub struct WildcardMarkDialog {
    pub input: String,
    pub ui: DialogUiState,
}

impl WildcardMarkDialog {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            ui: DialogUiState::new(0),
        }
    }
}

impl Default for WildcardMarkDialog {
    fn default() -> Self {
        Self::new()
    }
}
