//! Rendered-screen capture as plain text (Phase 7.15 §5.3).
//!
//! After `terminal.draw(...)`, ratatui's back buffer *is* the screen. Turning
//! it into readable text is the whole of a TUI snapshot.
//!
//! # The wide-character trap
//!
//! A double-width glyph (CJK, some emoji) occupies **two** cells: the first
//! holds the symbol, the second is a continuation placeholder. Walking cells
//! one at a time and concatenating `Cell::symbol()` yields `"日 本 語"` — a
//! spurious space after every wide character, shifting the rest of the row and
//! silently misaligning any snapshot of a Japanese directory listing.
//!
//! **The placeholder is a single space, not an empty string** (ratatui fills it
//! via `Cell::reset()`), so it cannot be told apart from a real space by
//! inspecting the symbol. The only correct approach is to advance the column
//! cursor by the *display width* of each symbol, skipping however many cells
//! that glyph consumed.
//!
//! Since CJK alignment is the property RWF is built around, a snapshot that
//! mangles it would be worse than no snapshot: it would send an investigator
//! after a rendering bug that does not exist.

use ratatui::buffer::Buffer;
use unicode_width::UnicodeWidthStr;

/// Convert a rendered buffer to plain text, one line per terminal row.
///
/// Trailing whitespace is trimmed per line; the result has no trailing newline.
pub fn buffer_to_text(buffer: &Buffer) -> String {
    let width = buffer.area.width;
    let height = buffer.area.height;
    let mut out = String::new();

    for y in 0..height {
        let mut line = String::new();
        let mut x = 0u16;
        while x < width {
            let symbol = buffer[(buffer.area.x + x, buffer.area.y + y)].symbol();
            line.push_str(symbol);
            // Step over the cells this glyph occupies. `max(1)` guarantees
            // forward progress: a zero-width or empty symbol would otherwise
            // loop forever.
            x += (UnicodeWidthStr::width(symbol) as u16).max(1);
        }
        out.push_str(line.trim_end());
        if y + 1 < height {
            out.push('\n');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ratatui::text::Line;
    use ratatui::widgets::Paragraph;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_lines(width: u16, height: u16, lines: &[&str]) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let text: Vec<Line> = lines.iter().map(|l| Line::from(*l)).collect();
                frame.render_widget(Paragraph::new(text), Rect::new(0, 0, width, height));
            })
            .expect("draw");
        buffer_to_text(terminal.backend().buffer())
    }

    #[test]
    fn ascii_rows_round_trip() {
        let out = render_lines(20, 3, &["hello", "world"]);
        assert_eq!(out, "hello\nworld\n");
    }

    #[test]
    fn trailing_whitespace_is_trimmed() {
        let out = render_lines(30, 1, &["short"]);
        assert_eq!(out, "short");
    }

    /// The headline case. Each CJK glyph occupies two cells; the second is an
    /// empty continuation placeholder. If it were emitted as a space the text
    /// would come back as "日 本 語" and every column after it would shift.
    #[test]
    fn wide_cjk_glyphs_survive_without_padding() {
        let out = render_lines(20, 1, &["日本語"]);
        assert_eq!(out, "日本語");
    }

    #[test]
    fn mixed_ascii_and_cjk_keeps_ordering() {
        let out = render_lines(40, 1, &["dir/日本語のファイル.txt"]);
        assert_eq!(out, "dir/日本語のファイル.txt");
    }

    /// A realistic pane row: CJK filename followed by right-hand columns. A
    /// mishandled continuation cell shows up here as a shifted size column.
    #[test]
    fn cjk_followed_by_further_content_does_not_shift() {
        let out = render_lines(40, 1, &["日本語.txt  1.2K"]);
        assert_eq!(out, "日本語.txt  1.2K");
    }

    #[test]
    fn empty_rows_become_empty_lines() {
        let out = render_lines(10, 3, &["top"]);
        assert_eq!(out, "top\n\n");
    }

    #[test]
    fn every_row_is_represented() {
        let out = render_lines(10, 4, &["a", "b", "c", "d"]);
        assert_eq!(out.lines().count(), 4);
        assert_eq!(out, "a\nb\nc\nd");
    }
}
