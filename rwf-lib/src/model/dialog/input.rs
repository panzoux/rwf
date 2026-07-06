//! Input dialog content.

#[derive(Debug, Clone)]
pub struct InputDialog {
    /// Prompt text displayed above the input field
    pub prompt: String,
    /// Default value (used for initializing input and cursor)
    pub default_value: String,
    /// Current text being edited
    pub input: String,
    /// Cursor position (in character count, not bytes)
    pub cursor_pos: usize,
    /// Horizontal scroll position for the input field
    pub scroll_pos: usize,
}

impl InputDialog {
    pub fn new(prompt: String, default_value: String) -> Self {
        let input = default_value.clone();
        let cursor_pos = input.chars().count();
        let scroll_pos = 0;
        Self {
            prompt,
            default_value,
            input,
            cursor_pos,
            scroll_pos,
        }
    }
}
