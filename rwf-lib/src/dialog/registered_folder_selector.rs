//! Registered folder selector dialog content.

use super::RegisteredFolder;

#[derive(Debug, Clone)]
pub struct RegisteredFolderSelectorContent {
    pub folders: Vec<RegisteredFolder>,
    pub filter: String,
    pub selected_index: usize,
}

impl RegisteredFolderSelectorContent {
    pub fn new(folders: Vec<RegisteredFolder>) -> Self {
        Self {
            folders,
            filter: String::new(),
            selected_index: 0,
        }
    }
}
