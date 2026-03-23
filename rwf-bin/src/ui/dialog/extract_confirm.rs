//! Extraction confirmation dialog

use super::{DialogAction, DialogContentRenderer};
use crossterm::event::KeyEvent;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

/// Extraction confirmation dialog
#[derive(Debug, Clone)]
pub struct ExtractionConfirmDialog {
    pub archive_name: String,
    pub dest_path: String,
    pub file_count: usize,
}

impl ExtractionConfirmDialog {
    pub fn new(archive_name: String, dest_path: String, file_count: usize) -> Self {
        Self {
            archive_name,
            dest_path,
            file_count,
        }
    }
}

impl DialogContentRenderer for ExtractionConfirmDialog {
    fn render(&self, frame: &mut Frame, area: Rect, _focused: bool) {
        // Simple message: "Extract 'archive.zip' to C:\path\?"
        let message = format!(
            "Extract '{}' ({} files) to:\n\n{}",
            self.archive_name,
            self.file_count,
            self.dest_path
        );
        
        let paragraph = Paragraph::new(message)
            .style(Style::default().fg(Color::White));
        
        frame.render_widget(paragraph, area);
    }
    
    fn handle_input(&mut self, _key: KeyEvent) -> DialogAction {
        DialogAction::None  // Only global shortcuts (Enter/Esc) apply
    }
    
    fn set_focused_field(&mut self, _index: usize) {
        // No focusable fields in confirmation dialog
    }
}
