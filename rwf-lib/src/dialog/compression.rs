//! Compression dialog content.

#[derive(Debug, Clone)]
pub struct CompressionDialog {
    // Data fields
    pub sources: Vec<crate::model::Location>,
    pub format: crate::input::ArchiveFormat,
    pub archive_name: String,
    pub selected_format_index: usize,
    pub selected_compression_index: usize,
    pub compression_level: u32,
    // Interaction state (persists while dialog is open)
    pub focused_field: usize, // 0=format, 1=compression, 2=name, 3=OK, 4=Cancel
    pub format_focus_index: usize, // Which format has focus (0-7)
    pub compression_focus_index: usize, // Which compression level has focus (0-5)
    pub cursor_pos: usize,    // Cursor position in archive name
    pub scroll_pos: usize,    // Scroll position in archive name
    // Vi mode support
    pub edit_mode: crate::config::EditMode,
    pub vi_mode: Option<crate::config::ViMode>,
    // Vi pending states (persisted between key presses)
    pub vi_pending_find_backward: Option<bool>,
    pub vi_pending_operator: Option<u8>, // 0=none, 1=change, 2=delete
    pub vi_pending_ctrl_x: bool,
    // Undo/Redo history
    pub history: Vec<String>,
    pub history_index: usize,
}

impl CompressionDialog {
    pub fn new(sources: Vec<crate::model::Location>, edit_mode: crate::config::EditMode) -> Self {
        let default_name = if sources.len() == 1 {
            let name = sources[0].display_path();
            // Get just the filename from the path
            std::path::Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("archive")
                .to_string()
        } else {
            "archive".to_string()
        };

        // Initialize vi_mode based on edit_mode
        let vi_mode = if edit_mode == crate::config::EditMode::Vi {
            Some(crate::config::ViMode::Normal)
        } else {
            None
        };

        Self {
            sources,
            format: crate::input::ArchiveFormat::ZIP,
            archive_name: default_name.clone(),
            selected_format_index: 0,
            selected_compression_index: 3, // Default to Normal
            compression_level: 5,          // Default to Normal
            // Initialize interaction state
            focused_field: 0,           // Start with format focused
            format_focus_index: 0,      // First format has focus
            compression_focus_index: 3, // Normal (5) has focus
            cursor_pos: default_name.chars().count(),
            scroll_pos: 0,
            edit_mode,
            vi_mode,
            // Initialize Vi pending states
            vi_pending_find_backward: None,
            vi_pending_operator: None,
            vi_pending_ctrl_x: false,
            // Initialize history
            history: vec![default_name],
            history_index: 0,
        }
    }
}
