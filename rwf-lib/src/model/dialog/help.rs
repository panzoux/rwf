//! Help dialog content.

use super::{HelpEntry, HelpTab};

#[derive(Debug, Clone)]
pub struct HelpDialog {
    /// All built-in + custom function help entries (built at dialog-open time)
    pub entries: Vec<HelpEntry>,
    /// Current search query
    pub query: String,
    /// True when Ctrl+R regex mode is active
    pub regex_mode: bool,
    /// Whether to show actions with no bound keys
    pub show_unbound: bool,
    /// Which tab is currently active
    pub active_tab: HelpTab,
    /// Scroll offset within the filtered list
    pub scroll_pos: usize,
    /// Active display language
    pub language: String,
    /// Timestamp of last query change (for debounce)
    pub last_query_change: Option<std::time::Instant>,
}

impl HelpDialog {
    pub fn new(language: String) -> Self {
        Self {
            entries: Vec::new(),
            query: String::new(),
            regex_mode: false,
            show_unbound: true,
            active_tab: HelpTab::NormalMode,
            scroll_pos: 0,
            language,
            last_query_change: None,
        }
    }
}
