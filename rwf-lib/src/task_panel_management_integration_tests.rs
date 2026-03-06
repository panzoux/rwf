//! Integration tests for task panel management
//!
//! Tests Requirements 47.1-47.7:
//! - Toggle task panel visibility
//! - Resize task panel (increase/decrease height)
//! - Scroll task panel (up/down)
//! - Persist task panel settings across sessions

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::state::{AppState, Transition, update_state};
    use crate::job::{JobSpec, JobKind};
    use crate::model::Location;
    use std::path::PathBuf;

    #[test]
    fn test_toggle_task_panel_visibility() {
        // **Validates: Requirements 47.1**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Initially visible
        assert!(state.ui.layout.show_task_panel);
        
        // Toggle to hide
        let result = update_state(&mut state, Transition::ToggleTaskPanel);
        assert!(result.ui_changed);
        assert!(!state.ui.layout.show_task_panel);
        
        // Toggle to show
        let result = update_state(&mut state, Transition::ToggleTaskPanel);
        assert!(result.ui_changed);
        assert!(state.ui.layout.show_task_panel);
    }

    #[test]
    fn test_increase_task_panel_height() {
        // **Validates: Requirements 47.2**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Initial height is 5
        assert_eq!(state.ui.layout.task_panel_height, 5);
        
        // Increase height
        let result = update_state(&mut state, Transition::IncreaseTaskPanelHeight);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_height, 6);
        
        // Increase again
        let result = update_state(&mut state, Transition::IncreaseTaskPanelHeight);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_height, 7);
    }

    #[test]
    fn test_increase_task_panel_height_max_limit() {
        // **Validates: Requirements 47.2**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set height to max (20)
        state.ui.layout.task_panel_height = 20;
        
        // Try to increase beyond max
        let result = update_state(&mut state, Transition::IncreaseTaskPanelHeight);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_height, 20); // Should stay at max
    }

    #[test]
    fn test_decrease_task_panel_height() {
        // **Validates: Requirements 47.3**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set initial height to 7
        state.ui.layout.task_panel_height = 7;
        
        // Decrease height
        let result = update_state(&mut state, Transition::DecreaseTaskPanelHeight);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_height, 6);
        
        // Decrease again
        let result = update_state(&mut state, Transition::DecreaseTaskPanelHeight);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_height, 5);
    }

    #[test]
    fn test_decrease_task_panel_height_min_limit() {
        // **Validates: Requirements 47.3**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set height to min (3)
        state.ui.layout.task_panel_height = 3;
        
        // Try to decrease below min
        let result = update_state(&mut state, Transition::DecreaseTaskPanelHeight);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_height, 3); // Should stay at min
    }

    #[test]
    fn test_scroll_task_panel_up() {
        // **Validates: Requirements 47.4**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Set initial scroll offset
        state.ui.layout.task_panel_scroll_offset = 5;
        
        // Scroll up
        let result = update_state(&mut state, Transition::ScrollTaskPanelUp);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 4);
        
        // Scroll up again
        let result = update_state(&mut state, Transition::ScrollTaskPanelUp);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 3);
    }

    #[test]
    fn test_scroll_task_panel_up_at_top() {
        // **Validates: Requirements 47.4**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Already at top
        state.ui.layout.task_panel_scroll_offset = 0;
        
        // Try to scroll up
        let result = update_state(&mut state, Transition::ScrollTaskPanelUp);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 0); // Should stay at 0
    }

    #[test]
    fn test_scroll_task_panel_down() {
        // **Validates: Requirements 47.5**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add some jobs to enable scrolling
        for i in 0..10 {
            let job_spec = JobSpec::new(JobKind::ReadDirectory {
                location: Location::Local(PathBuf::from(format!("/test/{}", i))),
            });
            state.jobs.enqueue(job_spec);
        }
        
        // Initial scroll offset is 0
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 0);
        
        // Scroll down
        let result = update_state(&mut state, Transition::ScrollTaskPanelDown);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 1);
        
        // Scroll down again
        let result = update_state(&mut state, Transition::ScrollTaskPanelDown);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 2);
    }

    #[test]
    fn test_scroll_task_panel_down_at_bottom() {
        // **Validates: Requirements 47.5**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add some jobs
        for i in 0..10 {
            let job_spec = JobSpec::new(JobKind::ReadDirectory {
                location: Location::Local(PathBuf::from(format!("/test/{}", i))),
            });
            state.jobs.enqueue(job_spec);
        }
        
        // Set scroll offset to max (total_items - visible_height)
        let total_items = state.jobs.queue.len();
        let visible_height = state.ui.layout.task_panel_height;
        let max_scroll = total_items.saturating_sub(visible_height);
        state.ui.layout.task_panel_scroll_offset = max_scroll;
        
        // Try to scroll down
        let result = update_state(&mut state, Transition::ScrollTaskPanelDown);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, max_scroll); // Should stay at max
    }

    #[test]
    fn test_scroll_task_panel_no_scroll_when_few_items() {
        // **Validates: Requirements 47.5, 47.7**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add only 2 jobs (less than visible height of 5)
        for i in 0..2 {
            let job_spec = JobSpec::new(JobKind::ReadDirectory {
                location: Location::Local(PathBuf::from(format!("/test/{}", i))),
            });
            state.jobs.enqueue(job_spec);
        }
        
        // Try to scroll down
        let result = update_state(&mut state, Transition::ScrollTaskPanelDown);
        assert!(result.ui_changed);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 0); // Should not scroll
    }

    #[test]
    fn test_task_panel_settings_persistence() {
        // **Validates: Requirements 47.6**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Modify task panel settings
        state.ui.layout.show_task_panel = false;
        state.ui.layout.task_panel_height = 10;
        
        // Save session
        let session = crate::session::save_session(
            &state.tabs.tabs,
            state.tabs.active_index,
            state.ui.active_pane,
            &state.marking.marked_locations,
            state.ui.layout.show_task_panel,
            state.ui.layout.task_panel_height,
        );
        
        // Verify settings are saved
        assert!(!session.show_task_panel);
        assert_eq!(session.task_panel_height, 10);
        
        // Create new state and restore
        let config2 = AppConfig::default();
        let mut state2 = AppState::new(config2);
        
        // Manually restore task panel settings (simulating restore_session)
        state2.ui.layout.show_task_panel = session.show_task_panel;
        state2.ui.layout.task_panel_height = session.task_panel_height;
        
        // Verify settings are restored
        assert!(!state2.ui.layout.show_task_panel);
        assert_eq!(state2.ui.layout.task_panel_height, 10);
    }

    #[test]
    fn test_combined_task_panel_operations() {
        // **Validates: Requirements 47.1-47.7**
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add some jobs
        for i in 0..15 {
            let job_spec = JobSpec::new(JobKind::ReadDirectory {
                location: Location::Local(PathBuf::from(format!("/test/{}", i))),
            });
            state.jobs.enqueue(job_spec);
        }
        
        // Toggle visibility
        update_state(&mut state, Transition::ToggleTaskPanel);
        assert!(!state.ui.layout.show_task_panel);
        
        // Toggle back
        update_state(&mut state, Transition::ToggleTaskPanel);
        assert!(state.ui.layout.show_task_panel);
        
        // Increase height
        update_state(&mut state, Transition::IncreaseTaskPanelHeight);
        assert_eq!(state.ui.layout.task_panel_height, 6);
        
        // Scroll down
        update_state(&mut state, Transition::ScrollTaskPanelDown);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 1);
        
        // Scroll down again
        update_state(&mut state, Transition::ScrollTaskPanelDown);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 2);
        
        // Scroll up
        update_state(&mut state, Transition::ScrollTaskPanelUp);
        assert_eq!(state.ui.layout.task_panel_scroll_offset, 1);
        
        // Decrease height
        update_state(&mut state, Transition::DecreaseTaskPanelHeight);
        assert_eq!(state.ui.layout.task_panel_height, 5);
    }
}
