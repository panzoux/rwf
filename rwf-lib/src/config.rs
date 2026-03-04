//! Configuration system for the two-pane file manager
//!
//! This module provides configuration structures and loading/saving functionality.
//! **Validates: Requirements 17.1, 17.6, 17.7, 17.8, 38.3, 38.5**

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Display configuration (colors, CJK width, etc.)
    pub display: DisplayConfig,
    /// Key bindings configuration
    pub key_bindings: KeyBindings,
    /// File operation settings
    pub file_operations: FileOpConfig,
    /// Search configuration
    pub search: SearchConfig,
    /// UI configuration
    pub ui: UIConfig,
    /// Number of worker threads in the pool
    pub worker_pool_size: usize,
    /// Log level for application logging
    pub log_level: crate::logging::LogLevel,
    /// Enable session state persistence
    pub session_persistence: bool,
    /// Key repeat delay in milliseconds (initial debounce)
    pub key_repeat_delay_ms: u32,
    /// Key repeat rate in milliseconds (after initial delay)
    pub key_repeat_rate_ms: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            display: DisplayConfig::default(),
            key_bindings: KeyBindings::default(),
            file_operations: FileOpConfig::default(),
            search: SearchConfig::default(),
            ui: UIConfig::default(),
            worker_pool_size: 4,
            log_level: crate::logging::LogLevel::Information,
            session_persistence: true,
            key_repeat_delay_ms: 300,
            key_repeat_rate_ms: 30,
        }
    }
}

/// Display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Show hidden files
    pub show_hidden: bool,
    /// Show system files
    pub show_system: bool,
    /// Date format string
    pub date_format: String,
    /// Time format (12 or 24 hour)
    pub time_format: TimeFormat,
    /// CJK character width (1 or 2)
    pub cjk_width: u8,
    /// Color scheme
    pub colors: ColorScheme,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_hidden: false,
            show_system: false,
            date_format: "%Y-%m-%d %H:%M".to_string(),
            time_format: TimeFormat::TwentyFourHour,
            cjk_width: 2,
            colors: ColorScheme::default(),
        }
    }
}

/// Time format options
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TimeFormat {
    TwentyFourHour,
    TwelveHour,
}

/// Color scheme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    // Main UI colors
    pub foreground_color: String,
    pub background_color: String,
    pub highlight_foreground_color: String,
    pub highlight_background_color: String,
    
    // File and directory colors
    pub marked_file_color: String,
    pub directory_color: String,
    pub directory_background_color: String,
    pub inactive_directory_color: String,
    pub inactive_directory_background_color: String,
    
    // Pane and border colors
    pub filename_label_foreground_color: String,
    pub filename_label_background_color: String,
    pub pane_border_color: String,
    
    // Top separator colors
    pub top_separator_foreground_color: String,
    pub top_separator_background_color: String,
    
    // Dialog colors
    pub dialog_help_foreground_color: String,
    pub dialog_help_background_color: String,
    
    // Tab colors
    pub active_tab_foreground_color: String,
    pub active_tab_background_color: String,
    pub inactive_tab_foreground_color: String,
    pub inactive_tab_background_color: String,
    pub tabbar_background_color: String,
    
    // Status colors
    pub ok_color: String,
    pub warning_color: String,
    pub error_color: String,
    
    // Text viewer colors
    pub text_viewer_foreground_color: String,
    pub text_viewer_background_color: String,
    pub text_viewer_status_foreground_color: String,
    pub text_viewer_status_background_color: String,
    pub text_viewer_message_foreground_color: String,
    pub text_viewer_message_background_color: String,
}

impl Default for ColorScheme {
    fn default() -> Self {
        // TWF-compatible defaults
        Self {
            foreground_color: "White".to_string(),
            background_color: "Black".to_string(),
            highlight_foreground_color: "Black".to_string(),
            highlight_background_color: "Cyan".to_string(),
            marked_file_color: "Cyan".to_string(),
            directory_color: "BrightCyan".to_string(),
            directory_background_color: "Black".to_string(),
            inactive_directory_color: "Cyan".to_string(),
            inactive_directory_background_color: "Black".to_string(),
            filename_label_foreground_color: "White".to_string(),
            filename_label_background_color: "Blue".to_string(),
            pane_border_color: "Red".to_string(),
            top_separator_foreground_color: "Black".to_string(),
            top_separator_background_color: "Gray".to_string(),
            dialog_help_foreground_color: "BrightYellow".to_string(),
            dialog_help_background_color: "Blue".to_string(),
            active_tab_foreground_color: "White".to_string(),
            active_tab_background_color: "Blue".to_string(),
            inactive_tab_foreground_color: "Gray".to_string(),
            inactive_tab_background_color: "Black".to_string(),
            tabbar_background_color: "Black".to_string(),
            ok_color: "Green".to_string(),
            warning_color: "Yellow".to_string(),
            error_color: "Red".to_string(),
            text_viewer_foreground_color: "White".to_string(),
            text_viewer_background_color: "Black".to_string(),
            text_viewer_status_foreground_color: "White".to_string(),
            text_viewer_status_background_color: "Gray".to_string(),
            text_viewer_message_foreground_color: "White".to_string(),
            text_viewer_message_background_color: "Blue".to_string(),
        }
    }
}

