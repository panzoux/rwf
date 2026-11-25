//! Tab selector dialog content.

#[derive(Debug, Clone)]
pub struct TabSelectorContent {
    pub tabs: Vec<String>,
    pub selected_index: usize,
}

impl TabSelectorContent {
    pub fn new(tabs: Vec<String>) -> Self {
        Self {
            tabs,
            selected_index: 0,
        }
    }
}
