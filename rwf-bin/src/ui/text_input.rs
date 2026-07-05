//! Reusable single-line text input widget with CJK support
//!
//! Features:
//! - CJK-aware cursor positioning (Japanese, Chinese, Korean)
//! - Emacs and Vi keybindings (configurable)
//! - Internal kill buffer (clipboard)
//! - Horizontal scrolling for long text
//! - Visible cursor with proper width calculation
//! - Undo/Redo history (100 levels)
//!
//! ## Usage Rules
//! 1. **Initialization**: Use `TextInput::new(text, mode)` and `set_original_text(text)` to enable Vi 'U' command.
//! 2. **State Persistence**: If the widget is reconstructed every frame (e.g. in a dynamic dialog),
//!    certain internal states MUST be persisted externally and restored:
//!    - `vi_mode`, `pending_operator`, `pending_find_backward`, `pending_ctrl_x`
//!    - `history` and `history_index` (if undo/redo must persist)
//! 3. **Rendering**: Call `set_width(area.width)` before `render()` to ensure proper scrolling.
//!
//! See `docs/TEXT_INPUT_WIDGET.md` for detailed keybindings and implementation details.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rwf_lib::config::{EditMode, ViMode};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Result of text input handling
#[derive(Debug, Clone, PartialEq)]
pub enum TextInputAction {
    None,        // Input consumed, no action
    Confirm,     // Enter pressed
    Cancel,      // Escape pressed
    NextField,   // Tab pressed
    PrevField,   // Shift+Tab pressed
    TextChanged, // Text was modified
    CursorMoved, // Cursor position changed (navigation)
    ModeToggled, // Edit mode switched (Ctrl+X)
    ModeChanged, // Vi sub-mode changed (Normal/Insert)
}

/// Vi operator for pending operations (c{motion}, d{motion})
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViOperator {
    Change, // c - delete and enter Insert mode
    Delete, // d - delete
}

/// Vi find state for ; and , commands
#[derive(Debug, Clone, Copy, PartialEq)]
struct ViFind {
    char: char,
    backward: bool, // true = F, false = f
}

/// Reusable single-line text input widget
pub struct TextInput {
    text: String,
    cursor: usize, // Character index (not byte index!)
    scroll: usize, // Display width offset
    mode: EditMode,
    vi_mode: ViMode,     // Only used when mode == Vi
    kill_buffer: String, // Internal clipboard
    width: u16,          // Display width of widget
    // Vi mode state
    pending_operator: Option<ViOperator>, // Pending c{motion} or d{motion}
    last_find: Option<ViFind>,            // Last f/F search
    pending_find_backward: Option<bool>,  // Pending f/F input (true=F, false=f)
    pending_ctrl_x: bool,                 // Pending Ctrl+X for Ctrl+X U sequence
    // Undo/Redo history
    history: Vec<String>,  // History stack
    history_index: usize,  // Current position in history (0 = oldest)
    original_text: String, // Text when dialog opened (for Vi U command)
}

impl TextInput {
    /// Create new text input with optional initial text
    pub fn new(text: Option<String>, mode: EditMode) -> Self {
        let text = text.unwrap_or_default();
        let cursor = text.chars().count(); // Start at end
        Self {
            text: text.clone(),
            cursor,
            scroll: 0,
            mode,
            vi_mode: ViMode::Insert,
            kill_buffer: String::new(),
            width: 40, // Default width
            pending_operator: None,
            last_find: None,
            pending_find_backward: None,
            pending_ctrl_x: false,
            history: vec![text],
            history_index: 0,
            original_text: String::new(),
        }
    }

    /// Set the original text (for Vi U command - revert to original)
    pub fn set_original_text(&mut self, text: String) {
        self.original_text = text;
    }

    /// Set display width
    pub fn set_width(&mut self, width: u16) {
        self.width = width;
    }

