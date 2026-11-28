//! Sort dialog content.

/// State for the Sort dialog: highlighted sort key/order and section focus.
#[derive(Debug, Clone)]
pub struct SortDialog {
    /// Currently highlighted sort key (0=Name, 1=Size, 2=Date, 3=Extension)
    pub selected_mode_index: usize,
    /// Currently highlighted order (0=Ascending, 1=Descending)
    pub selected_order_index: usize,
    /// Which section has keyboard focus (0=sort-key list, 1=order list, 2=OK, 3=Cancel)
    pub focused_section: usize,
}

impl SortDialog {
    /// New sort dialog state pre-selected to the pane's current mode/order (focus starts at 0).
    pub fn new(selected_mode_index: usize, selected_order_index: usize) -> Self {
        Self {
            selected_mode_index,
            selected_order_index,
            focused_section: 0,
        }
    }
}
