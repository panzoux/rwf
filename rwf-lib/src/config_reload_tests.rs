//! Tests for configuration reload functionality
//! **Validates: Requirements 38.2**

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, ConfigManager};
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::test_state;
    use tempfile::TempDir;

    #[test]
    fn test_reload_config_transition() {
        let mut state = test_state();

        let result = update_state(&mut state, Transition::ReloadConfig);
        assert!(result.ui_changed);
    }

    #[test]
    fn test_update_config_transition() {
        let mut state = test_state();

        assert_eq!(state.config.worker_pool_size, 4);

        let new_config = AppConfig {
            worker_pool_size: 8,
            session_persistence: false,
            ..Default::default()
        };

        let result = update_state(
            &mut state,
            Transition::UpdateConfig {
                config: Box::new(new_config.clone()),
            },
        );

        assert!(result.ui_changed);
        assert_eq!(state.config.worker_pool_size, 8);
        assert!(!state.config.session_persistence);
    }

    #[test]
    fn test_update_config_updates_job_manager() {
        let mut state = test_state();

        assert_eq!(state.jobs.max_parallel, 4);

        let new_config = AppConfig {
            worker_pool_size: 6,
            ..Default::default()
        };

        update_state(
            &mut state,
            Transition::UpdateConfig {
                config: Box::new(new_config),
            },
        );

        assert_eq!(state.jobs.max_parallel, 6);
    }

    #[test]
    fn test_config_reload_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");

        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);

        // Save initial config
        let config = AppConfig {
            worker_pool_size: 4,
            ..Default::default()
        };
        manager.save_config(&config).unwrap();

        // Create state with initial config
        let mut state = AppState::new(config);
        assert_eq!(state.config.worker_pool_size, 4);

        // Modify config file
        let new_config = AppConfig {
            worker_pool_size: 8,
            ..Default::default()
        };
        manager.save_config(&new_config).unwrap();

        // Reload config
        let loaded_config = manager.load_config().unwrap();
        update_state(
            &mut state,
            Transition::UpdateConfig {
                config: Box::new(loaded_config),
            },
        );

        assert_eq!(state.config.worker_pool_size, 8);
    }

    #[test]
    fn test_config_reload_preserves_state() {
        let mut state = test_state();

        // Add some state
        update_state(&mut state, Transition::CreateTab);
        assert_eq!(state.tabs.tabs.len(), 2);

        // Reload config
        let new_config = AppConfig {
            worker_pool_size: 6,
            ..Default::default()
        };

        update_state(
            &mut state,
            Transition::UpdateConfig {
                config: Box::new(new_config),
            },
        );

        // Verify state is preserved
        assert_eq!(state.tabs.tabs.len(), 2);
        assert_eq!(state.config.worker_pool_size, 6);
    }

    #[test]
    fn test_config_reload_without_restart() {
        let mut state = test_state();

        // Simulate application running with some state
        update_state(&mut state, Transition::CreateTab);
        state.last_tab_created = None;
        update_state(&mut state, Transition::CreateTab);

        // Reload config (simulating Shift+Z)
        let mut new_config = AppConfig {
            worker_pool_size: 8,
            ..Default::default()
        };
        new_config.display.show_hidden = true;

        update_state(
            &mut state,
            Transition::UpdateConfig {
                config: Box::new(new_config),
            },
        );

        // Verify config updated without losing state
        assert_eq!(state.tabs.tabs.len(), 3);
        assert_eq!(state.config.worker_pool_size, 8);
        assert!(state.config.display.show_hidden);
    }
}
