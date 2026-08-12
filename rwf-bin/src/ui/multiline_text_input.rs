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
//! - **Soft line-wrap with a `↵` marker.** A logical line too wide for the box
//!   continues on the next row. The rightmost column is reserved: `↵` there
//!   means a real newline ends that row, and its absence means the row wrapped.
//!   Without that distinction a reader cannot tell an entered line break from a
//!   wrapped one — which matters when the text is a bug report someone else
//!   reads.
//!
//!   The first implementation scrolled long lines horizontally instead. Dogfooding
//!   on 2026-08-12 rejected it: a single line sliding sideways under the cursor
//!   fights what a text box is expected to do, and nothing indicated that content
//!   existed beyond the edge. Wrapping removes the hidden-content problem
//!   entirely rather than signposting it.
//!
//!   The cost is that Up/Down must walk **visual** rows, so navigation and
//!   rendering have to agree on the wrap boundaries. They do, because both call
//!   [`MultiLineTextInput::visual_rows`] — the boundaries are computed once, in
//!   one place, rather than derived independently on each side.
//! - **CJK width awareness.** All cursor positioning — including the
//!   sticky-column behaviour of Up/Down between rows of different content, and
//!   the wrap points themselves — is computed from `unicode_width` display
//!   columns, never character counts or byte offsets. A double-width character
//!   is never split across rows.
//! - **Vertical scroll is persisted, wrapping is not.** The viewport position
//!   must be remembered so it does not re-snap on every keystroke; the wrap
//!   layout is cheap to recompute and cannot go stale if it never persists.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
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

/// One rendered row: the slice of a logical line that fits the wrap width.
///
/// A logical line always produces at least one row, so an empty line still
/// occupies a row and can hold the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisualRow {
    /// Index into `lines`.
    pub line: usize,
    /// First character index of this slice.
    pub start: usize,
    /// One past the last character index of this slice.
    pub end: usize,
    /// Whether this is the final slice of its logical line — i.e. whether a real
    /// newline follows. Drives the `↵` marker.
    pub last_of_line: bool,
}

// Soft wrapping
impl MultiLineTextInput {
    /// Columns available for text. The rightmost column is reserved for the `↵`
    /// marker so it never collides with content.
    fn wrap_width(&self) -> usize {
        (self.width as usize).saturating_sub(1).max(1)
    }

    /// Split one logical line into character ranges that each fit `width`
    /// display columns.
    ///
    /// Greedy by display width, never splitting a double-width glyph: a wide
    /// character that would straddle the boundary starts the next row instead.
    fn wrap_line(line: &str, width: usize) -> Vec<(usize, usize)> {
        let width = width.max(1);
        let mut rows = Vec::new();
        let mut start = 0usize;
        let mut used = 0usize;
        let mut idx = 0usize;

        for c in line.chars() {
            let cw = Self::char_width(c);
            if used + cw > width && idx > start {
                rows.push((start, idx));
                start = idx;
                used = 0;
            }
            used += cw;
            idx += 1;
        }
        // Always emit a final row, including for an empty line.
        rows.push((start, idx));
        rows
    }

    /// Every visual row of the whole buffer, in order.
    pub(crate) fn visual_rows(&self) -> Vec<VisualRow> {
        let width = self.wrap_width();
        let last_line = self.lines.len().saturating_sub(1);
        let mut rows = Vec::new();
        for (line_idx, line) in self.lines.iter().enumerate() {
            let segments = Self::wrap_line(line, width);
            let last_seg = segments.len() - 1;
            for (seg_idx, (start, end)) in segments.into_iter().enumerate() {
                rows.push(VisualRow {
                    line: line_idx,
                    start,
                    end,
                    // The final logical line has no newline after it, so it never
                    // shows the marker.
                    last_of_line: seg_idx == last_seg && line_idx < last_line,
                });
            }
        }
        rows
    }

    /// Index into [`visual_rows`] holding the cursor.
    ///
    /// When the cursor sits exactly at a wrap boundary it belongs to the row that
    /// *ends* there, which is where it is drawn.
    pub(crate) fn cursor_visual_row(&self, rows: &[VisualRow]) -> usize {
        let mut fallback = 0;
        for (i, r) in rows.iter().enumerate() {
            if r.line != self.cursor_line {
                continue;
            }
            fallback = i;
            if self.cursor_col >= r.start && self.cursor_col < r.end {
                return i;
            }
        }
        // Cursor at end-of-line: the last row of that line.
        fallback
    }
}

