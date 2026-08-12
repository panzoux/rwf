//! Multi-line text input widget with CJK support (Phase 7.17)
//!
//! Distinct from `text_input::TextInput` (single-line, Emacs/Vi modal editing,
//! undo/redo, kill buffer). This widget covers the simpler needs of a free-form
//! multi-line text box (e.g. the diagnostic report prompt): printable insert,
//! Backspace/Delete across line boundaries, Enter splits a line, arrow-key
//! navigation by character and by line, Home/End for line start/end, and
//! vertical scrolling that keeps the cursor visible.
//!
//! ## Design notes
//!
//! - **No soft line-wrap.** Each logical line (split on `\n`) is exactly one
//!   row. Long lines are clipped to the widget width rather than wrapped; only
//!   *vertical* scrolling is required by the spec this widget was built for
//!   (see `plan/7.17...`). This keeps rendering and input handling from ever
//!   needing to agree on an exact pixel width, unlike a wrapping design where
//!   Up/Down would have to replicate the renderer's wrap boundaries exactly.
//! - **CJK width awareness.** All cursor positioning — including the
//!   sticky-column behaviour of Up/Down between lines of different content —
//!   is computed from `unicode_width` display columns, never character counts
//!   or byte offsets. A double-width character is never split.
//! - **No persisted horizontal scroll.** Only the cursor's own line ever needs
//!   horizontal scrolling, and it's cheap to recompute from scratch on every
//!   render (unlike the vertical scroll, which must remember the previous
//!   position to avoid re-snapping the viewport on every keystroke).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};
use unicode_width::UnicodeWidthChar;

/// Result of multi-line text input handling.
#[derive(Debug, Clone, PartialEq)]
pub enum MultiLineInputAction {
    /// Input consumed, nothing else changed.
    None,
    /// Confirm the dialog (`Ctrl+S`, or `Ctrl+Enter` where the terminal reports it).
    Confirm,
    /// Escape pressed — cancel the dialog.
    Cancel,
    /// Text content was modified.
    TextChanged,
    /// Cursor position changed (navigation only).
    CursorMoved,
}

/// Multi-line text input widget.
pub struct MultiLineTextInput {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize, // character index within `lines[cursor_line]`, not byte index
    scroll_row: usize, // first visible logical line
    width: u16,
    height: u16,
}

impl MultiLineTextInput {
    /// Create a new multi-line input, splitting `text` on `\n`.
    pub fn new(text: Option<String>) -> Self {
        let text = text.unwrap_or_default();
        let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let cursor_line = lines.len() - 1;
        let cursor_col = lines[cursor_line].chars().count();
        Self {
            lines,
            cursor_line,
            cursor_col,
            scroll_row: 0,
            width: 40,
            height: 10,
        }
    }

    /// Joined text, lines separated by `\n`. Test-only: production callers
    /// persist state via `lines()` instead (see `dialog/multiline_input.rs`),
    /// so this exists purely to keep assertions in this file's own tests
    /// readable — gated out of non-test builds to avoid a dead-code warning
    /// under `-D warnings`.
    #[cfg(test)]
    fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Logical lines, for persisting widget state across dialog rebuilds.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn cursor_line(&self) -> usize {
        self.cursor_line
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    pub fn scroll(&self) -> usize {
        self.scroll_row
    }

    pub fn set_width(&mut self, width: u16) {
        self.width = width;
    }

    pub fn set_height(&mut self, height: u16) {
        self.height = height;
    }

    /// Restore cursor position, clamping to the current text.
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        self.cursor_line = line.min(self.lines.len().saturating_sub(1));
        let max_col = self.lines[self.cursor_line].chars().count();
        self.cursor_col = col.min(max_col);
    }

    /// Restore vertical scroll position.
    pub fn set_scroll(&mut self, scroll: usize) {
        self.scroll_row = scroll;
    }
}

// CJK-aware width helpers
impl MultiLineTextInput {
    fn char_width(c: char) -> usize {
        UnicodeWidthChar::width(c).unwrap_or(1)
    }

    /// Display width of `line[..col]` (col is a character index).
    fn width_upto(line: &str, col: usize) -> usize {
        line.chars().take(col).map(Self::char_width).sum()
    }

