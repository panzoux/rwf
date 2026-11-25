//! Type mismatch warning dialog content (Phase 7.3, magic-byte detection).
//!
//! Shown before running an `ExtensionAssociation` command when the target
//! file's leading bytes look like an executable but the extension says
//! otherwise. Confirm proceeds with the original command; Cancel does nothing.

#[derive(Debug, Clone)]
pub struct TypeMismatchWarningDialog {
    pub path: std::path::PathBuf,
    pub detected: crate::magic::DetectedKind,
    pub command: String,
    pub working_dir: crate::model::Location,
    pub shell: Option<String>,
}

impl TypeMismatchWarningDialog {
    pub fn new(
        path: std::path::PathBuf,
        detected: crate::magic::DetectedKind,
        command: String,
        working_dir: crate::model::Location,
        shell: Option<String>,
    ) -> Self {
        Self {
            path,
            detected,
            command,
            working_dir,
            shell,
        }
    }
}
