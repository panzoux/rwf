//! Integration tests for configuration system
//! **Validates: Requirements 17.1-17.9, 38.1-38.10**

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, ConfigManager, ConfigError};
    use crate::state::{AppState, Transition, update_state};
    use tempfile::TempDir;
    use std::fs;
    
    #[test]
    fn test_config_loading_with_default_fallback() {
        // Test Requirement 17.1, 17.3: Load config or use defaults
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        
        // Load config when file doesn't exist - should return defaults
        let config = manager.load_config().unwrap();
        assert_eq!(config.worker_pool_size, 4);
        assert_eq!(config.session_persistence, true);
    }
    
    #[test]
    fn test_config_loading_from_file() {
        // Test Requirement 17.2: Load config from file if it exists
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        // Create a config file
        let mut config = AppConfig::default();
        config.worker_pool_size = 8;
        config.session_persistence = false;
        
        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);
        manager.save_config(&config).unwrap();
        
        // Load config
        let loaded_config = manager.load_config().unwrap();
        assert_eq!(loaded_config.worker_pool_size, 8);
        assert_eq!(loaded_config.session_persistence, false);
    }
    
    #[test]
    fn test_keybindings_loading() {
        // Test Requirement 17.4: Load key bindings from keybindings.json
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path.clone());
        
        // Load default keybindings
        let keybindings = manager.load_keybindings().unwrap();
        
        // Verify TWF-compatible defaults (Requirement 18.3)
        assert!(keybindings.normal_mode.contains_key("C"));
        assert!(keybindings.normal_mode.contains_key("M"));
        assert!(keybindings.normal_mode.contains_key("D"));
    }
    
    #[test]
    fn test_display_preferences_loading() {
        // Test Requirement 17.6: Load display preferences
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.display.show_hidden = true;
        config.display.cjk_width = 1;
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        manager.save_config(&config).unwrap();
        
        let loaded_config = manager.load_config().unwrap();
        assert_eq!(loaded_config.display.show_hidden, true);
        assert_eq!(loaded_config.display.cjk_width, 1);
    }
    
    #[test]
    fn test_color_scheme_loading() {
        // Test Requirement 17.7: Load color scheme
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.display.colors.foreground_color = "Green".to_string();
        config.display.colors.background_color = "Blue".to_string();
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        manager.save_config(&config).unwrap();
        
        let loaded_config = manager.load_config().unwrap();
        assert_eq!(loaded_config.display.colors.foreground_color, "Green");
        assert_eq!(loaded_config.display.colors.background_color, "Blue");
    }
    
    #[test]
    fn test_worker_pool_size_configuration() {
        // Test Requirement 17.8: Load Worker_Pool size configuration
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.worker_pool_size = 8;
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        manager.save_config(&config).unwrap();
        
        let loaded_config = manager.load_config().unwrap();
        assert_eq!(loaded_config.worker_pool_size, 8);
    }
    
    #[test]
    fn test_malformed_config_error() {
        // Test Requirement 17.9, 38.10: Display error for malformed config
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        // Write invalid JSON
        fs::write(&config_path, "{ invalid json }").unwrap();
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.load_config();
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ParseError(_)));
    }
    
    #[test]
    fn test_config_validation() {
        // Test Requirement 38.9: Validate configuration on load
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.worker_pool_size = 0; // Invalid
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        
        // Validation should fail
        let result = manager.validate_config(&config);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_config_reload_without_restart() {
        // Test Requirement 38.2: Reload config without restart (Shift+Z)
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);
        
        // Initial config
        let mut config = AppConfig::default();
        config.worker_pool_size = 4;
        manager.save_config(&config).unwrap();
        
        let mut state = AppState::new(config);
        assert_eq!(state.config.worker_pool_size, 4);
        
        // Modify config file
        let mut new_config = AppConfig::default();
        new_config.worker_pool_size = 8;
        manager.save_config(&new_config).unwrap();
        
        // Reload config (simulating Shift+Z)
        let loaded_config = manager.load_config().unwrap();
        update_state(&mut state, Transition::UpdateConfig {
            config: Box::new(loaded_config),
        });
        
        assert_eq!(state.config.worker_pool_size, 8);
    }
    
    #[test]
    fn test_invalid_config_handling() {
        // Test Requirement 38.10: Use default settings if config is malformed
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        // Write invalid JSON
        fs::write(&config_path, "{ invalid }").unwrap();
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.load_config();
        
        // Should return error, not default config
        assert!(result.is_err());
        
        // Application should handle this by using defaults
        let default_config = AppConfig::default();
        let state = AppState::new(default_config);
        assert_eq!(state.config.worker_pool_size, 4);
    }
    
    #[test]
    fn test_custom_key_mappings() {
        // Test Requirement 17.5, 18.2: Support custom key mappings
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let keybindings_json = r#"{
            "normal_mode": {
                "c": "Copy",
                "m": "Move"
            },
            "search_mode": {},
            "dialog_mode": {},
            "viewer_mode": {}
        }"#;
        
        fs::write(&keybindings_path, keybindings_json).unwrap();
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let keybindings = manager.load_keybindings().unwrap();
        
        assert!(keybindings.normal_mode.contains_key("c"));
        assert!(keybindings.normal_mode.contains_key("m"));
    }
    
    #[test]
    fn test_complete_config_workflow() {
        // Integration test for complete config workflow
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path.clone());
        
        // 1. Create and save config
        let mut config = AppConfig::default();
        config.worker_pool_size = 6;
        config.display.show_hidden = true;
        config.display.cjk_width = 1;
        manager.save_config(&config).unwrap();
        
        // 2. Load config
        let loaded_config = manager.load_config().unwrap();
        assert_eq!(loaded_config.worker_pool_size, 6);
        assert_eq!(loaded_config.display.show_hidden, true);
        assert_eq!(loaded_config.display.cjk_width, 1);
        
        // 3. Create state with config
        let mut state = AppState::new(loaded_config);
        assert_eq!(state.config.worker_pool_size, 6);
        
        // 4. Modify and reload config
        let mut new_config = AppConfig::default();
        new_config.worker_pool_size = 8;
        manager.save_config(&new_config).unwrap();
        
        let reloaded_config = manager.load_config().unwrap();
        update_state(&mut state, Transition::UpdateConfig {
            config: Box::new(reloaded_config),
        });
        
        assert_eq!(state.config.worker_pool_size, 8);
    }
    
    #[test]
    fn test_config_validation_cjk_width() {
        // Test CJK width validation
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.display.cjk_width = 3; // Invalid
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.validate_config(&config);
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ValidationError(_)));
    }
    
    #[test]
    fn test_config_validation_refresh_rate() {
        // Test UI refresh rate validation
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.ui.refresh_rate = 0; // Invalid
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.validate_config(&config);
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_config_validation_buffer_size() {
        // Test buffer size validation
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.file_operations.buffer_size = 0; // Invalid
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.validate_config(&config);
        
        assert!(result.is_err());
    }
}
