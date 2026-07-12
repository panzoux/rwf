//! Leap Navigation state

use std::path::PathBuf;

/// Result returned by `LeapState::backspace()`.
#[derive(Debug, PartialEq)]
pub enum BackspaceResult {
    /// Removed a regular character from the local filter.
    PopChar,
    /// Removed the '/' separator — caller must navigate to parent directory.
    GoToParent,
    /// Buffer was already empty; nothing changed.
    Empty,
}

/// State for the Leap navigation mode (F3).
///
/// The `buffer` encodes the full navigation path via '/' separators:
///   "mapm/ci/te"  →  trail="mapm/ci/"  local_filter="te"
#[derive(Debug, Clone)]
pub struct LeapState {
    /// Full buffer including '/' depth separators.
    pub buffer: String,
    /// Last buffer state that produced >= 1 match (for TaskPanel rollback).
    pub last_valid_buffer: String,
    /// Directory where F3 was pressed (for Ctrl+K / LeapClearAll).
    pub root_dir: PathBuf,
    /// Cursor index in the pane at the moment F3 was pressed (for Esc / LeapCancel).
    pub root_cursor: usize,
    /// Stack of (directory_path, buffer_len_at_entry) for each auto-entered directory.
    pub dir_stack: Vec<(PathBuf, usize)>,
}

impl LeapState {
    /// Create a new leap navigation state at the given directory with cursor position
    pub fn new(root_dir: PathBuf, root_cursor: usize) -> Self {
        Self {
            buffer: String::new(),
            last_valid_buffer: String::new(),
            root_dir,
            root_cursor,
            dir_stack: Vec::new(),
        }
    }

    /// The active local filter: everything after the last '/'.
    pub fn local_filter(&self) -> &str {
        match self.buffer.rfind('/') {
            Some(i) => &self.buffer[i + 1..],
            None => &self.buffer,
        }
    }

    /// The trail portion: everything up to and including the last '/' (may be empty).
    pub fn trail(&self) -> &str {
        match self.buffer.rfind('/') {
            Some(i) => &self.buffer[..=i],
            None => "",
        }
    }

    /// Append a character to the buffer (unconditionally — debounce decides validity).
    pub fn push_char(&mut self, c: char) {
        self.buffer.push(c);
    }

    /// Remove the last character.
    ///
    /// Returns `GoToParent` if the removed character was '/' (caller navigates up),
    /// `PopChar` for any other character, or `Empty` if the buffer was already empty.
    pub fn backspace(&mut self) -> BackspaceResult {
        match self.buffer.pop() {
            None => BackspaceResult::Empty,
            Some('/') => {
                self.dir_stack.pop();
                BackspaceResult::GoToParent
            }
            Some(_) => BackspaceResult::PopChar,
        }
    }

    /// Enter a directory: record it in the stack and append '/' to the buffer.
    pub fn push_separator(&mut self, entered_dir: PathBuf) {
        self.dir_stack.push((entered_dir, self.buffer.len()));
        self.buffer.push('/');
    }

    /// Go to parent directory (Left arrow): strip local filter + separator in one step.
    ///
    /// Returns `true` if we were inside a sub-directory and moved up,
    /// `false` if already at the leap root (no '/' in buffer).
    pub fn go_parent(&mut self) -> bool {
        if let Some(sep_idx) = self.buffer.rfind('/') {
            self.buffer.truncate(sep_idx);
            self.dir_stack.pop();
            true
        } else {
            false
        }
    }

    /// Clear the local filter only (Ctrl+U): everything after the last '/'.
    pub fn clear_local(&mut self) {
        if let Some(i) = self.buffer.rfind('/') {
            self.buffer.truncate(i + 1);
        } else {
            self.buffer.clear();
        }
    }

    /// Clear the entire buffer and dir_stack (Ctrl+K): return to leap root.
    pub fn clear_all(&mut self) {
        self.buffer.clear();
        self.last_valid_buffer.clear();
        self.dir_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> LeapState {
        LeapState::new(PathBuf::from("C:/test"), 0)
    }

    #[test]
    fn local_filter_empty_buffer() {
        let s = state();
        assert_eq!(s.local_filter(), "");
    }

    #[test]
    fn local_filter_no_separator() {
        let mut s = state();
        s.buffer = "mapm".to_string();
        assert_eq!(s.local_filter(), "mapm");
    }

    #[test]
    fn local_filter_with_trail() {
        let mut s = state();
        s.buffer = "mapm/ci/te".to_string();
        assert_eq!(s.local_filter(), "te");
        assert_eq!(s.trail(), "mapm/ci/");
    }

    #[test]
    fn backspace_regular_char() {
        let mut s = state();
        s.buffer = "ma".to_string();
        assert_eq!(s.backspace(), BackspaceResult::PopChar);
        assert_eq!(s.buffer, "m");
    }

    #[test]
    fn backspace_separator_pops_dir_stack() {
        let mut s = state();
        s.push_separator(PathBuf::from("C:/test/mapmodels"));
        s.push_char('c');
        s.backspace(); // pop 'c'
        assert_eq!(s.backspace(), BackspaceResult::GoToParent); // pop '/'
        assert!(s.dir_stack.is_empty());
    }

    #[test]
    fn backspace_empty_buffer() {
        let mut s = state();
        assert_eq!(s.backspace(), BackspaceResult::Empty);
    }

    #[test]
    fn clear_local_with_trail() {
        let mut s = state();
        s.buffer = "mapm/ci/te".to_string();
        s.clear_local();
        assert_eq!(s.buffer, "mapm/ci/");
    }

    #[test]
    fn clear_local_no_separator() {
        let mut s = state();
        s.buffer = "mapm".to_string();
        s.clear_local();
        assert_eq!(s.buffer, "");
    }

    #[test]
    fn go_parent_strips_local_and_sep() {
        let mut s = state();
        s.buffer = "mapm/we".to_string();
        s.dir_stack.push((PathBuf::from("C:/test/mapmodels"), 4));
        assert!(s.go_parent());
        assert_eq!(s.buffer, "mapm");
        assert!(s.dir_stack.is_empty());
    }

    #[test]
    fn go_parent_at_root_returns_false() {
        let mut s = state();
        s.buffer = "mapm".to_string();
        assert!(!s.go_parent());
    }

    #[test]
    fn clear_all_resets_everything() {
        let mut s = state();
        s.buffer = "mapm/ci/te".to_string();
        s.dir_stack.push((PathBuf::from("C:/test/mapmodels"), 4));
        s.dir_stack
            .push((PathBuf::from("C:/test/mapmodels/city"), 7));
        s.clear_all();
        assert_eq!(s.buffer, "");
        assert!(s.dir_stack.is_empty());
    }
}
