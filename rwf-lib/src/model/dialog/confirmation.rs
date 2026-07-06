//! Confirmation dialog content.

#[derive(Debug, Clone)]
pub struct ConfirmationDialog {
    pub message: String,
}

impl ConfirmationDialog {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}
