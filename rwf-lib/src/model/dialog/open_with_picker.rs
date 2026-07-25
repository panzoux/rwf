//! Open With picker dialog content (Phase 7.3): choose among multiple
//! ExtensionAssociation candidates that match one file's extension.
//!
//! `paths` holds the file(s) the chosen candidate will run against: a single
//! file for the ordinary cursor-file flow, or a group of marked files sharing
//! the same (DetectedKind, extension) pair for the batch "Open With..." flow
//! (Phase 7.3 §3, multi-select).

#[derive(Debug, Clone)]
pub struct OpenWithPickerDialog {
    pub paths: Vec<std::path::PathBuf>,
    pub candidates: Vec<crate::config::ExtensionAssociation>,
    pub selected_index: usize,
}

impl OpenWithPickerDialog {
    pub fn new(
        paths: Vec<std::path::PathBuf>,
        candidates: Vec<crate::config::ExtensionAssociation>,
    ) -> Self {
        Self {
            paths,
            candidates,
            selected_index: 0,
        }
    }
}