// Vertical scroll
impl MultiLineTextInput {
    /// Keep the cursor's *visual* row inside the viewport.
    ///
    /// Scrolling counts wrapped rows, not logical lines: with wrapping enabled a
    /// single long line can fill the box on its own, and a logical-line scroll
    /// would leave the cursor off-screen.
    fn update_scroll(&mut self) {
        let h = self.height.max(1) as usize;
        let rows = self.visual_rows();
        let cursor_row = self.cursor_visual_row(&rows);
        if cursor_row < self.scroll_row {
            self.scroll_row = cursor_row;
        } else if cursor_row >= self.scroll_row + h {
            self.scroll_row = cursor_row + 1 - h;
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
    /// Move the cursor one **visual** row up or down.
    ///
    /// With soft wrapping, Up/Down must step through wrapped rows rather than
    /// logical lines — otherwise a long wrapped paragraph would be skipped in a
    /// single keypress and the cursor would appear to jump several rows.
    ///
    /// The target column is chosen by display width offset *within the row*, so
    /// the cursor keeps its visual column across rows of differing content —
    /// including between an ASCII row and a CJK one.
    fn move_vertical(&mut self, delta: isize) {
        let rows = self.visual_rows();
        let current = self.cursor_visual_row(&rows);
        let target = current as isize + delta;
        if target < 0 || target as usize >= rows.len() {
            return;
        }
        let target = target as usize;

        let cur_row = rows[current];
        let cur_line = &self.lines[cur_row.line];
        // Offset from the start of the current row, in display columns.
        let offset = Self::width_upto(cur_line, self.cursor_col)
            .saturating_sub(Self::width_upto(cur_line, cur_row.start));

        let dst = rows[target];
        let dst_line = &self.lines[dst.line];
        let dst_row_start_width = Self::width_upto(dst_line, dst.start);
        let col = Self::col_for_width(dst_line, dst_row_start_width + offset);

        self.cursor_line = dst.line;
        self.cursor_col = col.clamp(dst.start, dst.end);
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
        // Dim so it reads as structure rather than as typed content.
        let mark_style = base_style.add_modifier(Modifier::DIM);

        let total_width = area.width as usize;
        let wrap_width = self.wrap_width();
        let rows = self.visual_rows();
        let cursor_row = if is_focused {
            Some(self.cursor_visual_row(&rows))
        } else {
            None
        };

        for screen_row in 0..area.height as usize {
            let y = area.y + screen_row as u16;
            let row_idx = self.scroll_row + screen_row;

            let Some(row) = rows.get(row_idx) else {
                frame.render_widget(
                    Span::styled(" ".repeat(total_width), base_style),
                    Rect::new(area.x, y, area.width, 1),
                );
                continue;
            };

            let line = &self.lines[row.line];
            let mut spans: Vec<Span> = Vec::new();
            let mut used = 0usize;

            for (i, c) in line.chars().enumerate().take(row.end).skip(row.start) {
                let is_cursor = cursor_row == Some(row_idx) && i == self.cursor_col;
                let style = if is_cursor { cursor_style } else { base_style };
                used += Self::char_width(c);
                spans.push(Span::styled(c.to_string(), style));
            }

            // Cursor resting at the end of this row (end of line, or at a wrap
            // boundary that this row terminates) is drawn as a block after the
            // text — otherwise it would be invisible.
            let cursor_at_row_end =
                cursor_row == Some(row_idx) && self.cursor_col >= row.end && used < wrap_width;
            if cursor_at_row_end {
                // A block glyph rather than a styled space: a space would rely
                // entirely on background colour, which is invisible in the
                // text-only snapshot dumps the dialog tests compare against.
                spans.push(Span::styled("█", cursor_style));
                used += 1;
            }

            if used < wrap_width {
                spans.push(Span::styled(" ".repeat(wrap_width - used), base_style));
                used = wrap_width;
            }

            // Reserved rightmost column: `↵` marks a real newline, so a wrapped
            // row (no marker) is distinguishable from an entered one. This is the
            // whole reason the column is reserved.
            let marker = if row.last_of_line { "↵" } else { " " };
            spans.push(Span::styled(marker, mark_style));
            used += 1;

            if used < total_width {
                spans.push(Span::styled(" ".repeat(total_width - used), base_style));
            }

            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(area.x, y, area.width, 1),
            );
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
    /// Replaces an earlier `render_horizontal_scroll_on_cjk_line_is_width_based`
    /// test: horizontal scrolling was removed in favour of soft wrapping after
    /// dogfooding on 2026-08-12 found a single line sliding sideways under the
    /// cursor to be the wrong behaviour for a text box.
    #[test]
    fn long_line_wraps_instead_of_scrolling_horizontally() {
        let mut ti = MultiLineTextInput::new(Some("ABCDEFGH".to_string()));
        ti.set_width(5); // 4 text columns + 1 reserved for the marker
        ti.set_height(4);

        let rows = ti.visual_rows();
        assert_eq!(rows.len(), 2, "8 chars over 4 columns must occupy 2 rows");
        assert_eq!((rows[0].start, rows[0].end), (0, 4));
        assert_eq!((rows[1].start, rows[1].end), (4, 8));
        assert!(
            rows.iter().all(|r| r.line == 0),
            "both rows belong to the same logical line"
        );
    }

    /// A double-width glyph must move to the next row whole rather than being
    /// split across the boundary — the failure mode that makes CJK text unreadable.
    #[test]
    fn wide_glyph_is_never_split_across_wrapped_rows() {
        // 4 text columns; "A" (1) + "日" (2) = 3, so the next "日" cannot fit.
        let mut ti = MultiLineTextInput::new(Some("A日日".to_string()));
        ti.set_width(5);
        ti.set_height(4);

        let rows = ti.visual_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].start, rows[0].end),
            (0, 2),
            "row 1 holds 'A日' (3 columns); a 4th column cannot hold half of '日'"
        );
        assert_eq!((rows[1].start, rows[1].end), (2, 3));
    }

