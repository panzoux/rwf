//! UI state

/// UI state
#[derive(Debug)]
pub struct UIState {
    pub active_pane: ActivePane,
    pub mode: UIMode,
    pub layout: LayoutState,
    /// Range marking mode state: stores the initial cursor position when entering range marking mode
    pub range_marking_start: Option<usize>,
    /// Whether to show hidden files
    pub show_hidden: bool,
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
            show_hidden: false,
        }
    }
}

/// How the file viewer is displayed when open
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ViewerLayout {
    /// Viewer occupies the full screen (current behaviour for "v")
    #[default]
    FullScreen,
    /// Viewer shares the screen with the file panes ("V")
    SideBySide,
}

/// Layout configuration
#[derive(Debug)]
pub struct LayoutState {
    pub pane_split_ratio: f64,
    pub show_status_bar: bool,
    pub show_task_panel: bool,
    pub show_tab_bar: bool,
    pub pane_height: usize,
    pub pane_width: usize,
    /// Task panel height in lines (default: 5)
    pub task_panel_height: usize,
    /// Task panel scroll offset (for scrolling through task history)
    pub task_panel_scroll_offset: usize,
    /// Current viewer layout (only meaningful when a viewer is open)
    pub viewer_layout: ViewerLayout,
    /// Remembered preference: the layout used when "v" opens a new viewer
    pub viewer_preferred_layout: ViewerLayout,
    /// Which file pane was active when the SideBySide viewer was opened.
    /// This is fixed for the lifetime of the SideBySide session so the viewer
    /// never jumps sides even if ui.active_pane changes.
    pub viewer_anchor_pane: ActivePane,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            pane_split_ratio: 0.5,
            show_status_bar: true,
            show_task_panel: true,
            show_tab_bar: true,
            pane_height: 20,
            pane_width: 80,
            task_panel_height: 5,
            task_panel_scroll_offset: 0,
            viewer_layout: ViewerLayout::FullScreen,
            viewer_preferred_layout: ViewerLayout::FullScreen,
            viewer_anchor_pane: ActivePane::Left,
        }
    }
}

/// Active pane identifier
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ActivePane {
    #[default]
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
    /// Viewer with the search input bar active (user is typing a query).
    ViewerSearch,
    /// Viewer with the command line active (line-jump, e.g. "100g").
    ViewerCommand,
}
