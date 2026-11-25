//! File conflict resolution dialog content.

use super::{ConflictAction, ConflictPair};

#[derive(Debug, Clone)]
pub struct FileConflictDialog {
    pub conflicts: Vec<ConflictPair>,
    pub current_index: usize,
    /// 0=Force, 1=OverwriteIfNew, 2=Skip, 3=Rename (Textbox), 4=Cancel
    pub focused_button: usize,
    pub rename_text: String,
    pub rename_cursor: usize,
    pub rename_scroll: usize,
    /// Emacs or Vi mode for textbox
    pub edit_mode: crate::config::EditMode,
    /// None = Emacs, Some = Vi mode state
    pub vi_mode: Option<crate::config::ViMode>,
    pub decisions: Vec<ConflictAction>,
    pub error_message: Option<String>,
    // Vi pending states (persisted between key presses)
    pub vi_pending_find_backward: Option<bool>,
    pub vi_pending_operator: Option<u8>, // 0=none, 1=change, 2=delete
    pub vi_pending_ctrl_x: bool,
    // Undo/Redo history
    pub history: Vec<String>,
    pub history_index: usize,
    /// "Copy" or "Move" — used in the dialog title
    pub operation: String,
}

impl FileConflictDialog {
    pub fn new(
        conflicts: Vec<ConflictPair>,
        current_index: usize,
        edit_mode: crate::config::EditMode,
        op_name: &str,
    ) -> Self {
        let rename_text = if !conflicts.is_empty() {
            conflicts[current_index].source.name.clone()
        } else {
            String::new()
        };
        let rename_cursor = if !conflicts.is_empty() {
            conflicts[current_index].source.name.len()
        } else {
            0
        };
        // Initialize vi_mode based on edit_mode
        let vi_mode = if edit_mode == crate::config::EditMode::Vi {
            Some(crate::config::ViMode::Normal)
        } else {
            None
        };
        Self {
            conflicts,
            current_index,
            focused_button: 3, // Rename button focused by default
            rename_text: rename_text.clone(),
            rename_cursor,
            rename_scroll: 0,
            edit_mode,
            vi_mode,
            decisions: Vec::new(),
            error_message: None,
            vi_pending_find_backward: None,
            vi_pending_operator: None,
            vi_pending_ctrl_x: false,
            history: vec![rename_text],
            history_index: 0,
            operation: op_name.to_string(),
        }
    }
}
