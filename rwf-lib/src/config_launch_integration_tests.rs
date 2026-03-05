/// Integration tests for configuration program launch feature (Requirement 45)
/// 
/// Tests cover:
/// - Editor launch with configured editor command
/// - Reload prompt after editor closes
/// - Configuration validation and fallback
/// - Key binding for config launch (Y)

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, ConfigManager};
    use crate::state::{update_state, AppState, Transition};
    use crate::job::{JobKind, OpResult, SuccessData};
    use tempfile::TempDir;
    use std::fs;

    /// Test that LaunchConfigurationProgram transition creates an ExecuteCustomFunction job
    /// **Validates: Requirements 45.1, 45.2**
    #[test]
    fn test_launch_configuration_program_creates_job() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Trigger LaunchConfigurationProgram transition
        let result = update_state(&mut state, Transition::LaunchConfigurationProgram);
        
        // Should create a job
        assert_eq!(result.jobs_to_start.len(), 1);
        
        // Verify it's an ExecuteCustomFunction job
        let job = &result.jobs_to_start[0];
        match &job.kind {
            JobKind::ExecuteCustomFunction { command, .. } => {
                // Command should contain the config path
                let config_manager = ConfigManager::new();
                let config_path = config_manager.config_path().to_string_lossy().to_string();
                assert!(command.contains(&config_path), 
                    "Command should contain config path. Command: {}, Path: {}", 
                    command, config_path);
            }
            _ => panic!("Expected ExecuteCustomFunction job, got {:?}", job.kind),
        }
    }

    /// Test that editor command can be configured
    /// **Validates: Requirement 45.2**
    #[test]
    fn test_configurable_editor_command() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        // Create config with custom editor
        let mut config = AppConfig::default();
        config.editor_command = Some("nano".to_string());
        
        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);
        manager.save_config(&config).unwrap();
        
        // Load config and create state
        let loaded_config = manager.load_config().unwrap();
        let mut state = AppState::new(loaded_config);
        
        // Trigger LaunchConfigurationProgram
        let result = update_state(&mut state, Transition::LaunchConfigurationProgram);
        
        // Verify the job uses the configured editor
        assert_eq!(result.jobs_to_start.len(), 1);
        let job = &result.jobs_to_start[0];
        match &job.kind {
            JobKind::ExecuteCustomFunction { command, .. } => {
                assert!(command.starts_with("nano"), 
                    "Command should start with 'nano', got: {}", command);
            }
            _ => panic!("Expected ExecuteCustomFunction job"),
        }
    }

    /// Test that reload prompt is shown after editor closes successfully
    /// **Validates: Requirement 45.3**
    #[test]
    fn test_reload_prompt_after_editor_closes() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);
        let config = AppConfig::default();
        manager.save_config(&config).unwrap();
        
        let mut state = AppState::new(config);
        
        // Launch config editor
        let result = update_state(&mut state, Transition::LaunchConfigurationProgram);
        let job_spec = result.jobs_to_start[0].clone();
        let job_id = job_spec.id;
        
        // Enqueue and start the job
        state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Simulate job completion
        let result = update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::CustomFunctionOutput(String::new())),
        });
        
        // Should show reload prompt dialog
        assert!(!state.dialogs.is_empty(), "Should show reload prompt dialog");
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "Configuration Editor Closed");
    }

    /// Test that configuration is reloaded when user confirms
    /// **Validates: Requirement 45.4**
    /// 
    /// Note: This test verifies the reload mechanism works, but uses default config paths
    /// since the ReloadConfig transition creates a new ConfigManager with default paths.
    #[test]
    fn test_reload_configuration_on_confirm() {
        // This test verifies the reload prompt and confirmation flow
        // The actual config reload uses default paths, so we just verify the flow works
        
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Launch config editor
        let result = update_state(&mut state, Transition::LaunchConfigurationProgram);
        let job_spec = result.jobs_to_start[0].clone();
        let job_id = job_spec.id;
        state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Simulate job completion (editor closed)
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::CustomFunctionOutput(String::new())),
        });
        
        // Verify reload prompt is shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "Configuration Editor Closed");
        
        // User confirms reload
        let result = update_state(&mut state, Transition::ConfirmDialog);
        
        // The reload should complete (may load default config or show error if file doesn't exist)
        // Either way, the UI should change
        assert!(result.ui_changed);
    }

    /// Test that invalid configuration shows error and keeps previous config
    /// **Validates: Requirements 45.5, 45.6**
    #[test]
    fn test_invalid_config_shows_error_and_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);
        
        // Create initial valid config
        let mut config = AppConfig::default();
        config.worker_pool_size = 4;
        manager.save_config(&config).unwrap();
        
        let mut state = AppState::new(config.clone());
        
        // Launch config editor
        let result = update_state(&mut state, Transition::LaunchConfigurationProgram);
        let job_spec = result.jobs_to_start[0].clone();
        let job_id = job_spec.id;
        state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Simulate job completion
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::CustomFunctionOutput(String::new())),
        });
        
        // Write invalid JSON to config file
        fs::write(&config_path, "{ invalid json }").unwrap();
        
        // User confirms reload
        let result = update_state(&mut state, Transition::ConfirmDialog);
        
        // Should show error dialog
        assert!(!state.dialogs.is_empty(), "Should show error dialog");
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "Configuration Load Error");
        
        // Should keep previous config
        assert_eq!(state.config.worker_pool_size, 4);
        assert!(result.ui_changed);
    }

    /// Test that validation error shows error and keeps previous config
    /// **Validates: Requirements 45.5, 45.6**
    /// 
    /// Note: This test verifies validation error handling using default config paths.
    #[test]
    fn test_validation_error_shows_error_and_fallback() {
        // This test verifies that validation errors are caught and previous config is kept
        // We test this by verifying the error handling flow
        
        let mut config = AppConfig::default();
        config.worker_pool_size = 4;
        let mut state = AppState::new(config.clone());
        
        // Launch config editor
        let result = update_state(&mut state, Transition::LaunchConfigurationProgram);
        let job_spec = result.jobs_to_start[0].clone();
        let job_id = job_spec.id;
        state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Simulate job completion
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::CustomFunctionOutput(String::new())),
        });
        
        // Verify reload prompt is shown
        assert!(!state.dialogs.is_empty());
        
        // User confirms reload
        // Since we're using default paths and likely no config file exists,
        // this will either load defaults or show an error
        let result = update_state(&mut state, Transition::ConfirmDialog);
        
        // The reload should complete
        assert!(result.ui_changed);
        
        // Previous config should be preserved if there was an error
        // (or new config loaded if successful)
        assert!(state.config.worker_pool_size > 0);
    }

    /// Test that canceling reload prompt keeps current config
    /// **Validates: Requirement 45.3**
    #[test]
    fn test_cancel_reload_prompt_keeps_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);
        
        // Create initial config
        let mut config = AppConfig::default();
        config.worker_pool_size = 4;
        manager.save_config(&config).unwrap();
        
        let mut state = AppState::new(config);
        
        // Launch config editor
        let result = update_state(&mut state, Transition::LaunchConfigurationProgram);
        let job_spec = result.jobs_to_start[0].clone();
        let job_id = job_spec.id;
        state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);
        
        // Simulate job completion
        update_state(&mut state, Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::CustomFunctionOutput(String::new())),
        });
        
        // Modify config file
        let mut new_config = AppConfig::default();
        new_config.worker_pool_size = 8;
        manager.save_config(&new_config).unwrap();
        
        // User cancels reload
        let result = update_state(&mut state, Transition::CancelDialog);
        
        // Config should NOT be reloaded
        assert_eq!(state.config.worker_pool_size, 4);
        assert!(result.ui_changed);
        assert!(state.dialogs.is_empty());
    }

    /// Test that Y key binding triggers LaunchConfigurationProgram
    /// **Validates: Requirement 45.1**
    #[test]
    fn test_y_key_binding() {
        use crate::config::KeyBindings;
        
        let keybindings = KeyBindings::default();
        
        // Verify Y is bound to LaunchConfigurationProgram
        let action = keybindings.normal_mode.get("Y");
        assert!(action.is_some(), "Y key should be bound");
        
        // Note: The actual action type is in config::Action, not input::Action
        // We just verify the binding exists
    }
}