/// Key bindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    pub normal_mode: HashMap<String, Action>,
    pub search_mode: HashMap<String, Action>,
    pub dialog_mode: HashMap<String, Action>,
    pub viewer_mode: HashMap<String, Action>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        // TWF-compatible defaults
        let mut normal_mode = HashMap::new();
        normal_mode.insert("Tab".to_string(), Action::SwitchPane);
        normal_mode.insert("Up".to_string(), Action::CursorUp);
        normal_mode.insert("Down".to_string(), Action::CursorDown);
        normal_mode.insert("k".to_string(), Action::CursorUp);
        normal_mode.insert("j".to_string(), Action::CursorDown);
        normal_mode.insert("Home".to_string(), Action::Home);
        normal_mode.insert("End".to_string(), Action::End);
        normal_mode.insert("PageUp".to_string(), Action::PageUp);
        normal_mode.insert("PageDown".to_string(), Action::PageDown);
        normal_mode.insert("Enter".to_string(), Action::EnterDirectory);
        normal_mode.insert("Backspace".to_string(), Action::ParentDirectory);
        normal_mode.insert("Left".to_string(), Action::ParentDirectory);
        normal_mode.insert("Space".to_string(), Action::ToggleMark);
        normal_mode.insert("*".to_string(), Action::MarkAll);
        normal_mode.insert("Ctrl+U".to_string(), Action::UnmarkAll);
        normal_mode.insert("C".to_string(), Action::Copy);
        normal_mode.insert("M".to_string(), Action::Move);
        normal_mode.insert("D".to_string(), Action::Delete);
        normal_mode.insert("R".to_string(), Action::Rename);
        normal_mode.insert("Shift+K".to_string(), Action::CreateDirectory);
        normal_mode.insert("/".to_string(), Action::StartSearch);
        normal_mode.insert("Ctrl+F".to_string(), Action::StartSearch);
        normal_mode.insert("f".to_string(), Action::SetFilter);
        normal_mode.insert("s".to_string(), Action::SortMenu);
        normal_mode.insert("Alt+Left".to_string(), Action::HistoryBack);
        normal_mode.insert("Alt+Right".to_string(), Action::HistoryForward);
        normal_mode.insert("Shift+Z".to_string(), Action::ReloadConfig);
        normal_mode.insert("Q".to_string(), Action::Quit);
        normal_mode.insert("Escape".to_string(), Action::Quit);
        
        Self {
            normal_mode,
            search_mode: HashMap::new(),
            dialog_mode: HashMap::new(),
            viewer_mode: HashMap::new(),
        }
    }
}

/// Actions that can be bound to keys
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Action {
    // Navigation
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    PageUp,
    PageDown,
    Home,
    End,
    EnterDirectory,
    ParentDirectory,
    SwitchPane,
    HistoryBack,
    HistoryForward,
    
    // File Operations
    Copy,
    Move,
    Delete,
    Rename,
    CreateDirectory,
    
    // Marking
    ToggleMark,
    MarkAll,
    UnmarkAll,
    MarkPattern,
    MarkRange,
    InvertMarks,
    
    // Search
    StartSearch,
    NextMatch,
    PrevMatch,
    ClearSearch,
    
    // View
    ChangeDisplayMode(u8),
    SortMenu,
    ToggleHidden,
    Refresh,
    SetFilter,
    
    // Tabs
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    TabSelector,
    
    // Advanced
    CustomFunction,
    RegisteredFolders,
    JobManager,
    ViewFile,
    HexView,
    CompareFiles,
    
    // Application
    Quit,
    ReloadConfig,
}

/// File operation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOpConfig {
    /// Confirm before deleting files
    pub confirm_delete: bool,
    /// Confirm before overwriting files
    pub confirm_overwrite: bool,
    /// Buffer size for file operations (bytes)
    pub buffer_size: usize,
    /// Preserve file timestamps during copy/move
    pub preserve_timestamps: bool,
}

