//! Extraction confirmation dialog content.

#[derive(Debug, Clone)]
pub struct ExtractionConfirmDialog {
    pub archive: crate::model::Location,
    pub dest: crate::model::Location,
    pub file_count: usize,
}

impl ExtractionConfirmDialog {
    pub fn new(
        archive: crate::model::Location,
        dest: crate::model::Location,
        file_count: usize,
    ) -> Self {
        Self {
            archive,
            dest,
            file_count,
        }
    }
}