    /// The `↵` marker is the whole point of reserving a column: it is what
    /// distinguishes a line the user ended from one the box wrapped.
    #[test]
    fn newline_marker_marks_entered_lines_not_wrapped_ones() {
        // "hi" fits one row; "ABCDEFGH" needs two at 4 text columns.
        let mut ti = MultiLineTextInput::new(Some("ABCDEFGH\nhi".to_string()));
        ti.set_width(5);
        ti.set_height(6);

        let rows = ti.visual_rows();
        assert_eq!(rows.len(), 3, "2 wrapped rows + 1 for the second line");
        assert!(!rows[0].last_of_line, "wrapped row: no newline follows it");
        assert!(rows[1].last_of_line, "a real newline ends the first line");
        assert!(
            !rows[2].last_of_line,
            "the final line has no newline after it"
        );
    }

    /// Up/Down must step one *visual* row. Stepping logical lines would jump the
    /// cursor over an entire wrapped paragraph in a single keypress.
    #[test]
    fn up_down_walk_visual_rows_inside_one_wrapped_line() {
        let mut ti = MultiLineTextInput::new(Some("ABCDEFGH".to_string()));
        ti.set_width(5);
        ti.set_height(4);
        ti.set_cursor(0, 1); // row 0, one column in

        ti.handle_input(&key(KeyCode::Down));
        assert_eq!(ti.cursor_line(), 0, "still the same logical line");
        assert_eq!(
            ti.cursor_col(),
            5,
            "moved to the second visual row, keeping the visual column"
        );

        ti.handle_input(&key(KeyCode::Up));
        assert_eq!(ti.cursor_col(), 1, "and back again");
    }

    /// Scrolling counts visual rows: one long line can fill the box by itself,
    /// and a logical-line scroll would leave the cursor off-screen.
    #[test]
    fn scroll_counts_visual_rows_not_logical_lines() {
        let mut ti = MultiLineTextInput::new(Some("A".repeat(20)));
        ti.set_width(5); // 4 text columns => 5 visual rows
        ti.set_height(2);
        ti.set_cursor(0, 0);
        ti.set_scroll(0);

        for _ in 0..4 {
            ti.handle_input(&key(KeyCode::Down));
        }

        assert!(
            ti.scroll() > 0,
            "cursor moved past the 2-row viewport within one logical line, so the \
             view must have scrolled; got scroll={}",
            ti.scroll()
        );
    }
}
