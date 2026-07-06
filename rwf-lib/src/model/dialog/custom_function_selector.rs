//! Custom function selector dialog content.

use super::CustomFunction;

#[derive(Debug, Clone)]
pub struct CustomFunctionSelectorContent {
    pub functions: Vec<CustomFunction>,
    pub filter: String,
    pub selected_index: usize,
}

impl CustomFunctionSelectorContent {
    pub fn new(functions: Vec<CustomFunction>) -> Self {
        Self {
            functions,
            filter: String::new(),
            selected_index: 0,
        }
    }
}
