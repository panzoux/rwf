//! Navigation history dialog content.

use crate::model::Location;

#[derive(Debug, Clone)]
pub struct HistoryDialogContent {
    pub left_entries: Vec<Location>,
    pub right_entries: Vec<Location>,
    pub left_selected: usize,
    pub right_selected: usize,
    pub left_current_pos: usize,
    pub right_current_pos: usize,
    pub active_pane: crate::model::ui::ActivePane,
}

impl HistoryDialogContent {
    pub fn new(
        left_entries: Vec<Location>,
        right_entries: Vec<Location>,
        left_current_pos: usize,
        right_current_pos: usize,
        active_pane: crate::model::ui::ActivePane,
    ) -> Self {
        Self {
            left_entries,
            right_entries,
            left_selected: left_current_pos,
            right_selected: right_current_pos,
            left_current_pos,
            right_current_pos,
            active_pane,
        }
    }
}
