//! Test-only helpers for dialog input handling.
//!
//! `handle_file_conflict_input` threads 13 mutable state variables through
//! every call; declaring them all per test buries the intent. Tests build a
//! [`ConflictInputHarness`] instead, override the fields they care about, and
//! feed keys via [`ConflictInputHarness::send`].
//!
//! Migration of the existing 50+ dialog tests to this harness happens in M3.

use crossterm::event::KeyEvent;
use rwf_lib::config::{EditMode, ViMode};
use rwf_lib::model::dialog::{ConflictAction, ConflictPair};

use super::file_conflict::handle_file_conflict_input;
use super::DialogAction;

/// Bundles the mutable state threaded through `handle_file_conflict_input`.
///
/// `new` initializes the same way the existing tests do: cursor at end of the
/// rename text, Force button focused, Emacs edit mode, rename history seeded
/// with the initial text.
pub struct ConflictInputHarness {
    pub conflicts: Vec<ConflictPair>,
    pub current_index: usize,
    pub focused_button: usize,
    pub rename_text: String,
    pub rename_cursor: usize,
    pub rename_scroll: usize,
    pub edit_mode: EditMode,
    pub vi_mode: Option<ViMode>,
    pub error_message: Option<String>,
    pub decisions: Vec<ConflictAction>,
    pub pending_find_backward: Option<bool>,
    pub pending_operator: Option<u8>,
    pub pending_ctrl_x: bool,
    pub history: Vec<String>,
    pub history_index: usize,
}

impl ConflictInputHarness {
    pub fn new(conflicts: Vec<ConflictPair>) -> Self {
        let rename_text = conflicts
            .first()
            .map(|c| c.dest.name.clone())
            .unwrap_or_default();
        Self {
            conflicts,
            current_index: 0,
            focused_button: 0,
            rename_cursor: rename_text.len(),
            rename_scroll: 0,
            edit_mode: EditMode::Emacs,
            vi_mode: None,
            error_message: None,
            decisions: Vec::new(),
            pending_find_backward: None,
            pending_operator: None,
            pending_ctrl_x: false,
            history: vec![rename_text.clone()],
            history_index: 0,
            rename_text,
        }
    }

    /// Feed one key event through `handle_file_conflict_input`.
    pub fn send(&mut self, key: KeyEvent) -> DialogAction {
        handle_file_conflict_input(
            &mut self.conflicts,
            &mut self.current_index,
            &mut self.focused_button,
            &mut self.rename_text,
            &mut self.rename_cursor,
            &mut self.rename_scroll,
            &mut self.edit_mode,
            &mut self.vi_mode,
            &mut self.error_message,
            &mut self.decisions,
            &mut self.pending_find_backward,
            &mut self.pending_operator,
            &mut self.pending_ctrl_x,
            &mut self.history,
            &mut self.history_index,
            key,
        )
    }
}
