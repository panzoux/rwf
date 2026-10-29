//! Extraction confirmation dialog

use ratatui::{layout::Rect, widgets::Paragraph, Frame};

use super::common::DIALOG_TEXT;

/// Extraction confirmation dialog
#[derive(Debug, Clone)]
pub struct ExtractionConfirmDialog {
    pub archive_name: String,
    pub dest_path: String,
    pub file_count: usize,
}

impl ExtractionConfirmDialog {
    /// Render the extraction confirmation dialog
    pub fn render(&self, frame: &mut Frame, area: Rect, _focused: bool) {
        let message = format!(
            "Extract '{}' ({} files) to:\n\n{}",
            self.archive_name, self.file_count, self.dest_path
        );

        let paragraph = Paragraph::new(message).style(DIALOG_TEXT);

        frame.render_widget(paragraph, area);
    }
}