    /// The character index in `line` whose cumulative display width first
    /// reaches (but does not exceed) `target_width`. Used to keep Up/Down
    /// navigation aligned on visual columns rather than character counts,
    /// so moving between an ASCII line and a CJK line lands the cursor where
    /// it looks right, not where the raw index happens to match.
    fn col_for_width(line: &str, target_width: usize) -> usize {
        let mut w = 0usize;
        let mut col = 0usize;
        for c in line.chars() {
            let cw = Self::char_width(c);
            if w + cw > target_width {
                break;
            }
            w += cw;
            col += 1;
        }
        col
    }

    fn current_line_char_count(&self) -> usize {
        self.lines[self.cursor_line].chars().count()
    }

    fn byte_pos(line: &str, col: usize) -> usize {
        line.chars().take(col).map(|c| c.len_utf8()).sum()
    }
}

// Vertical scroll
impl MultiLineTextInput {
    fn update_scroll(&mut self) {
        let h = self.height.max(1) as usize;
        if self.cursor_line < self.scroll_row {
            self.scroll_row = self.cursor_line;
        } else if self.cursor_line >= self.scroll_row + h {
            self.scroll_row = self.cursor_line + 1 - h;
        }
    }
}

// Text manipulation
impl MultiLineTextInput {
    fn insert_char(&mut self, c: char) {
        let byte_pos = Self::byte_pos(&self.lines[self.cursor_line], self.cursor_col);
        self.lines[self.cursor_line].insert(byte_pos, c);
        self.cursor_col += 1;
    }

    /// Enter: split the current line at the cursor into two lines.
    fn insert_newline(&mut self) {
        let byte_pos = Self::byte_pos(&self.lines[self.cursor_line], self.cursor_col);
        let rest = self.lines[self.cursor_line].split_off(byte_pos);
        self.lines.insert(self.cursor_line + 1, rest);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.update_scroll();
    }

    /// Backspace: delete the character before the cursor, merging into the
    /// previous line when at column 0. Returns whether anything changed.
    fn backspace(&mut self) -> bool {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            let end = Self::byte_pos(line, self.cursor_col);
            let start = Self::byte_pos(line, self.cursor_col - 1);
            line.drain(start..end);
            self.cursor_col -= 1;
            true
        } else if self.cursor_line > 0 {
            let removed = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_char_count = self.lines[self.cursor_line].chars().count();
            self.lines[self.cursor_line].push_str(&removed);
            self.cursor_col = prev_char_count;
            self.update_scroll();
            true
        } else {
            false
        }
    }

    /// Delete: delete the character at the cursor, merging the next line in
    /// when at end-of-line. Returns whether anything changed.
    fn delete_forward(&mut self) -> bool {
        let char_count = self.current_line_char_count();
        if self.cursor_col < char_count {
            let line = &mut self.lines[self.cursor_line];
            let start = Self::byte_pos(line, self.cursor_col);
            if let Some(c) = line[start..].chars().next() {
                let end = start + c.len_utf8();
                line.drain(start..end);
            }
            true
        } else if self.cursor_line + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next);
            true
        } else {
            false
        }
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.current_line_char_count();
            self.update_scroll();
        }
    }

    fn move_right(&mut self) {
        if self.cursor_col < self.current_line_char_count() {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
            self.update_scroll();
        }
    }

    /// Up (`delta = -1`) / Down (`delta = 1`) by one logical line, preserving
    /// the cursor's visual (width-based) column rather than its character
    /// index — see `col_for_width`.
    fn move_vertical(&mut self, delta: isize) {
        let target = self.cursor_line as isize + delta;
        if target < 0 || target as usize >= self.lines.len() {
            return;
        }
        let target = target as usize;
        let target_width = Self::width_upto(&self.lines[self.cursor_line], self.cursor_col);
        self.cursor_line = target;
        self.cursor_col = Self::col_for_width(&self.lines[target], target_width);
        self.update_scroll();
    }
}

