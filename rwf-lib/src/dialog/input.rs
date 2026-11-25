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
    /// Inline validation error shown under the textbox.
    ///
    /// Set by `process_dialog_confirmation` when Enter is pressed on a name
    /// that can't be used (empty / invalid characters / already taken). The
    /// dialog stays open in that case (`suppress_next_dialog_pop`) so the user
    /// can correct the name instead of re-opening the dialog and retyping it.
    /// Cleared as soon as the text changes.
    pub error: Option<String>,
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
            error: None,
        }
    }
}

/// Characters that can never appear in a new file/directory name.
///
/// `< > : " | ? *` mirrors what `LocalFilesystemBackend::create_file` /
/// `create_directory` reject, so the dialog refuses exactly what the job
/// would have failed on. `/` and `\` are added on top: the name is joined
/// onto the pane's current location, so a separator would silently create
/// the entry somewhere else (or fail) rather than in the visible directory.
const INVALID_NAME_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Windows device names that can't be used as a file name in any directory,
/// with or without an extension (`CON`, `con.txt`, ... all fail). Checked on
/// every platform so the rule doesn't change with the build host — the names
/// are unlikely enough elsewhere that rejecting them costs nothing.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validate a name typed into the Create File / Create Directory dialog.
///
/// Pure (no filesystem access) — the "already exists" check needs the pane's
/// location and lives in the confirmation handler. Returns the message to show
/// inline in the dialog on rejection.
pub fn validate_new_entry_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if name == "." || name == ".." {
        return Err("'.' and '..' are reserved".to_string());
    }
    let invalid: Vec<char> = name
        .chars()
        .filter(|c| INVALID_NAME_CHARS.contains(c))
        .collect();
    if let Some(first) = invalid.first() {
        return Err(format!("Invalid character in name: {first}"));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("Control characters are not allowed in a name".to_string());
    }
    // Windows silently strips a trailing dot/space, so the created entry would
    // not have the name the user typed — reject instead of surprising them.
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("Name cannot end with '.' or a space".to_string());
    }
    let stem = name.split('.').next().unwrap_or(name);
    if RESERVED_NAMES.iter().any(|r| stem.eq_ignore_ascii_case(r)) {
        return Err(format!("'{stem}' is a reserved device name"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_dialog_starts_without_error() {
        let d = InputDialog::new("File name:".to_string(), String::new());
        assert!(d.error.is_none());
    }

    #[test]
    fn accepts_ordinary_names() {
        for name in ["notes.txt", "a", "日本語.md", "my file.rs", ".gitignore"] {
            assert!(
                validate_new_entry_name(name).is_ok(),
                "expected {name} to be accepted"
            );
        }
    }

    #[test]
    fn rejects_empty_and_whitespace() {
        assert!(validate_new_entry_name("").is_err());
        assert!(validate_new_entry_name("   ").is_err());
    }

    #[test]
    fn rejects_dot_entries() {
        assert!(validate_new_entry_name(".").is_err());
        assert!(validate_new_entry_name("..").is_err());
    }

    #[test]
    fn rejects_invalid_characters_including_separators() {
        for name in [
            "a<b",
            "a>b",
            "a:b",
            "a\"b",
            "a|b",
            "a?b",
            "a*b",
            "sub/name",
            "sub\\name",
        ] {
            let err = validate_new_entry_name(name)
                .expect_err(&format!("expected {name} to be rejected"));
            assert!(err.contains("Invalid character"), "unexpected error: {err}");
        }
    }

    #[test]
    fn rejects_control_characters() {
        assert!(validate_new_entry_name("a\tb").is_err());
        assert!(validate_new_entry_name("a\nb").is_err());
    }

    #[test]
    fn rejects_trailing_dot_or_space() {
        assert!(validate_new_entry_name("name.").is_err());
        assert!(validate_new_entry_name("name ").is_err());
    }

    #[test]
    fn rejects_reserved_device_names_with_and_without_extension() {
        assert!(validate_new_entry_name("CON").is_err());
        assert!(validate_new_entry_name("con.txt").is_err());
        assert!(validate_new_entry_name("LPT9").is_err());
        // Not reserved: the device name has to be the whole stem.
        assert!(validate_new_entry_name("console.txt").is_ok());
    }
}
