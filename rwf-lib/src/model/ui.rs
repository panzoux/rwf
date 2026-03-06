//! UI state

/// UI state
#[derive(Debug)]
pub struct UIState {
    pub active_pane: ActivePane,
    pub mode: UIMode,
    pub layout: LayoutState,
    /// Range marking mode state: stores the initial cursor position when entering range marking mode
    pub range_marking_start: Option<usize>,
}

impl Default for UIState {
    fn default() -> Self {
        Self::new()
    }
}

impl UIState {
    pub fn new() -> Self {
        Self {
            active_pane: ActivePane::Left,
            mode: UIMode::Normal,
            layout: LayoutState::default(),
            range_marking_start: None,
        }
    }
}

/// Layout configuration
#[derive(Debug)]
pub struct LayoutState {
    pub pane_split_ratio: f64,
    pub show_status_bar: bool,
    pub show_task_panel: bool,
    pub show_tab_bar: bool,
    pub pane_height: usize,
    /// Task panel height in lines (default: 5)
    pub task_panel_height: usize,
    /// Task panel scroll offset (for scrolling through task history)
    pub task_panel_scroll_offset: usize,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            pane_split_ratio: 0.5,
            show_status_bar: true,
            show_task_panel: true,
            show_tab_bar: true,
            pane_height: 20, // Default fallback
            task_panel_height: 5,
            task_panel_scroll_offset: 0,
        }
    }
}

/// Active pane identifier
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivePane {
    Left,
    Right,
}

impl ActivePane {
    pub fn opposite(&self) -> Self {
        match self {
            ActivePane::Left => ActivePane::Right,
            ActivePane::Right => ActivePane::Left,
        }
    }
}

/// UI mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UIMode {
    Normal,
    Search,
    Command,
    Dialog,
    Viewer,
}
