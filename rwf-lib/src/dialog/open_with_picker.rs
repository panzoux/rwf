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
    /// The content type detection already produced before this picker was
    /// shown (Phase 7.3b, Task 9), if any. `None` when the picker was
    /// reached without a detected kind in hand (detection failed/was
    /// cancelled, or magic-byte detection is disabled) — the title falls
    /// back to the plain "Open With..." text in that case.
    pub detected_kind: Option<crate::magic::DetectedKind>,
}

impl OpenWithPickerDialog {
    pub fn new(
        paths: Vec<std::path::PathBuf>,
        candidates: Vec<crate::config::ExtensionAssociation>,
        detected_kind: Option<crate::magic::DetectedKind>,
    ) -> Self {
        Self {
            paths,
            candidates,
            selected_index: 0,
            detected_kind,
        }
    }
}
