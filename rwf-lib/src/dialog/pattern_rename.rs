//! Pattern rename dialog content (TWF-style: separate Find/Replace fields).

#[derive(Debug, Clone)]
pub struct PatternRenameContent {
    pub find: String,
    pub find_cursor_pos: usize,
    pub find_scroll_pos: usize,
    pub replace: String,
    pub replace_cursor_pos: usize,
    pub replace_scroll_pos: usize,
    pub use_regex: bool,
    pub case_sensitive: bool,
    pub preview: Vec<(String, String)>,
    /// 0=find textbox, 1=replace textbox, 2=filelist
    pub focused_field: usize,
    pub preview_scroll: usize,
    pub preview_horizontal_scroll: usize,
    /// Non-None when a collision was detected at confirm time
    pub error_message: Option<String>,
    /// 0=SIDE-BY-SIDE, 1=Preview (new names), 2=Original (original names); Alt+P cycles
    pub preview_mode: u8,
    /// true=show all files, false=matching files only; Alt+A toggles
    pub show_all: bool,
}

impl PatternRenameContent {
    pub fn new() -> Self {
        Self {
            find: String::new(),
            find_cursor_pos: 0,
            find_scroll_pos: 0,
            replace: String::new(),
            replace_cursor_pos: 0,
            replace_scroll_pos: 0,
            use_regex: true,
            case_sensitive: false,
            preview: Vec::new(),
            focused_field: 0,
            preview_scroll: 0,
            preview_horizontal_scroll: 0,
            error_message: None,
            preview_mode: 0,
            show_all: true,
        }
    }
}

impl Default for PatternRenameContent {
    fn default() -> Self {
        Self::new()
    }
}