// Key input handling
impl MultiLineTextInput {
    pub fn handle_input(&mut self, key: &KeyEvent) -> MultiLineInputAction {
        match (key.code, key.modifiers) {
            // Two confirm keys, and both are needed.
            //
            // `Ctrl+Enter` is what a Windows user expects, and crossterm reports
            // it there because the Console API delivers the modifier. On a plain
            // Unix terminal it is **indistinguishable from Enter** — the terminal
            // sends CR for both, and rwf does not push crossterm's keyboard
            // enhancement flags (see `terminal.rs`, which only enters the
            // alternate screen and enables raw mode). With Ctrl+Enter as the sole
            // confirm key, a Linux or macOS user could type a whole report and
            // have no way to submit it: Enter would keep inserting newlines and
            // Escape would throw the text away.
            //
            // `Ctrl+S` arrives as a distinct control byte on every platform. Raw
            // mode disables XON/XOFF, so terminal flow control does not eat it.
            (KeyCode::Enter, m) if m.contains(KeyModifiers::CONTROL) => {
                MultiLineInputAction::Confirm
            }
            (KeyCode::Char('s' | 'S'), m) if m.contains(KeyModifiers::CONTROL) => {
                MultiLineInputAction::Confirm
            }
            (KeyCode::Enter, _) => {
                self.insert_newline();
                MultiLineInputAction::TextChanged
            }
            (KeyCode::Esc, _) => MultiLineInputAction::Cancel,
            (KeyCode::Backspace, _) => {
                if self.backspace() {
                    MultiLineInputAction::TextChanged
                } else {
                    MultiLineInputAction::None
                }
            }
            (KeyCode::Delete, _) => {
                if self.delete_forward() {
                    MultiLineInputAction::TextChanged
                } else {
                    MultiLineInputAction::None
                }
            }
            (KeyCode::Left, _) => {
                self.move_left();
                MultiLineInputAction::CursorMoved
            }
            (KeyCode::Right, _) => {
                self.move_right();
                MultiLineInputAction::CursorMoved
            }
            (KeyCode::Up, _) => {
                self.move_vertical(-1);
                MultiLineInputAction::CursorMoved
            }
            (KeyCode::Down, _) => {
                self.move_vertical(1);
                MultiLineInputAction::CursorMoved
            }
            (KeyCode::Home, _) => {
                self.cursor_col = 0;
                MultiLineInputAction::CursorMoved
            }
            (KeyCode::End, _) => {
                self.cursor_col = self.current_line_char_count();
                MultiLineInputAction::CursorMoved
            }
            (KeyCode::Char(c), m)
                if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
            {
                self.insert_char(c);
                MultiLineInputAction::TextChanged
            }
            _ => MultiLineInputAction::None,
        }
    }
}

// Rendering
impl MultiLineTextInput {
    /// Render into `area`. `area.height` rows are shown starting at
    /// `scroll_row`; `area.width` columns per row.
    pub fn render(&self, frame: &mut Frame, area: Rect, is_focused: bool) {
        let base_style = if is_focused {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray).bg(Color::Gray)
        };
        let cursor_style = Style::default()
            .bg(Color::Cyan)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

        let visible_width = area.width as usize;
        let rows = area.height as usize;

