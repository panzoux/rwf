//! Open With picker dialog content (Phase 7.3): choose among multiple
//! ExtensionAssociation candidates that match one file's extension.

#[derive(Debug, Clone)]
pub struct OpenWithPickerDialog {
    pub path: std::path::PathBuf,
    pub candidates: Vec<crate::config::ExtensionAssociation>,
    pub selected_index: usize,
}

impl OpenWithPickerDialog {
    pub fn new(
        path: std::path::PathBuf,
        candidates: Vec<crate::config::ExtensionAssociation>,
    ) -> Self {
        Self {
            path,
            candidates,
            selected_index: 0,
        }
    }
}
