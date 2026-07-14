//! Integration tests for configuration system
//! **Validates: Requirements 17.1-17.9, 38.1-38.10**

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, ConfigError, ConfigManager};
    use crate::state::{update_state, AppState, Transition};
    use std::fs;
    use tempfile::TempDir;

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
        assert!(config.session_persistence);
    }

    #[test]
    fn test_config_loading_from_file() {
        // Test Requirement 17.2: Load config from file if it exists
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");

        // Create a config file
        let config = AppConfig {
            worker_pool_size: 8,
            session_persistence: false,
            ..Default::default()
        };

        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);
        manager.save_config(&config).unwrap();

        // Load config
        let loaded_config = manager.load_config().unwrap();
        assert_eq!(loaded_config.worker_pool_size, 8);
        assert!(!loaded_config.session_persistence);
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
        assert!(loaded_config.display.show_hidden);
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

        let config = AppConfig {
            worker_pool_size: 8,
            ..Default::default()
        };

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

        let config = AppConfig {
            worker_pool_size: 0, // Invalid
            ..Default::default()
        };

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
        let config = AppConfig {
            worker_pool_size: 4,
            ..Default::default()
        };
        manager.save_config(&config).unwrap();

        let mut state = AppState::new(config);
        assert_eq!(state.config.worker_pool_size, 4);

        // Modify config file
        let new_config = AppConfig {
            worker_pool_size: 8,
            ..Default::default()
        };
        manager.save_config(&new_config).unwrap();

        // Reload config (simulating Shift+Z)
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
            "NormalMode": {
                "c": "Copy",
                "m": "Move"
            },
            "SearchMode": {},
            "DialogMode": {},
            "ViewerMode": {}
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
        let mut config = AppConfig {
            worker_pool_size: 6,
            ..Default::default()
        };
        config.display.show_hidden = true;
        config.display.cjk_width = 1;
        manager.save_config(&config).unwrap();

        // 2. Load config
        let loaded_config = manager.load_config().unwrap();
        assert_eq!(loaded_config.worker_pool_size, 6);
        assert!(loaded_config.display.show_hidden);
        assert_eq!(loaded_config.display.cjk_width, 1);

        // 3. Create state with config
        let mut state = AppState::new(loaded_config);
        assert_eq!(state.config.worker_pool_size, 6);

        // 4. Modify and reload config
        let new_config = AppConfig {
            worker_pool_size: 8,
            ..Default::default()
        };
        manager.save_config(&new_config).unwrap();

        let reloaded_config = manager.load_config().unwrap();
        update_state(
            &mut state,
            Transition::UpdateConfig {
                config: Box::new(reloaded_config),
            },
        );

        assert_eq!(state.config.worker_pool_size, 8);
    }

    #[test]
    fn test_twf_config_format() {
        // Test loading TWF-style config with colors directly under Display (not nested)
        // This verifies that #[serde(flatten)] works correctly
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");

        // Create a TWF-style config with colors directly under Display
        let config_json = r#"{
            "Display": {
                "ShowHiddenFiles": false,
                "ShowSystem": false,
                "DateFormat": "%Y-%m-%d %H:%M",
                "TimeFormat": "TwentyFourHour",
                "CjkWidth": 2,
                "ForegroundColor": "White",
                "BackgroundColor": "Black",
                "HighlightForegroundColor": "Black",
                "HighlightBackgroundColor": "Cyan",
                "PaneInfoForegroundColor": "Black",
                "PaneInfoBackgroundColor": "Gray",
                "InactiveFilePaneCursorForegroundColor": "Black",
                "InactiveFilePaneCursorBackgroundColor": "DarkGray",
                "MarkedFileColor": "Cyan",
                "DirectoryColor": "BrightCyan",
                "DirectoryBackgroundColor": "Black",
                "InactiveDirectoryColor": "Cyan",
                "InactiveDirectoryBackgroundColor": "Black",
                "FilenameLabelForegroundColor": "White",
                "FilenameLabelBackgroundColor": "Blue",
                "PaneBorderColor": "Red",
                "TopSeparatorForegroundColor": "Black",
                "TopSeparatorBackgroundColor": "Gray",
                "DialogHelpForegroundColor": "BrightYellow",
                "DialogHelpBackgroundColor": "Blue",
                "ActiveTabForegroundColor": "White",
                "ActiveTabBackgroundColor": "Blue",
                "InactiveTabForegroundColor": "Gray",
                "InactiveTabBackgroundColor": "Black",
                "TabbarBackgroundColor": "Black",
                "OkColor": "Green",
                "WarningColor": "Yellow",
                "ErrorColor": "Red",
                "TextViewerForegroundColor": "White",
                "TextViewerBackgroundColor": "Black",
                "TextViewerStatusForegroundColor": "White",
                "TextViewerStatusBackgroundColor": "Gray",
                "TextViewerMessageForegroundColor": "White",
                "TextViewerMessageBackgroundColor": "Blue"
            },
            "KeyBindings": {
                "NormalMode": {},
                "SearchMode": {},
                "DialogMode": {},
                "ViewerMode": {}
            },
            "FileOperations": {
                "ConfirmDelete": true,
                "ConfirmOverwrite": true,
                "BufferSize": 8192,
                "PreserveTimestamps": true
            },
            "Search": {
                "CaseSensitive": false,
                "UseRegex": false,
                "UseMigemo": false,
                "MaxResults": 1000,
                "SearchDebounceMs": 150
            },
            "Ui": {
                "RefreshRate": 30,
                "ScrollOffset": 3,
                "TabWidth": 4
            },
            "WorkerPoolSize": 4,
            "LogLevel": "Information",
            "SessionPersistence": true,
            "KeyRepeatDelayMs": 300,
            "KeyRepeatRateMs": 30,
            "Ellipsis": "…",
            "MaxLogLinesInMemory": 2000,
            "LogSavePath": "logs/session.log",
            "SaveLogOnExit": true,
            "LogFileProgressThresholdMs": 5000,
            "Editor": null,
            "HelpLanguage": "en"
        }"#;

        fs::write(&config_path, config_json).unwrap();

        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let config = manager.load_config().unwrap();

        // Verify PaneInfo colors are loaded correctly
        assert_eq!(
            config.display.colors.pane_info_background_color,
            Some("Gray".to_string())
        );
        assert_eq!(
            config.display.colors.pane_info_foreground_color,
            Some("Black".to_string())
        );

        // Verify inactive file pane cursor colors are loaded correctly
        assert_eq!(
            config
                .display
                .colors
                .inactive_file_pane_cursor_background_color,
            Some("DarkGray".to_string())
        );
        assert_eq!(
            config
                .display
                .colors
                .inactive_file_pane_cursor_foreground_color,
            Some("Black".to_string())
        );

        // Verify other colors are also loaded
        assert_eq!(config.display.colors.foreground_color, "White");
        assert_eq!(config.display.colors.background_color, "Black");
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
        assert!(matches!(
            result.unwrap_err(),
            ConfigError::ValidationError(_)
        ));
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

    #[test]
    fn leap_config_defaults() {
        use crate::config::{JumpNavConfig, NoMatchFeedback};
        let cfg = JumpNavConfig::default();
        assert!(cfg.leap_enabled);
        assert!(cfg.leap_migemo_enabled);
        assert_eq!(cfg.leap_migemo_min_chars, 2);
        assert_eq!(cfg.leap_debounce_ms, 150);
        assert_eq!(cfg.no_match_feedback, NoMatchFeedback::TaskPanel);
    }

    #[test]
    fn app_state_loads_file_type_map_at_startup() {
        let state = AppState::new(AppConfig::default());
        assert!(!state.file_type_map.is_empty());
        assert!(state.file_type_map.iter().any(|m| m.extension == "mp4"));
    }
}