    /// Get current text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get cursor position (character index)
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get scroll position
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Set cursor position
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.text.chars().count());
        self.update_scroll();
    }

    /// Set scroll position
    pub fn set_scroll(&mut self, scroll: usize) {
        self.scroll = scroll;
    }

    /// Get edit mode
    pub fn mode(&self) -> EditMode {
        self.mode
    }

    /// Toggle between Emacs and Vi mode
    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            EditMode::Emacs => EditMode::Vi,
            EditMode::Vi => EditMode::Emacs,
        };
        if self.mode == EditMode::Vi {
            self.vi_mode = ViMode::Normal; // Start in Normal mode when switching to Vi
        }
    }

    /// Get Vi sub-mode (only relevant in Vi mode)
    pub fn vi_mode(&self) -> Option<ViMode> {
        if self.mode == EditMode::Vi {
            Some(self.vi_mode)
        } else {
            None
        }
    }

    /// Set Vi sub-mode (for persisting state between key presses)
    pub fn set_vi_mode(&mut self, vi_mode: ViMode) {
        if self.mode == EditMode::Vi {
            self.vi_mode = vi_mode;
        }
    }

    /// Get pending find state (for persisting between key presses)
    pub fn pending_find_backward(&self) -> Option<bool> {
        self.pending_find_backward
    }

    /// Set pending find state
    pub fn set_pending_find_backward(&mut self, val: Option<bool>) {
        self.pending_find_backward = val;
    }

    /// Get pending operator state (for persisting between key presses)
    pub fn pending_operator(&self) -> Option<ViOperator> {
        self.pending_operator
    }

    /// Set pending operator state
    pub fn set_pending_operator(&mut self, val: Option<ViOperator>) {
        self.pending_operator = val;
    }

    /// Get pending Ctrl+X state (for Ctrl+X U two-key sequence)
    pub fn pending_ctrl_x(&self) -> bool {
        self.pending_ctrl_x
    }

    /// Set pending Ctrl+X state
    pub fn set_pending_ctrl_x(&mut self, val: bool) {
        self.pending_ctrl_x = val;
    }

    /// Save current text to history (call before modifications)
    fn save_to_history(&mut self) {
        // Truncate any future history if we're in the middle
        if self.history_index < self.history.len() - 1 {
            self.history.truncate(self.history_index + 1);
        }
        // Push current state
        self.history.push(self.text.clone());
        self.history_index = self.history.len() - 1;
        // Limit history size
        if self.history.len() > 100 {
            self.history.remove(0);
            self.history_index = self.history.len() - 1;
        }
    }

    /// Undo: revert to previous state
    fn undo(&mut self) -> bool {
        if self.history_index > 0 {
            self.history_index -= 1;
            let prev_text = self.history[self.history_index].clone();
            let prev_cursor = prev_text.chars().count().min(self.cursor);
            self.text = prev_text;
            self.cursor = prev_cursor;
            self.update_scroll();
            true
        } else {
            false
        }
    }

    /// Redo: advance to next state
    fn redo(&mut self) -> bool {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            let next_text = self.history[self.history_index].clone();
            let next_cursor = next_text.chars().count().min(self.cursor);
            self.text = next_text;
            self.cursor = next_cursor;
            self.update_scroll();
            true
        } else {
            false
        }
    }

    /// Revert to original text (Vi U command)
    fn revert_to_original(&mut self) -> bool {
        if !self.original_text.is_empty() && self.text != self.original_text {
            let orig_cursor = self.original_text.chars().count().min(self.cursor);
            self.text = self.original_text.clone();
            self.cursor = orig_cursor;
            self.update_scroll();
            true
        } else {
            false
        }
    }

    /// Get history stack (for persistence)
    pub fn history(&self) -> &Vec<String> {
        &self.history
    }

    /// Set history stack (for persistence)
    pub fn set_history(&mut self, history: Vec<String>) {
        if !history.is_empty() {
            self.history = history;
            self.history_index = self.history.len() - 1;
        }
    }

    /// Get current history index
    pub fn history_index(&self) -> usize {
        self.history_index
    }

    /// Set current history index (for state restoration — does NOT modify text or cursor)
    pub fn set_history_index(&mut self, index: usize) {
        if index < self.history.len() {
            self.history_index = index;
        }
    }
}

// CJK-Aware Width Calculation
impl TextInput {
    /// Calculate display width of a character (CJK-aware)
    fn char_width(c: char) -> usize {
        UnicodeWidthChar::width(c).unwrap_or(1)
    }

    /// Calculate display width of text up to character index
    fn text_width_to_cursor(&self) -> usize {
        self.text
            .chars()
            .take(self.cursor)
            .map(Self::char_width)
            .sum()
    }

