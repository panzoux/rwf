//! Drive selection dialog content.

use super::DriveInfo;
use crate::job::JobId;

#[derive(Debug, Clone)]
pub struct DriveSelectionDialog {
    pub drives: Vec<DriveInfo>,
    pub selected_index: usize,
    pub filter: String,
    /// The `ListDrives` job whose drives are still to be appended; `None` once they
    /// have arrived (or for a dialog built with its full list).
    pub loading_job_id: Option<JobId>,
}

impl DriveSelectionDialog {
    pub fn new(drives: Vec<DriveInfo>) -> Self {
        Self {
            drives,
            selected_index: 0,
            filter: String::new(),
            loading_job_id: None,
        }
    }
}
