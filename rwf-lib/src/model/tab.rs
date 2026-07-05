//! Tab management

use super::ui::{ActivePane, ViewerLayout};
use super::{Location, NavigationHistory, PaneModel};
use std::path::PathBuf;

/// Viewer state saved for a tab that is not currently active.
/// When a tab becomes active, this is moved into AppState fields.
/// When a tab is deactivated, AppState fields are moved here.
#[derive(Debug, Default)]
pub struct TabViewerState {
    pub viewer: Option<crate::model::viewer::ViewerState>,
    pub viewer_job_id: Option<crate::job::JobId>,
    pub viewer_search_job_id: Option<crate::job::JobId>,
    pub viewer_layout: ViewerLayout,
    pub viewer_preferred_layout: ViewerLayout,
    pub viewer_anchor_pane: ActivePane,
    /// Whether the tab was in viewer-focus mode (UIMode::Viewer/Search/Command)
    pub viewer_was_focused: bool,
    pub viewer_search_input: String,
    pub viewer_command_input: String,
}

/// State for a single tab
#[derive(Debug)]
pub struct TabState {
    pub id: usize,
    pub left_pane: PaneModel,
    pub right_pane: PaneModel,
    pub history: NavigationHistory,
    /// Viewer state saved while this tab is not active.
    pub tab_viewer: TabViewerState,
}

impl TabState {
    pub fn new(id: usize) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        tracing::debug!(
            "[TabState::new] Initializing tab id={} with CWD={:?}",
            id,
            cwd
        );
        Self {
            id,
            left_pane: PaneModel::new(Location::Local(cwd.clone())),
            right_pane: PaneModel::new(Location::Local(cwd)),
            history: NavigationHistory::new(),
            tab_viewer: TabViewerState::default(),
        }
    }
}

/// Manages multiple tabs
#[derive(Debug)]
pub struct TabManager {
    pub tabs: Vec<TabState>,
    pub active_index: usize,
    next_tab_id: usize,
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TabManager {
    pub fn new() -> Self {
        let initial_tab = TabState::new(0);
        Self {
            tabs: vec![initial_tab],
            active_index: 0,
            next_tab_id: 1,
        }
    }

    /// Create a new tab
    pub fn create_tab(&mut self) -> usize {
        let new_id = self.next_tab_id;
        let new_tab = TabState::new(new_id);
        self.tabs.push(new_tab);
        self.next_tab_id += 1;
        self.tabs.len() - 1 // Return the new index
    }

    /// Update next_tab_id after session restore to prevent ID conflicts
    pub fn update_next_id_after_restore(&mut self) {
        if let Some(max_id) = self.tabs.iter().map(|t| t.id).max() {
            self.next_tab_id = max_id + 1;
        }
    }

    /// Close a tab by index
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 {
            return false; // Cannot close last tab
        }

        self.tabs.remove(index);

        // Adjust active index if necessary
        if self.active_index >= self.tabs.len() {
            self.active_index = self.tabs.len() - 1;
        }

        true
    }

    /// Switch to next tab
    pub fn switch_to_next(&mut self) {
        self.active_index = (self.active_index + 1) % self.tabs.len();
    }

    /// Switch to previous tab
    pub fn switch_to_prev(&mut self) {
        if self.active_index == 0 {
            self.active_index = self.tabs.len() - 1;
        } else {
            self.active_index -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_manager_initialization() {
        let manager = TabManager::new();
        assert_eq!(manager.tabs.len(), 1);
        assert_eq!(manager.active_index, 0);
        assert_eq!(manager.tabs[0].id, 0);
    }

    #[test]
    fn test_create_tab() {
        let mut manager = TabManager::new();
        let new_id = manager.create_tab();
        assert_eq!(new_id, 1);
        assert_eq!(manager.tabs.len(), 2);
        assert_eq!(manager.tabs[1].id, 1);
    }

    #[test]
    fn test_close_tab() {
        let mut manager = TabManager::new();
        manager.create_tab();
        manager.create_tab();

        assert_eq!(manager.tabs.len(), 3);

        // Close middle tab
        let result = manager.close_tab(1);
        assert!(result);
        assert_eq!(manager.tabs.len(), 2);
    }

    #[test]
    fn test_cannot_close_last_tab() {
        let mut manager = TabManager::new();
        let result = manager.close_tab(0);
        assert!(!result);
        assert_eq!(manager.tabs.len(), 1);
    }

    #[test]
    fn test_close_tab_adjusts_active_index() {
        let mut manager = TabManager::new();
        manager.create_tab();
        manager.create_tab();
        manager.active_index = 2;

        // Close the active tab
        manager.close_tab(2);

        // Active index should be adjusted to last tab
        assert_eq!(manager.active_index, 1);
    }

    #[test]
    fn test_switch_to_next() {
        let mut manager = TabManager::new();
        manager.create_tab();
        manager.create_tab();

        assert_eq!(manager.active_index, 0);

        manager.switch_to_next();
        assert_eq!(manager.active_index, 1);

        manager.switch_to_next();
        assert_eq!(manager.active_index, 2);

        // Should wrap around
        manager.switch_to_next();
        assert_eq!(manager.active_index, 0);
    }

    #[test]
    fn test_switch_to_prev() {
        let mut manager = TabManager::new();
        manager.create_tab();
        manager.create_tab();

        assert_eq!(manager.active_index, 0);

        // Should wrap around to last tab
        manager.switch_to_prev();
        assert_eq!(manager.active_index, 2);

        manager.switch_to_prev();
        assert_eq!(manager.active_index, 1);

        manager.switch_to_prev();
        assert_eq!(manager.active_index, 0);
    }

    #[test]
    fn test_tab_state_initialization() {
        let tab = TabState::new(5);
        assert_eq!(tab.id, 5);
        assert_eq!(tab.left_pane.entries.len(), 0);
        assert_eq!(tab.right_pane.entries.len(), 0);
    }
}