        for row in 0..rows {
            let y = area.y + row as u16;
            let line_idx = self.scroll_row + row;
            if line_idx >= self.lines.len() {
                frame.render_widget(
                    Span::styled(" ".repeat(visible_width), base_style),
                    Rect::new(area.x, y, area.width, 1),
                );
                continue;
            }

            let line = &self.lines[line_idx];
            let is_cursor_line = is_focused && line_idx == self.cursor_line;

            // Horizontal scroll: only the cursor's own line needs it, and it's
            // cheap enough to recompute from scratch every render (see module
            // doc comment for why this is fine to not persist).
            let hscroll = if is_cursor_line {
                let cursor_x = Self::width_upto(line, self.cursor_col);
                let visible = visible_width.saturating_sub(1).max(1);
                cursor_x.saturating_sub(visible.saturating_sub(1))
            } else {
                0
            };

            let mut spans: Vec<Span> = Vec::new();
            let mut col_width = 0usize; // display width consumed so far, from hscroll
            let mut rendered_width = 0usize;
            let mut char_idx = 0usize;
            let mut cursor_drawn = false;

            for c in line.chars() {
                let cw = Self::char_width(c);
                let abs_width_before = col_width;
                col_width += cw;

                if abs_width_before < hscroll {
                    char_idx += 1;
                    continue;
                }
                // Check-before-add (not check-after): a double-width char
                // that would push past `visible_width` must be dropped
                // whole, never split across the row boundary.
                if rendered_width + cw > visible_width {
                    break;
                }

                let is_cursor_char = is_cursor_line && char_idx == self.cursor_col;
                if is_cursor_char {
                    spans.push(Span::styled(c.to_string(), cursor_style));
                    cursor_drawn = true;
                } else {
                    spans.push(Span::styled(c.to_string(), base_style));
                }
                rendered_width += cw;
                char_idx += 1;
            }

            let line_char_count = line.chars().count();
            if is_cursor_line
                && !cursor_drawn
                && self.cursor_col >= line_char_count
                && rendered_width < visible_width
            {
                spans.push(Span::styled("\u{2588}", cursor_style));
                rendered_width += 1;
            }

            if rendered_width < visible_width {
                spans.push(Span::styled(
                    " ".repeat(visible_width - rendered_width),
                    base_style,
                ));
            }

            frame.render_widget(Line::from(spans), Rect::new(area.x, y, area.width, 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn new_starts_at_end_of_text() {
        let ti = MultiLineTextInput::new(Some("hello\nworld".to_string()));
        assert_eq!(ti.cursor_line(), 1);
        assert_eq!(ti.cursor_col(), 5);
        assert_eq!(ti.text(), "hello\nworld");
    }

    #[test]
    fn new_empty_text_is_single_empty_line() {
        let ti = MultiLineTextInput::new(None);
        assert_eq!(ti.lines(), &[String::new()]);
        assert_eq!(ti.cursor_line(), 0);
        assert_eq!(ti.cursor_col(), 0);
    }

    #[test]
    fn insert_char_at_cursor() {
        let mut ti = MultiLineTextInput::new(Some("ac".to_string()));
        ti.set_cursor(0, 1);
        let action = ti.handle_input(&key(KeyCode::Char('b')));
        assert_eq!(action, MultiLineInputAction::TextChanged);
        assert_eq!(ti.text(), "abc");
        assert_eq!(ti.cursor_col(), 2);
    }

    #[test]
    fn enter_splits_line_into_two() {
        let mut ti = MultiLineTextInput::new(Some("hello world".to_string()));
        ti.set_cursor(0, 5);
        let action = ti.handle_input(&key(KeyCode::Enter));
        assert_eq!(action, MultiLineInputAction::TextChanged);
        assert_eq!(ti.text(), "hello\n world");
        assert_eq!(ti.cursor_line(), 1);
        assert_eq!(ti.cursor_col(), 0);
    }

    #[test]
    fn ctrl_enter_confirms_without_inserting_newline() {
        let mut ti = MultiLineTextInput::new(Some("hello".to_string()));
        ti.set_cursor(0, 5);
        let action = ti.handle_input(&ctrl_key(KeyCode::Enter));
        assert_eq!(action, MultiLineInputAction::Confirm);
        // Text must be unchanged — Ctrl+Enter must not also insert a newline.
        assert_eq!(ti.text(), "hello");
    }

    /// A plain Unix terminal cannot distinguish Ctrl+Enter from Enter (both are
    /// CR), and rwf does not push crossterm's keyboard enhancement flags. Without
    /// a second confirm key there is no way to submit on Linux or macOS: Enter
    /// keeps inserting newlines and Escape discards the text.
    #[test]
    fn ctrl_s_also_confirms_for_terminals_without_ctrl_enter() {
        let mut ti = MultiLineTextInput::new(Some("hello".to_string()));
        ti.set_cursor(0, 5);

        let action = ti.handle_input(&ctrl_key(KeyCode::Char('s')));
        assert_eq!(action, MultiLineInputAction::Confirm);
        assert_eq!(ti.text(), "hello", "Ctrl+S must not insert anything");

        // Uppercase too: some terminals report Ctrl+Shift+S this way.
        let mut ti = MultiLineTextInput::new(Some("hello".to_string()));
        assert_eq!(
            ti.handle_input(&ctrl_key(KeyCode::Char('S'))),
            MultiLineInputAction::Confirm
        );
    }

    /// Direct coverage of the two width helpers everything else builds on.
    ///
    /// Added after a reversion check showed that stubbing `char_width` to return
    /// 1 was caught by only a single higher-level test. The snapshot tests cannot
    /// catch it at all: ratatui places the glyphs itself, so a cursor-arithmetic
    /// bug leaves the rendered *text* correct and only misplaces the cursor. That
    /// makes these helpers worth pinning directly rather than through their
    /// callers.
    #[test]
    fn width_helpers_count_display_columns_not_characters() {
        // 3 CJK chars = 3 characters but 6 display columns.
        assert_eq!(MultiLineTextInput::width_upto("日本語", 3), 6);
        assert_eq!(MultiLineTextInput::width_upto("日本語", 1), 2);
        assert_eq!(MultiLineTextInput::width_upto("abc", 3), 3);
        // Mixed: "日" (2) + "ab" (2) = 4.
        assert_eq!(MultiLineTextInput::width_upto("日ab", 3), 4);

        // col_for_width is the inverse, and must never land mid-glyph: asking for
        // column 1 inside a 2-wide character yields index 0, not a split.
        assert_eq!(MultiLineTextInput::col_for_width("日本語", 0), 0);
        assert_eq!(MultiLineTextInput::col_for_width("日本語", 1), 0);
        assert_eq!(MultiLineTextInput::col_for_width("日本語", 2), 1);
        assert_eq!(MultiLineTextInput::col_for_width("日本語", 4), 2);
        assert_eq!(MultiLineTextInput::col_for_width("日本語", 99), 3);
        assert_eq!(MultiLineTextInput::col_for_width("abc", 2), 2);
    }

    /// The modifier is what distinguishes confirm from typing. A bare `s` must
    /// still be inserted as text.
    #[test]
    fn plain_s_is_typed_not_treated_as_confirm() {
        let mut ti = MultiLineTextInput::new(Some(String::new()));
        let action = ti.handle_input(&key(KeyCode::Char('s')));
        assert_eq!(action, MultiLineInputAction::TextChanged);
        assert_eq!(ti.text(), "s");
    }

    #[test]
    fn esc_cancels() {
        let mut ti = MultiLineTextInput::new(Some("hello".to_string()));
        assert_eq!(
            ti.handle_input(&key(KeyCode::Esc)),
            MultiLineInputAction::Cancel
        );
    }

    #[test]
    fn backspace_at_line_start_merges_with_previous_line() {
        let mut ti = MultiLineTextInput::new(Some("hello\nworld".to_string()));
        ti.set_cursor(1, 0);
        let action = ti.handle_input(&key(KeyCode::Backspace));
        assert_eq!(action, MultiLineInputAction::TextChanged);
        assert_eq!(ti.text(), "helloworld");
        assert_eq!(ti.cursor_line(), 0);
        assert_eq!(ti.cursor_col(), 5);
    }

    #[test]
    fn backspace_at_very_start_is_noop() {
        let mut ti = MultiLineTextInput::new(Some("hello".to_string()));
        ti.set_cursor(0, 0);
        let action = ti.handle_input(&key(KeyCode::Backspace));
        assert_eq!(action, MultiLineInputAction::None);
        assert_eq!(ti.text(), "hello");
    }

    #[test]
    fn delete_at_line_end_merges_next_line() {
        let mut ti = MultiLineTextInput::new(Some("hello\nworld".to_string()));
        ti.set_cursor(0, 5);
        let action = ti.handle_input(&key(KeyCode::Delete));
        assert_eq!(action, MultiLineInputAction::TextChanged);
        assert_eq!(ti.text(), "helloworld");
        assert_eq!(ti.cursor_line(), 0);
        assert_eq!(ti.cursor_col(), 5);
    }

    #[test]
    fn delete_at_very_end_is_noop() {
        let mut ti = MultiLineTextInput::new(Some("hello".to_string()));
        ti.set_cursor(0, 5);
        let action = ti.handle_input(&key(KeyCode::Delete));
        assert_eq!(action, MultiLineInputAction::None);
        assert_eq!(ti.text(), "hello");
    }

    #[test]
    fn left_right_cross_line_boundaries() {
        let mut ti = MultiLineTextInput::new(Some("ab\ncd".to_string()));
        ti.set_cursor(1, 0);
        ti.handle_input(&key(KeyCode::Left));
        assert_eq!((ti.cursor_line(), ti.cursor_col()), (0, 2));

        ti.handle_input(&key(KeyCode::Right));
        assert_eq!((ti.cursor_line(), ti.cursor_col()), (1, 0));
    }

    #[test]
    fn up_down_move_between_logical_lines() {
        let mut ti = MultiLineTextInput::new(Some("aaa\nbbb\nccc".to_string()));
        ti.set_cursor(1, 2);
        ti.handle_input(&key(KeyCode::Up));
        assert_eq!(ti.cursor_line(), 0);
        ti.handle_input(&key(KeyCode::Down));
        assert_eq!(ti.cursor_line(), 1);
        ti.handle_input(&key(KeyCode::Down));
        assert_eq!(ti.cursor_line(), 2);
        // One more Down at the last line is a no-op (clamped).
        ti.handle_input(&key(KeyCode::Down));
        assert_eq!(ti.cursor_line(), 2);
    }

    #[test]
    fn up_down_preserve_visual_column_across_cjk_and_ascii_lines() {
        // "日本語" = 3 chars, width 6. ASCII "abcdef" = width 6 too, so a
        // char-count-only implementation happens to also work here — use a
        // narrower CJK line to force the two to disagree.
        let mut ti = MultiLineTextInput::new(Some("abcdef\n日本\nxyz".to_string()));
        // Start at the end of the ASCII line: visual column 6.
        ti.set_cursor(0, 6);
        ti.handle_input(&key(KeyCode::Down));
        // Line 1 is "日本" (width 4, 2 chars). Width-aware landing clamps to
        // the full line (col 2, width 4) since target width 6 > line width.
        assert_eq!(ti.cursor_line(), 1);
        assert_eq!(ti.cursor_col(), 2);

        // Now go from a narrow visual position on the CJK line back up: put
        // cursor after the first CJK char (col 1, visual width 2), move up to
        // "abcdef" — should land at char index 2 (visual width 2), not 1.
        ti.set_cursor(1, 1);
        ti.handle_input(&key(KeyCode::Up));
        assert_eq!(ti.cursor_line(), 0);
        assert_eq!(ti.cursor_col(), 2);
    }

    #[test]
    fn home_end_go_to_line_start_and_end() {
        let mut ti = MultiLineTextInput::new(Some("hello world".to_string()));
        ti.set_cursor(0, 5);
        ti.handle_input(&key(KeyCode::Home));
        assert_eq!(ti.cursor_col(), 0);
        ti.handle_input(&key(KeyCode::End));
        assert_eq!(ti.cursor_col(), 11);
    }

    #[test]
    fn insert_cjk_char_advances_cursor_by_one_char_not_by_width() {
        let mut ti = MultiLineTextInput::new(Some(String::new()));
        ti.handle_input(&key(KeyCode::Char('日')));
        ti.handle_input(&key(KeyCode::Char('本')));
        assert_eq!(ti.text(), "日本");
        assert_eq!(ti.cursor_col(), 2); // 2 characters, not display width 4
    }

    #[test]
    fn scroll_follows_cursor_downward_past_visible_height() {
        let mut ti = MultiLineTextInput::new(Some("l0\nl1\nl2\nl3\nl4".to_string()));
        ti.set_height(2);
        ti.set_cursor(0, 0);
        ti.set_scroll(0);
        // Move down 3 times: cursor line goes 0 -> 1 -> 2 -> 3.
        ti.handle_input(&key(KeyCode::Down));
        ti.handle_input(&key(KeyCode::Down));
        ti.handle_input(&key(KeyCode::Down));
        assert_eq!(ti.cursor_line(), 3);
        // With height 2, the scroll must have followed so the cursor line is
        // within [scroll, scroll+2).
        assert!(ti.scroll() <= ti.cursor_line());
        assert!(ti.cursor_line() < ti.scroll() + 2);
    }

    #[test]
    fn ctrl_char_does_not_insert_literal_character() {
        let mut ti = MultiLineTextInput::new(Some(String::new()));
        let action = ti.handle_input(&KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(action, MultiLineInputAction::None);
        assert_eq!(ti.text(), "");
    }

    /// A line starting with double-width CJK characters, cursor at the end,
    /// in a viewport too narrow to show the whole line — the horizontal
    /// scroll must be computed in display columns (2 per CJK char), not
    /// character counts, or it lands on the wrong slice of the line. Checked
    /// against actual rendered `Buffer` content, not internal fields, so a
    /// regression here is a real visible-output failure.
    #[test]
    fn render_horizontal_scroll_on_cjk_line_is_width_based() {
        use crate::ui::screen_text::buffer_to_text;
        use ratatui::{backend::TestBackend, Terminal};

        // "日本" = width 4, "ABCDEF" = width 6, total width 10.
        let mut ti = MultiLineTextInput::new(Some("日本ABCDEF".to_string()));
        ti.set_width(4);
        ti.set_height(1);
        let backend = TestBackend::new(4, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                ti.render(frame, area, true);
            })
            .expect("draw");
        let text = buffer_to_text(terminal.backend().buffer());
        // Cursor (at the end, after 'F') pushes the scroll window forward.
        // A char-count-based (rather than width-based) scroll calculation
        // would compute a different offset and show a different slice —
        // e.g. still including "日本" instead of having scrolled past it.
        assert!(
            text.starts_with("EF"),
            "expected the width-based scroll window to start at 'EF', got {text:?}"
        );
        assert!(
            !text.contains('日') && !text.contains('本'),
            "CJK prefix should have scrolled out of view, got {text:?}"
        );
    }
}
