//! Error dialog content.

use super::ErrorType;

#[derive(Debug, Clone)]
pub struct ErrorDialog {
    pub message: String,
    pub details: Option<String>,
    pub error_type: ErrorType,
    pub focused_button: usize,
}

impl ErrorDialog {
    pub fn new(message: String, details: Option<String>, error_type: ErrorType) -> Self {
        Self {
            message,
            details,
            error_type,
            focused_button: 0,
        }
    }
}
