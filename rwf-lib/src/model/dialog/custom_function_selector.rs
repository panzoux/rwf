//! Custom function selector dialog content.

use super::CustomFunction;

#[derive(Debug, Clone)]
pub struct CustomFunctionSelectorContent {
    pub functions: Vec<CustomFunction>,
    pub filter: String,
    pub selected_index: usize,
    /// Built-in or user, by function name (Phase 7.26). Empty = show no marker.
    pub origins: std::collections::HashMap<String, crate::config_layers::ConfigOrigin>,
}

impl CustomFunctionSelectorContent {
    pub fn new(functions: Vec<CustomFunction>) -> Self {
        Self {
            functions,
            filter: String::new(),
            selected_index: 0,
            origins: std::collections::HashMap::new(),
        }
    }
}