    /// Update scroll offset to keep cursor visible
    fn update_scroll(&mut self) {
        let cursor_x = self.text_width_to_cursor();
        let visible_width = self.width as usize - 1; // Leave 1 char for cursor

        if cursor_x < self.scroll {
            // Cursor moved left of visible area
            self.scroll = cursor_x;
        } else if cursor_x >= self.scroll + visible_width {
            // Cursor moved right of visible area
            self.scroll = cursor_x - visible_width + 1;
        }
    }
}

// Rendering
impl TextInput {
    /// Render the text input widget
    pub fn render(&self, frame: &mut Frame, area: Rect, is_focused: bool) {
        // Get visible text based on scroll offset
        let visible_text = self.get_visible_text();

        // Build spans with cursor
        let spans = self.build_spans(&visible_text, is_focused);

        // Render
        let style = if is_focused {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray).bg(Color::Gray)
        };

        let paragraph = Paragraph::new(Line::from(spans)).style(style);
        frame.render_widget(paragraph, area);

        // Render mode indicator if in Vi mode (after textbox, 1 char left to avoid border overlap)
        if self.mode == EditMode::Vi {
            let indicator = match self.vi_mode {
                ViMode::Normal => " -NORMAL-",
                ViMode::Insert => " -INSERT-",
            };
            let indicator_style = Style::default()
                .fg(Color::Yellow)
                .bg(Color::Gray)
                .add_modifier(Modifier::BOLD);
            let indicator_x = area.x + area.width.saturating_sub(1);
            let indicator_para = Paragraph::new(indicator).style(indicator_style);
            frame.render_widget(
                indicator_para,
                Rect::new(indicator_x, area.y, indicator.len() as u16, 1),
            );
        }
    }

    /// Get visible portion of text based on scroll offset
    fn get_visible_text(&self) -> String {
        let mut visible = String::new();
        let mut current_width = 0;
        let target_width = self.width as usize - 1;

        for c in self.text.chars() {
            let w = Self::char_width(c);
            if current_width + w > self.scroll + target_width {
                break;
            }
            if current_width >= self.scroll {
                visible.push(c);
            }
            current_width += w;
        }

        visible
    }

    /// Build spans with cursor highlighting
    fn build_spans(&self, visible_text: &str, is_focused: bool) -> Vec<Span<'_>> {
        let mut spans = Vec::new();

        // Calculate visible cursor position
        let mut visible_cursor = 0;
        let mut current_width = 0;
        for c in self.text.chars() {
            let w = Self::char_width(c);
            if current_width >= self.scroll {
                if current_width == self.text_width_to_cursor() {
                    break;
                }
                visible_cursor += w;
            }
            current_width += w;
        }

        // Build spans with cursor
        let mut char_visible_pos = 0;
        for c in visible_text.chars() {
            if char_visible_pos == visible_cursor && is_focused {
                // This is the cursor position - show with inverse video
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ));
            } else {
                spans.push(Span::raw(c.to_string()));
            }
            char_visible_pos += Self::char_width(c);
        }

        // If cursor is at end, add cursor block
        let total_visible_width: usize = visible_text.chars().map(Self::char_width).sum();
        if visible_cursor >= total_visible_width && is_focused {
            spans.push(Span::styled(
                "█",
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Pad to fill width
        let current_width: usize = spans.iter().map(|s| s.content.width()).sum();
        if current_width < self.width as usize {
            spans.push(Span::raw(" ".repeat(self.width as usize - current_width)));
        }

        spans
    }
}

// Text Manipulation
impl TextInput {
    /// Insert character at cursor
    fn insert_char(&mut self, c: char) {
        self.save_to_history();
        let byte_pos: usize = self
            .text
            .chars()
            .take(self.cursor)
            .map(|ch| ch.len_utf8())
            .sum();
        self.text.insert(byte_pos, c);
        self.cursor += 1;
    }

    /// Delete character at cursor
    fn delete_char_at_cursor(&mut self) {
        self.save_to_history();
        let byte_pos: usize = self
            .text
            .chars()
            .take(self.cursor)
            .map(|ch| ch.len_utf8())
            .sum();
        if let Some(c) = self.text[byte_pos..].chars().next() {
            let end = byte_pos + c.len_utf8();
            self.text.drain(byte_pos..end);
        }
    }

    /// Delete character before cursor (backspace)
    fn delete_char_before_cursor(&mut self) {
        if self.cursor > 0 {
            self.save_to_history();
            self.cursor -= 1;
            self.delete_char_at_cursor();
        }
    }

    /// Kill to end of line (Ctrl+K)
    fn kill_to_end(&mut self) {
        let byte_pos: usize = self
            .text
            .chars()
            .take(self.cursor)
            .map(|ch| ch.len_utf8())
            .sum();
        self.kill_buffer = self.text[byte_pos..].to_string();
        self.text.truncate(byte_pos);
    }

    /// Kill to beginning of line (Ctrl+U)
    fn kill_to_start(&mut self) {
        let byte_pos: usize = self
            .text
            .chars()
            .take(self.cursor)
            .map(|ch| ch.len_utf8())
            .sum();
        self.kill_buffer = self.text[..byte_pos].to_string();
        self.text.drain(..byte_pos);
        self.cursor = 0;
    }

    /// Yank (paste) from kill buffer (Ctrl+Y)
    fn yank(&mut self) {
        if !self.kill_buffer.is_empty() {
            let byte_pos: usize = self
                .text
                .chars()
                .take(self.cursor)
                .map(|ch| ch.len_utf8())
                .sum();
            self.text.insert_str(byte_pos, &self.kill_buffer);
            self.cursor += self.kill_buffer.chars().count();
        }
    }

    /// Delete word before cursor (Ctrl+W)
    fn delete_word_before(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut pos = self.cursor;

        // Skip whitespace
        while pos > 0 && chars[pos - 1].is_whitespace() {
            pos -= 1;
        }
        // Find word boundary
        while pos > 0 && !chars[pos - 1].is_whitespace() {
            pos -= 1;
        }

        let byte_start: usize = self.text.chars().take(pos).map(|ch| ch.len_utf8()).sum();
        let byte_end: usize = self
            .text
            .chars()
            .take(self.cursor)
            .map(|ch| ch.len_utf8())
            .sum();

        self.kill_buffer = self.text[byte_start..byte_end].to_string();
        self.text.drain(byte_start..byte_end);
        self.cursor = pos;
    }

    /// Transpose characters (Ctrl+T)
    fn transpose_chars(&mut self) {
        if self.cursor < 2 {
            return;
        }
        let byte_pos_1: usize = self
            .text
            .chars()
            .take(self.cursor - 2)
            .map(|ch| ch.len_utf8())
            .sum();
        let byte_pos_2: usize = self
            .text
            .chars()
            .take(self.cursor - 1)
            .map(|ch| ch.len_utf8())
            .sum();
        let byte_pos_3: usize = self
            .text
            .chars()
            .take(self.cursor)
            .map(|ch| ch.len_utf8())
            .sum();

        let c1 = self.text[byte_pos_1..byte_pos_2].to_string();
        let c2 = self.text[byte_pos_2..byte_pos_3].to_string();

        self.text
            .replace_range(byte_pos_1..byte_pos_3, &format!("{}{}", c2, c1));
    }
}

// Key Input Handling
impl TextInput {
    /// Handle key input, return action
    pub fn handle_input(&mut self, key: &KeyEvent) -> TextInputAction {
        match self.mode {
            EditMode::Emacs => self.handle_emacs_input(key),
            EditMode::Vi => self.handle_vi_input(key),
        }
    }

    fn handle_emacs_input(&mut self, key: &KeyEvent) -> TextInputAction {
        match (key.code, key.modifiers) {
            // Navigation
            (KeyCode::Left, _) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.update_scroll();
                }
                TextInputAction::CursorMoved
            }
            (KeyCode::Right, _) => {
                if self.cursor < self.text.chars().count() {
                    self.cursor += 1;
                    self.update_scroll();
                }
                TextInputAction::CursorMoved
            }
            (KeyCode::Home, _) | (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                // Ctrl+A: Beginning of line
                self.cursor = 0;
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::End, _) | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                // Ctrl+E: End of line
                self.cursor = self.text.chars().count();
                self.update_scroll();
                TextInputAction::CursorMoved
            }

            // Mode toggle
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
                // Ctrl+X: Toggle mode
                self.toggle_mode();
                TextInputAction::ModeToggled
            }

            // Editing
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                // Ctrl+D: Delete char at cursor
                if self.cursor < self.text.chars().count() {
                    self.delete_char_at_cursor();
                    self.update_scroll();
                    return TextInputAction::TextChanged;
                }
                TextInputAction::None
            }
            (KeyCode::Char('h'), KeyModifiers::CONTROL) | (KeyCode::Backspace, _) => {
                // Ctrl+H or Backspace: Delete char before cursor
                if self.cursor > 0 {
                    self.delete_char_before_cursor();
                    self.update_scroll();
                    return TextInputAction::TextChanged;
                }
                TextInputAction::None
            }
            (KeyCode::Delete, _) => {
                // Delete key: Delete char at cursor
                if self.cursor < self.text.chars().count() {
                    self.delete_char_at_cursor();
                    self.update_scroll();
                    return TextInputAction::TextChanged;
                }
                TextInputAction::None
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                // Ctrl+K: Kill to end of line
                self.kill_to_end();
                TextInputAction::TextChanged
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                // Ctrl+U: Kill to beginning
                self.kill_to_start();
                TextInputAction::TextChanged
            }
            (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                // Ctrl+Y: Yank (paste)
                self.yank();
                TextInputAction::TextChanged
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                // Ctrl+W: Delete word before cursor
                self.delete_word_before();
                TextInputAction::TextChanged
            }
            (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
                // Ctrl+T: Transpose characters
                self.transpose_chars();
                TextInputAction::TextChanged
            }
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                // Ctrl+Z: Undo
                if self.undo() {
                    TextInputAction::TextChanged
                } else {
                    TextInputAction::None
                }
            }
            (KeyCode::Char('/'), KeyModifiers::CONTROL) => {
                // Ctrl+/: Undo (alternate)
                if self.undo() {
                    TextInputAction::TextChanged
                } else {
                    TextInputAction::None
                }
            }
            (KeyCode::Char('_'), KeyModifiers::CONTROL) => {
                // Ctrl+_: Undo (alternate)
                if self.undo() {
                    TextInputAction::TextChanged
                } else {
                    TextInputAction::None
                }
            }
            (KeyCode::Char('y'), KeyModifiers::ALT) => {
                // Alt+Y: Redo
                if self.redo() {
                    TextInputAction::TextChanged
                } else {
                    TextInputAction::None
                }
            }
            (KeyCode::Char('x'), KeyModifiers::ALT) => {
                // Alt+X: Toggle mode
                self.toggle_mode();
                TextInputAction::ModeToggled
            }

            // Actions
            (KeyCode::Enter, _) => TextInputAction::Confirm,
            (KeyCode::Esc, _) => TextInputAction::Cancel,
            (KeyCode::Tab, _) => TextInputAction::NextField,
            (KeyCode::BackTab, _) => TextInputAction::PrevField,

            // Printable characters
            (KeyCode::Char(c), _) => {
                self.insert_char(c);
                self.update_scroll();
                TextInputAction::TextChanged
            }

            _ => TextInputAction::None,
        }
    }

    fn handle_vi_input(&mut self, key: &KeyEvent) -> TextInputAction {
        match self.vi_mode {
            ViMode::Normal => self.handle_vi_normal(key),
            ViMode::Insert => self.handle_vi_insert(key),
        }
    }

    fn handle_vi_normal(&mut self, key: &KeyEvent) -> TextInputAction {
        // If we have a pending f/F, the next char is the search target
        if let Some(backward) = self.pending_find_backward.take() {
            if let KeyCode::Char(c) = key.code {
                if backward {
                    self.find_prev_char(c);
                } else {
                    self.find_next_char(c);
                }
                return TextInputAction::CursorMoved;
            }
            // If not a char, cancel find
            return TextInputAction::None;
        }

        // If we have a pending Ctrl+X, check for U (undo)
        if self.pending_ctrl_x {
            self.pending_ctrl_x = false;
            if let KeyCode::Char('U') = key.code {
                if self.undo() {
                    return TextInputAction::TextChanged;
                }
                return TextInputAction::None;
            }
            // Not U, just process the key normally
        }

        // If we have a pending operator (c{motion}, d{motion}), handle motion keys
        if let Some(op) = self.pending_operator.take() {
            return self.handle_vi_operator_motion(op, key);
        }

        match (key.code, key.modifiers) {
            // Mode toggle (check before 'x' delete)
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
                // Ctrl+X: Toggle mode
                self.toggle_mode();
                TextInputAction::ModeToggled
            }
            (KeyCode::Char('h'), _) => {
                // Left
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.update_scroll();
                }
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('l'), _) | (KeyCode::Right, _) => {
                // Right
                if self.cursor < self.text.chars().count() {
                    self.cursor += 1;
                    self.update_scroll();
                }
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('0'), _) | (KeyCode::Home, _) => {
                // Beginning of line
                self.cursor = 0;
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('^'), _) => {
                // First non-blank character of line
                let chars: Vec<char> = self.text.chars().collect();
                let first_nonblank = chars.iter().position(|c| !c.is_whitespace()).unwrap_or(0);
                self.cursor = first_nonblank;
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('$'), _) | (KeyCode::End, _) => {
                // End of line
                self.cursor = self.text.chars().count();
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('w'), _) => {
                // Next word beginning
                self.cursor = self.next_word_start(false);
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('b'), _) => {
                // Previous word beginning
                self.cursor = self.prev_word_start(false);
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('e'), _) => {
                // Next word end
                self.cursor = self.next_word_end(false);
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('W'), _) => {
                // Next WORD beginning (whitespace + '.' delimited)
                self.cursor = self.next_word_start(true);
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('B'), _) => {
                // Previous WORD beginning (whitespace + '.' delimited)
                self.cursor = self.prev_word_start(true);
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('E'), _) => {
                // Next WORD end (whitespace + '.' delimited)
                self.cursor = self.next_word_end(true);
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('f'), _) => {
                // Find next char - wait for next key
                self.pending_find_backward = Some(false);
                TextInputAction::None
            }
            (KeyCode::Char('F'), _) => {
                // Find prev char - wait for next key
                self.pending_find_backward = Some(true);
                TextInputAction::None
            }
            (KeyCode::Char(';'), _) => {
                // Repeat last find
                if let Some(find) = self.last_find {
                    self.cursor = self.find_char(find.char, find.backward, self.cursor);
                    self.update_scroll();
                }
                TextInputAction::CursorMoved
            }
            (KeyCode::Char(','), _) => {
                // Repeat last find in opposite direction
                if let Some(find) = self.last_find {
                    self.cursor = self.find_char(find.char, !find.backward, self.cursor);
                    self.update_scroll();
                }
                TextInputAction::CursorMoved
            }
            (KeyCode::Char('x'), _) => {
                // Delete char at cursor
                if self.cursor < self.text.chars().count() {
                    self.delete_char_at_cursor();
                    self.update_scroll();
                    return TextInputAction::TextChanged;
                }
                TextInputAction::None
            }
            (KeyCode::Char('i'), _) => {
                // Enter Insert mode
                self.vi_mode = ViMode::Insert;
                TextInputAction::ModeChanged
            }
            (KeyCode::Char('a'), _) => {
                // Append: move right and enter Insert mode
                if self.cursor < self.text.chars().count() {
                    self.cursor += 1;
                }
                self.vi_mode = ViMode::Insert;
                TextInputAction::ModeChanged
            }
            (KeyCode::Char('I'), _) => {
                // Insert at beginning
                self.cursor = 0;
                self.vi_mode = ViMode::Insert;
                TextInputAction::ModeChanged
            }
            (KeyCode::Char('A'), _) => {
                // Append at end
                self.cursor = self.text.chars().count();
                self.vi_mode = ViMode::Insert;
                TextInputAction::ModeChanged
            }
            (KeyCode::Char('c'), _) => {
                // Change operator - wait for motion
                self.pending_operator = Some(ViOperator::Change);
                TextInputAction::None
            }
            (KeyCode::Char('d'), _) => {
                // Delete operator - wait for motion
                self.pending_operator = Some(ViOperator::Delete);
                TextInputAction::None
            }
            (KeyCode::Char('U'), _) => {
                // U: Revert to original text (Vi undo)
                if self.revert_to_original() {
                    TextInputAction::TextChanged
                } else {
                    TextInputAction::None
                }
            }
            (KeyCode::Char('u'), _) => {
                // u: Undo (step back in history)
                if self.undo() {
                    TextInputAction::TextChanged
                } else {
                    TextInputAction::None
                }
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                // Ctrl+R: Redo
                if self.redo() {
                    TextInputAction::TextChanged
                } else {
                    TextInputAction::None
                }
            }
            (KeyCode::Esc, _) | (KeyCode::Char('['), KeyModifiers::CONTROL) => {
                // Esc or Ctrl+[: Cancel dialog (same as Emacs mode)
                TextInputAction::Cancel
            }
            (KeyCode::Enter, _) => TextInputAction::Confirm,
            (KeyCode::Tab, _) => TextInputAction::NextField,
            (KeyCode::BackTab, _) => TextInputAction::PrevField,
            _ => TextInputAction::None,
        }
    }

    /// Handle motion after operator (c{motion}, d{motion})
    fn handle_vi_operator_motion(&mut self, op: ViOperator, key: &KeyEvent) -> TextInputAction {
        let start = self.cursor;

        let end = match (key.code, key.modifiers) {
            (KeyCode::Char('w'), _) => self.next_word_start(false),
            (KeyCode::Char('b'), _) => self.prev_word_start(false),
            (KeyCode::Char('W'), _) => self.next_word_start(true),
            (KeyCode::Char('B'), _) => self.prev_word_start(true),
            (KeyCode::Char('f'), _) => self.text.chars().count(),
            (KeyCode::Char('F'), _) => 0,
            (KeyCode::Char('$'), _) => self.text.chars().count(),
            (KeyCode::Char('^'), _) | (KeyCode::Char('0'), _) => 0,
            _ => return TextInputAction::None,
        };

        let (del_start, del_end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        self.delete_range(del_start, del_end);

        if op == ViOperator::Change {
            self.vi_mode = ViMode::Insert;
            return TextInputAction::ModeChanged;
        }

        TextInputAction::TextChanged
    }

    /// Delete characters from start to end (exclusive)
    fn delete_range(&mut self, start: usize, end: usize) {
        if start >= end || start >= self.text.chars().count() {
            return;
        }
        self.save_to_history();
        let end = end.min(self.text.chars().count());

        let byte_start: usize = self.text.chars().take(start).map(|ch| ch.len_utf8()).sum();
        let byte_end: usize = self.text.chars().take(end).map(|ch| ch.len_utf8()).sum();

        // Save to kill buffer
        self.kill_buffer = self.text[byte_start..byte_end].to_string();

        self.text.drain(byte_start..byte_end);
        self.cursor = start.min(self.text.chars().count());
        self.update_scroll();
    }

    /// Find next/previous occurrence of char
    fn find_char(&self, c: char, backward: bool, from: usize) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        if backward {
            for i in (0..from).rev() {
                if chars[i] == c {
                    return i;
                }
            }
        } else {
            for (i, &ch) in chars.iter().enumerate().skip(from + 1) {
                if ch == c {
                    return i;
                }
            }
        }
        from // Not found, stay in place
    }

    /// Find next occurrence of char and update state
    fn find_next_char(&mut self, c: char) {
        self.cursor = self.find_char(c, false, self.cursor);
        self.last_find = Some(ViFind {
            char: c,
            backward: false,
        });
        self.update_scroll();
    }

    /// Find previous occurrence of char and update state
    fn find_prev_char(&mut self, c: char) {
        self.cursor = self.find_char(c, true, self.cursor);
        self.last_find = Some(ViFind {
            char: c,
            backward: true,
        });
        self.update_scroll();
    }

    /// Find next word end
    fn next_word_end(&self, word_boundary: bool) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let len = chars.len();
        let mut i = self.cursor;

        if word_boundary {
            // WORD: whitespace and '.' delimited
            // Skip whitespace first
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            // Skip WORD chars
            while i < len && Self::is_word_char_big(chars[i]) {
                i += 1;
            }
        } else {
            // word: alphanumeric + underscore delimited
            // Skip non-word chars first
            while i < len && !Self::is_word_char(chars[i]) {
                i += 1;
            }
            // Skip word chars
            while i < len && Self::is_word_char(chars[i]) {
                i += 1;
            }
        }

        // Move back one to get to end of word
        if i > 0 {
            i - 1
        } else {
            0
        }
    }

    /// Check if character is a word character (alphanumeric + underscore)
    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// Check if character is a WORD character (non-whitespace, non-dot)
    fn is_word_char_big(c: char) -> bool {
        !c.is_whitespace() && c != '.'
    }

    /// Find next word start
    fn next_word_start(&self, word_boundary: bool) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let len = chars.len();
        let mut i = self.cursor;

        if word_boundary {
            // WORD: whitespace and '.' delimited
            while i < len && Self::is_word_char_big(chars[i]) {
                i += 1;
            }
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
        } else {
            // word: alphanumeric + underscore delimited
            // Skip current word
            while i < len && Self::is_word_char(chars[i]) {
                i += 1;
            }
            // Skip non-word chars
            while i < len && !Self::is_word_char(chars[i]) && !chars[i].is_whitespace() {
                i += 1;
            }
            // Skip whitespace
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
        }

        i.min(len)
    }

    /// Find previous word start
    fn prev_word_start(&self, word_boundary: bool) -> usize {
        let chars: Vec<char> = self.text.chars().collect();

        if self.cursor == 0 {
            return 0;
        }

        let mut i = self.cursor;

        if word_boundary {
            // WORD: whitespace and '.' delimited
            // Move back one position first
            i = i.saturating_sub(1);
            // Skip whitespace backwards
            while i > 0 && chars[i].is_whitespace() {
                i -= 1;
            }
            // Skip WORD chars backwards
            while i > 0 && Self::is_word_char_big(chars[i]) {
                i -= 1;
            }
            // If we're on a WORD char, we're at the start
            if Self::is_word_char_big(chars[i]) {
                i
            } else {
                (i + 1).min(chars.len())
            }
        } else {
            // word: alphanumeric + underscore delimited
            // Move back one position first
            i = i.saturating_sub(1);
            // Skip whitespace/non-word chars backwards
            while i > 0 && (!Self::is_word_char(chars[i]) || chars[i].is_whitespace()) {
                i -= 1;
            }
            // Skip word chars backwards
            while i > 0 && Self::is_word_char(chars[i]) {
                i -= 1;
            }
            // Move to start of word
            if Self::is_word_char(chars[i]) {
                i
            } else {
                (i + 1).min(chars.len())
            }
        }
    }

    fn handle_vi_insert(&mut self, key: &KeyEvent) -> TextInputAction {
        match (key.code, key.modifiers) {
            // Mode toggle
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
                // Ctrl+X: Toggle mode
                self.toggle_mode();
                TextInputAction::ModeToggled
            }
            (KeyCode::Char('x'), KeyModifiers::ALT) => {
                // Alt+X: Toggle mode
                self.toggle_mode();
                TextInputAction::ModeToggled
            }
            (KeyCode::Esc, _) | (KeyCode::Char('['), KeyModifiers::CONTROL) => {
                // Esc or Ctrl+[: Enter Normal mode (standard Vi behavior)
                self.vi_mode = ViMode::Normal;
                TextInputAction::ModeChanged
            }
            (KeyCode::Enter, _) => TextInputAction::Confirm,
            (KeyCode::Tab, _) => TextInputAction::NextField,
            (KeyCode::BackTab, _) => TextInputAction::PrevField,
            (KeyCode::Char(c), _) => {
                self.insert_char(c);
                self.update_scroll();
                TextInputAction::TextChanged
            }
            (KeyCode::Backspace, _) => {
                if self.cursor > 0 {
                    self.delete_char_before_cursor();
                    self.update_scroll();
                    return TextInputAction::TextChanged;
                }
                TextInputAction::None
            }
            (KeyCode::Delete, _) => {
                if self.cursor < self.text.chars().count() {
                    self.delete_char_at_cursor();
                    self.update_scroll();
                    return TextInputAction::TextChanged;
                }
                TextInputAction::None
            }
            (KeyCode::Left, _) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.update_scroll();
                }
                TextInputAction::CursorMoved
            }
            (KeyCode::Right, _) => {
                if self.cursor < self.text.chars().count() {
                    self.cursor += 1;
                    self.update_scroll();
                }
                TextInputAction::CursorMoved
            }
            (KeyCode::Home, _) => {
                self.cursor = 0;
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            (KeyCode::End, _) => {
                self.cursor = self.text.chars().count();
                self.update_scroll();
                TextInputAction::CursorMoved
            }
            _ => TextInputAction::None,
        }
    }
}
