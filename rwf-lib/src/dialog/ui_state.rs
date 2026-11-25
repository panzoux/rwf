//! Shared UI interaction state for single-line text-input dialogs.

/// Cursor/scroll/button-focus state shared by dialogs that are a single text
/// input plus OK/Cancel buttons (`FileMask`, `WildcardMark`, `SimpleRename`).
#[derive(Debug, Clone, Default)]
pub struct DialogUiState {
    pub cursor_pos: usize,
    pub scroll_pos: usize,
    /// 0=textbox (default), 1=OK, 2=Cancel
    pub focused_field: usize,
}

impl DialogUiState {
    /// New state with the cursor placed at `cursor_pos` (scroll/focus start at 0).
    pub fn new(cursor_pos: usize) -> Self {
        Self {
            cursor_pos,
            scroll_pos: 0,
            focused_field: 0,
        }
    }
}
