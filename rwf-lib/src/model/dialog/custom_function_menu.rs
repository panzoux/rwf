//! Custom function menu dialog content (second-level menu from a menu-type entry).

use super::MenuItem;

#[derive(Debug, Clone)]
pub struct CustomFunctionMenuDialog {
    pub items: Vec<MenuItem>,
    pub selected_index: usize,
}

impl CustomFunctionMenuDialog {
    /// Selects the first selectable item (skipping separators) by default.
    pub fn new(items: Vec<MenuItem>) -> Self {
        let first_sel = items.iter().position(|i| i.is_selectable()).unwrap_or(0);
        Self {
            items,
            selected_index: first_sel,
        }
    }
}
