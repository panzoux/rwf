//! Progress dialog content.

#[derive(Debug, Clone)]
pub struct ProgressDialog {
    pub operation: String,
    pub progress: f64,
    pub details: String,
}

impl ProgressDialog {
    pub fn new(operation: String, progress: f64, details: String) -> Self {
        Self {
            operation,
            progress,
            details,
        }
    }
}