impl Default for FileOpConfig {
    fn default() -> Self {
        Self {
            confirm_delete: true,
            confirm_overwrite: true,
            buffer_size: 8192,
            preserve_timestamps: true,
        }
    }
}

/// Search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Case-sensitive search by default
    pub case_sensitive: bool,
    /// Use regex patterns by default
    pub use_regex: bool,
    /// Use migemo search by default
    pub use_migemo: bool,
    /// Maximum number of search results
    pub max_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            use_migemo: false,
            max_results: 1000,
        }
    }
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    /// UI refresh rate (Hz)
    pub refresh_rate: u64,
    /// Scroll offset (lines to keep visible above/below cursor)
    pub scroll_offset: usize,
    /// Tab width for display
    pub tab_width: usize,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            refresh_rate: 30,
            scroll_offset: 3,
            tab_width: 4,
        }
    }
}

/// Configuration manager for loading and saving configuration files
/// **Validates: Requirements 17.1, 17.2, 17.3, 17.9, 38.1, 38.9, 38.10**
pub struct ConfigManager {
    config_path: std::path::PathBuf,
    keybindings_path: std::path::PathBuf,
}

impl ConfigManager {
    /// Create a new ConfigManager with default paths
    pub fn new() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("two-pane-fm");
        
        Self {
            config_path: config_dir.join("config.json"),
            keybindings_path: config_dir.join("keybindings.json"),
        }
    }
    
    /// Create a ConfigManager with custom paths (for testing)
    pub fn with_paths(config_path: std::path::PathBuf, keybindings_path: std::path::PathBuf) -> Self {
        Self {
            config_path,
            keybindings_path,
        }
    }
    
    /// Load configuration from config.json
    /// Returns default configuration if file doesn't exist
    /// Returns error if file is malformed
    pub fn load_config(&self) -> Result<AppConfig, ConfigError> {
        if self.config_path.exists() {
            let content = std::fs::read_to_string(&self.config_path)
                .map_err(ConfigError::IoError)?;
            
            let config: AppConfig = serde_json::from_str(&content)
                .map_err(|e| ConfigError::ParseError(format!("Invalid config.json: {}", e)))?;
            
            // Validate configuration
            self.validate_config(&config)?;
            
            Ok(config)
        } else {
            // Use default settings if file doesn't exist
            Ok(AppConfig::default())
        }
    }
    
    /// Save configuration to config.json
    pub fn save_config(&self, config: &AppConfig) -> Result<(), ConfigError> {
        // Ensure directory exists
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(ConfigError::IoError)?;
        }
        
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| ConfigError::SerializeError(format!("Failed to serialize config: {}", e)))?;
        
        std::fs::write(&self.config_path, content)
            .map_err(ConfigError::IoError)?;
        
        Ok(())
    }
    
    /// Load key bindings from keybindings.json
    /// Returns default key bindings if file doesn't exist
    pub fn load_keybindings(&self) -> Result<KeyBindings, ConfigError> {
        if self.keybindings_path.exists() {
            let content = std::fs::read_to_string(&self.keybindings_path)
                .map_err(ConfigError::IoError)?;
            
            let keybindings: KeyBindings = serde_json::from_str(&content)
                .map_err(|e| ConfigError::ParseError(format!("Invalid keybindings.json: {}", e)))?;
            
            Ok(keybindings)
        } else {
            Ok(KeyBindings::default())
        }
    }
    
    /// Save key bindings to keybindings.json
    pub fn save_keybindings(&self, keybindings: &KeyBindings) -> Result<(), ConfigError> {
        // Ensure directory exists
        if let Some(parent) = self.keybindings_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(ConfigError::IoError)?;
        }
        
        let content = serde_json::to_string_pretty(keybindings)
            .map_err(|e| ConfigError::SerializeError(format!("Failed to serialize keybindings: {}", e)))?;
        
        std::fs::write(&self.keybindings_path, content)
            .map_err(ConfigError::IoError)?;
        
        Ok(())
    }
    
    /// Validate configuration settings
    pub fn validate_config(&self, config: &AppConfig) -> Result<(), ConfigError> {
        // Validate worker pool size
        if config.worker_pool_size == 0 {
            return Err(ConfigError::ValidationError(
                "worker_pool_size must be greater than 0".to_string()
            ));
        }
        
        if config.worker_pool_size > 32 {
            return Err(ConfigError::ValidationError(
                "worker_pool_size must not exceed 32".to_string()
            ));
        }
        
        // Validate CJK width
        if config.display.cjk_width != 1 && config.display.cjk_width != 2 {
            return Err(ConfigError::ValidationError(
                "cjk_width must be 1 or 2".to_string()
            ));
        }
        
        // Validate UI refresh rate
        if config.ui.refresh_rate == 0 {
            return Err(ConfigError::ValidationError(
                "refresh_rate must be greater than 0".to_string()
            ));
        }
        
        // Validate buffer size
        if config.file_operations.buffer_size == 0 {
            return Err(ConfigError::ValidationError(
                "buffer_size must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Serialization error: {0}")]
    SerializeError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_load_default_config_when_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let config = manager.load_config().unwrap();
        
        assert_eq!(config.worker_pool_size, 4);
        assert_eq!(config.session_persistence, true);
    }
    
    #[test]
    fn test_load_config_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        // Write a config file
        let config_json = r#"{
            "display": {
                "show_hidden": true,
                "show_system": false,
                "date_format": "%Y-%m-%d",
                "time_format": "TwentyFourHour",
                "cjk_width": 2,
                "colors": {
                    "foreground_color": "White",
                    "background_color": "Black",
                    "highlight_foreground_color": "Black",
                    "highlight_background_color": "Cyan",
                    "marked_file_color": "Cyan",
                    "directory_color": "BrightCyan",
                    "directory_background_color": "Black",
                    "inactive_directory_color": "Cyan",
                    "inactive_directory_background_color": "Black",
                    "filename_label_foreground_color": "White",
                    "filename_label_background_color": "Blue",
                    "pane_border_color": "Red",
                    "top_separator_foreground_color": "Black",
                    "top_separator_background_color": "Gray",
                    "dialog_help_foreground_color": "BrightYellow",
                    "dialog_help_background_color": "Blue",
                    "active_tab_foreground_color": "White",
                    "active_tab_background_color": "Blue",
                    "inactive_tab_foreground_color": "Gray",
                    "inactive_tab_background_color": "Black",
                    "tabbar_background_color": "Black",
                    "ok_color": "Green",
                    "warning_color": "Yellow",
                    "error_color": "Red",
                    "text_viewer_foreground_color": "White",
                    "text_viewer_background_color": "Black",
                    "text_viewer_status_foreground_color": "White",
                    "text_viewer_status_background_color": "Gray",
                    "text_viewer_message_foreground_color": "White",
                    "text_viewer_message_background_color": "Blue"
                }
            },
            "key_bindings": {
                "normal_mode": {},
                "search_mode": {},
                "dialog_mode": {},
                "viewer_mode": {}
            },
            "file_operations": {
                "confirm_delete": true,
                "confirm_overwrite": true,
                "buffer_size": 8192,
                "preserve_timestamps": true
            },
            "search": {
                "case_sensitive": false,
                "use_regex": false,
                "use_migemo": false,
                "max_results": 1000
            },
            "ui": {
                "refresh_rate": 30,
                "scroll_offset": 3,
                "tab_width": 4
            },
            "worker_pool_size": 8,
            "log_level": "Debug",
            "session_persistence": false,
            "key_repeat_delay_ms": 300,
            "key_repeat_rate_ms": 50
        }"#;
        
        std::fs::write(&config_path, config_json).unwrap();
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let config = manager.load_config().unwrap();
        
        assert_eq!(config.worker_pool_size, 8);
        assert_eq!(config.session_persistence, false);
        assert_eq!(config.display.show_hidden, true);
    }
    
    #[test]
    fn test_invalid_config_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        // Write invalid JSON
        std::fs::write(&config_path, "{ invalid json }").unwrap();
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.load_config();
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ParseError(_)));
    }
    
    #[test]
    fn test_validate_worker_pool_size() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.worker_pool_size = 0;
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.validate_config(&config);
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ValidationError(_)));
    }
    
    #[test]
    fn test_validate_cjk_width() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let mut config = AppConfig::default();
        config.display.cjk_width = 3;
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.validate_config(&config);
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path.clone(), keybindings_path);
        
        let mut config = AppConfig::default();
        config.worker_pool_size = 6;
        config.session_persistence = false;
        
        manager.save_config(&config).unwrap();
        
        let loaded_config = manager.load_config().unwrap();
        assert_eq!(loaded_config.worker_pool_size, 6);
        assert_eq!(loaded_config.session_persistence, false);
    }
    
    #[test]
    fn test_load_default_keybindings_when_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let keybindings = manager.load_keybindings().unwrap();
        
        assert!(keybindings.normal_mode.contains_key("Tab"));
        assert_eq!(keybindings.normal_mode.get("Tab"), Some(&Action::SwitchPane));
    }
}
