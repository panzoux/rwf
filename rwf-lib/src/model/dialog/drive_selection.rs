//! Drive selection dialog content.

use super::DriveInfo;

#[derive(Debug, Clone)]
pub struct DriveSelectionDialog {
    pub drives: Vec<DriveInfo>,
    pub selected_index: usize,
    pub filter: String,
}

impl DriveSelectionDialog {
    pub fn new(drives: Vec<DriveInfo>) -> Self {
        Self {
            drives,
            selected_index: 0,
            filter: String::new(),
        }
    }
}
