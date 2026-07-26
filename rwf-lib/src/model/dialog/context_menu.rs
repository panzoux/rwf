//! Context menu dialog content.

use super::ContextMenuOption;

#[derive(Debug, Clone)]
pub struct ContextMenuDialog {
    pub options: Vec<ContextMenuOption>,
    pub selected_index: usize,
    /// Display string for the live-detected content type of the cursor entry
    /// (Phase 7.3b, Task 9), shown appended to the "Open With..." row once
    /// known — e.g. `Some("PNG image")`. `None` before detection completes
    /// (or when detection never ran: flag off, non-Local entry, or a
    /// directory under the cursor).
    pub detected_type_label: Option<String>,
    /// Set while a `DetectFileType { ContextMenuLabel }` job is in flight for
    /// this dialog; `is_some()` IS the "detecting..." signal — no separate
    /// bool needed, unlike `FileInfoDialog` (kept as 2 fields here since
    /// there's nothing else that needs a distinct "detecting" state).
    pub detected_type_job_id: Option<crate::job::JobId>,
}

impl ContextMenuDialog {
    pub fn new(options: Vec<ContextMenuOption>) -> Self {
        Self {
            options,
            selected_index: 0,
            detected_type_label: None,
            detected_type_job_id: None,
        }
    }
}
