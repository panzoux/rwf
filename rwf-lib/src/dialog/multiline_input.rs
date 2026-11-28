//! Multi-line input dialog content (Phase 7.17).
//!
//! A dedicated variant rather than a mode flag on `InputDialog`: `InputDialog`
//! is reused by several unrelated single-line callers (Register Folder,
//! Create Directory, Create File, Custom Function Input), and giving it a
//! multi-line branch would touch every one of those render/input paths for a
//! feature only one caller (the diagnostic report prompt) needs today. See
//! `docs/recipes/add-a-dialog.md`.

/// Multi-line input dialog: prompt + a small free-form text box.
#[derive(Debug, Clone)]
pub struct MultiLineInputDialog {
    /// Prompt text displayed above the input field.
    pub prompt: String,
    /// Current text, one entry per logical line (split on `\n`).
    pub lines: Vec<String>,
    /// Cursor line index into `lines`.
    pub cursor_line: usize,
    /// Cursor column — character index (not byte index) within `lines[cursor_line]`.
    pub cursor_col: usize,
    /// First visible line (vertical scroll position).
    pub scroll_row: usize,
}

impl MultiLineInputDialog {
    pub fn new(prompt: impl Into<String>, default_value: impl Into<String>) -> Self {
        let default_value: String = default_value.into();
        let mut lines: Vec<String> = default_value.split('\n').map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let cursor_line = lines.len() - 1;
        let cursor_col = lines[cursor_line].chars().count();
        Self {
            prompt: prompt.into(),
            lines,
            cursor_line,
            cursor_col,
            scroll_row: 0,
        }
    }

    /// Joined text, lines separated by `\n` — what confirmation handlers see.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_seeds_lines_and_cursor_at_end() {
        let d = MultiLineInputDialog::new("What happened?", "line one\nline two");
        assert_eq!(
            d.lines,
            vec!["line one".to_string(), "line two".to_string()]
        );
        assert_eq!(d.cursor_line, 1);
        assert_eq!(d.cursor_col, 8);
        assert_eq!(d.text(), "line one\nline two");
    }

    #[test]
    fn new_empty_default_is_single_empty_line() {
        let d = MultiLineInputDialog::new("Prompt", "");
        assert_eq!(d.lines, vec![String::new()]);
        assert_eq!(d.cursor_line, 0);
        assert_eq!(d.cursor_col, 0);
        assert_eq!(d.text(), "");
    }
}
