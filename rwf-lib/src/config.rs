//! Configuration system for the two-pane file manager
//!
//! This module provides configuration structures and loading/saving functionality.
//! **Validates: Requirements 17.1, 17.6, 17.7, 17.8, 38.3, 38.5**

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main application configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
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
    /// Ellipsis character for truncation (default: "…")
    #[serde(rename = "Ellipsis")]
    pub ellipsis: String,
    /// Maximum log lines to keep in memory (default: 2000)
    #[serde(rename = "MaxLogLinesInMemory")]
    pub max_log_lines_in_memory: usize,
    /// Path where session logs are saved (default: "logs/session.log")
    #[serde(rename = "LogSavePath")]
    pub log_save_path: String,
    /// Temporary flag for tab creation state
    #[serde(skip)]
    pub is_creating_tab: bool,
    /// Save log on exit (default: true)
    #[serde(rename = "SaveLogOnExit")]
    pub save_log_on_exit: bool,
    /// Threshold in milliseconds for logging slow file operations (default: 5000)
    #[serde(rename = "LogFileProgressThresholdMs")]
    pub log_file_progress_threshold_ms: u64,
    /// Editor command for launching configuration editor (default: system default editor)
    #[serde(rename = "EditorCommand")]
    pub editor_command: Option<String>,
    /// Help language (default: "en")
    #[serde(rename = "HelpLanguage")]
    pub help_language: String,

    /// Archive configuration
    #[serde(default)]
    pub archive: ArchiveConfig,

    /// Job manager configuration
    #[serde(default)]
    pub job_manager: JobManagerConfig,

    /// Text input configuration
    #[serde(default)]
    pub text_input: TextInputConfig,

    /// Jump navigation configuration (Jump to File / Jump to Directory dialogs)
    #[serde(default)]
    pub jump_nav: JumpNavConfig,

    /// Background polling interval in milliseconds for detecting external filesystem changes.
    /// Used by Layer 2 of the pane update mechanism (Phase 7). 0 = disabled.
    #[serde(default = "default_polling_interval_ms")]
    pub polling_interval_ms: u32,
}

fn default_polling_interval_ms() -> u32 { 1000 }

/// Configuration for Jump to File / Jump to Directory dialogs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JumpNavConfig {
    /// Maximum number of file+dir candidates collected for Jump to File (default: 1000)
    #[serde(default = "default_jump_file_max_results")]
    pub jump_file_max_results: usize,
    /// Recursive search depth for Jump to File (default: 4)
    #[serde(default = "default_jump_file_max_depth")]
    pub jump_file_max_depth: usize,
    /// Maximum number of directory candidates collected for Jump to Directory (default: 500)
    #[serde(default = "default_jump_path_max_results")]
    pub jump_path_max_results: usize,
    /// Recursive search depth for Jump to Directory (default: 3)
    #[serde(default = "default_jump_path_max_depth")]
    pub jump_path_max_depth: usize,
}

fn default_jump_file_max_results() -> usize { 1000 }
fn default_jump_file_max_depth()   -> usize { 4 }
fn default_jump_path_max_results() -> usize { 500 }
fn default_jump_path_max_depth()   -> usize { 3 }

impl Default for JumpNavConfig {
    fn default() -> Self {
        Self {
            jump_file_max_results: default_jump_file_max_results(),
            jump_file_max_depth:   default_jump_file_max_depth(),
            jump_path_max_results: default_jump_path_max_results(),
            jump_path_max_depth:   default_jump_path_max_depth(),
        }
    }
}

/// Archive configuration for compression operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveConfig {
    /// Default archive format (default: ZIP)
    #[serde(default = "default_archive_format")]
    pub default_format: crate::input::ArchiveFormat,
    
    /// Compression level (1-9, default: 6)
    #[serde(default = "default_compression_level")]
    pub compression_level: u32,
    
    /// Last used archive name for quick access
    #[serde(default)]
    pub last_archive_name: String,
}

fn default_archive_format() -> crate::input::ArchiveFormat {
    crate::input::ArchiveFormat::ZIP
}

fn default_compression_level() -> u32 {
    6
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            default_format: crate::input::ArchiveFormat::ZIP,
            compression_level: 6,
            last_archive_name: String::new(),
        }
    }
}

/// Job manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JobManagerConfig {
    /// Maximum number of simultaneous jobs (default: 4)
    #[serde(default = "default_max_simultaneous_jobs")]
    pub max_simultaneous_jobs: usize,

    /// Update interval for job progress in milliseconds (default: 300, min: 100)
    #[serde(default = "default_update_interval_ms")]
    pub update_interval_ms: u64,

    /// Default task panel height in lines (default: 10)
    #[serde(default = "default_task_panel_height")]
    pub task_panel_height: usize,

    /// Refresh interval for task panel in milliseconds (default: 500)
    #[serde(default = "default_task_panel_refresh_interval_ms")]
    pub task_panel_refresh_interval_ms: u64,

    /// Maximum log lines to keep in memory for task panel (default: 1000)
    #[serde(default = "default_max_task_panel_log_lines")]
    pub max_task_panel_log_lines: usize,

    /// Maximum number of log files to keep (default: 5)
    #[serde(default = "default_max_log_files")]
    pub max_log_files: usize,

    /// Refresh interval for job manager dialog in milliseconds (default: 500)
    #[serde(default = "default_job_manager_refresh_interval_ms")]
    pub job_manager_refresh_interval_ms: u64,

    /// How long to keep completed/cancelled jobs in seconds (default: 5)
    #[serde(default = "default_job_retention_period_secs")]
    pub job_retention_period_secs: u64,
}

fn default_max_simultaneous_jobs() -> usize { 4 }
fn default_update_interval_ms() -> u64 { 300 }
fn default_task_panel_height() -> usize { 10 }
fn default_task_panel_refresh_interval_ms() -> u64 { 500 }
fn default_max_task_panel_log_lines() -> usize { 1000 }
fn default_max_log_files() -> usize { 5 }
fn default_job_manager_refresh_interval_ms() -> u64 { 500 }
fn default_job_retention_period_secs() -> u64 { 5 }

impl Default for JobManagerConfig {
    fn default() -> Self {
        Self {
            max_simultaneous_jobs: default_max_simultaneous_jobs(),
            update_interval_ms: default_update_interval_ms(),
            task_panel_height: default_task_panel_height(),
            task_panel_refresh_interval_ms: default_task_panel_refresh_interval_ms(),
            max_task_panel_log_lines: default_max_task_panel_log_lines(),
            max_log_files: default_max_log_files(),
            job_manager_refresh_interval_ms: default_job_manager_refresh_interval_ms(),
            job_retention_period_secs: default_job_retention_period_secs(),
        }
    }
}

/// Text input edit mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditMode {
    Emacs,
    Vi,
}

/// Vi sub-mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViMode {
    Normal,
    Insert,
}

fn default_edit_mode() -> EditMode {
    EditMode::Emacs
}

/// Text input configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextInputConfig {
    /// Edit mode (default: Emacs)
    #[serde(default = "default_edit_mode")]
    pub edit_mode: EditMode,
}

impl Default for TextInputConfig {
    fn default() -> Self {
        Self {
            edit_mode: EditMode::Emacs,
        }
    }
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
            key_repeat_rate_ms: 15,
            ellipsis: "…".to_string(),  // Unicode ellipsis U+2026
            max_log_lines_in_memory: 2000,
            log_save_path: "logs/session.log".to_string(),
            save_log_on_exit: true,
            log_file_progress_threshold_ms: 5000,
            editor_command: None,  // Use system default editor
            help_language: "en".to_string(),
            archive: ArchiveConfig::default(),
            job_manager: JobManagerConfig::default(),
            text_input: TextInputConfig::default(),
            jump_nav: JumpNavConfig::default(),
            polling_interval_ms: default_polling_interval_ms(),
            is_creating_tab: false,
        }
    }
}

/// Display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DisplayConfig {
    /// Show hidden files
    #[serde(rename = "ShowHiddenFiles")]
    pub show_hidden: bool,
    /// Show system files
    pub show_system: bool,
    /// Date format string
    pub date_format: String,
    /// Time format (12 or 24 hour)
    pub time_format: TimeFormat,
    /// CJK character width (1 or 2)
    pub cjk_width: u8,
    /// Color scheme (flattened into Display for TWF compatibility)
    #[serde(flatten)]
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
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct ColorScheme {
    // Main UI colors
    pub foreground_color: String,
    pub background_color: String,
    pub highlight_foreground_color: String,
    pub highlight_background_color: String,
    
    // Active file pane cursor colors (UI area 4)
    pub file_pane_cursor_foreground_color: Option<String>,
    pub file_pane_cursor_background_color: Option<String>,
    
    // Inactive file pane cursor colors (UI area 4)
    pub inactive_file_pane_cursor_foreground_color: Option<String>,
    pub inactive_file_pane_cursor_background_color: Option<String>,
    
    // Inactive pane colors (UI area 4)
    pub inactive_foreground_color: Option<String>,
    pub inactive_background_color: Option<String>,
    
    // File and directory colors
    pub marked_file_color: String,
    pub directory_color: String,
    pub directory_background_color: String,
    pub inactive_directory_color: String,
    pub inactive_directory_background_color: String,
    
    // Pane info bar colors (UI area 5)
    pub pane_info_foreground_color: Option<String>,
    pub pane_info_background_color: Option<String>,
    
    // Pane and border colors
    pub filename_label_foreground_color: String,
    pub filename_label_background_color: String,
    pub pane_border_color: String,
    
    // Top separator colors (UI area 3)
    pub top_separator_foreground_color: String,
    pub top_separator_background_color: String,
    
    // Dialog colors
    pub dialog_help_foreground_color: String,
    pub dialog_help_background_color: String,
    
    // Tab colors (UI area 1)
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
            file_pane_cursor_foreground_color: None, // Falls back to highlight_foreground_color
            file_pane_cursor_background_color: None, // Falls back to highlight_background_color
            inactive_file_pane_cursor_foreground_color: Some("Black".to_string()),
            inactive_file_pane_cursor_background_color: Some("DarkGray".to_string()),
            inactive_foreground_color: Some("Gray".to_string()),
            inactive_background_color: Some("Black".to_string()),
            marked_file_color: "Cyan".to_string(),
            directory_color: "BrightCyan".to_string(),
            directory_background_color: "Black".to_string(),
            inactive_directory_color: "Cyan".to_string(),
            inactive_directory_background_color: "Black".to_string(),
            pane_info_foreground_color: Some("Black".to_string()),
            pane_info_background_color: Some("DarkGray".to_string()),
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

impl ColorScheme {
    /// Get file pane cursor foreground color with backward compatibility
    /// Falls back to highlight_foreground_color if not set
    /// **Validates: Requirements 49.9**
    pub fn get_file_pane_cursor_foreground(&self) -> &str {
        self.file_pane_cursor_foreground_color
            .as_deref()
            .unwrap_or(&self.highlight_foreground_color)
    }
    
    /// Get file pane cursor background color with backward compatibility
    /// Falls back to highlight_background_color if not set
    /// **Validates: Requirements 49.10**
    pub fn get_file_pane_cursor_background(&self) -> &str {
        self.file_pane_cursor_background_color
            .as_deref()
            .unwrap_or(&self.highlight_background_color)
    }
    
    /// Get inactive file pane cursor foreground color with backward compatibility
    /// Falls back to inactive_foreground_color, then foreground_color
    /// **Validates: Requirements 49.9**
    pub fn get_inactive_file_pane_cursor_foreground(&self) -> &str {
        self.inactive_file_pane_cursor_foreground_color
            .as_deref()
            .or(self.inactive_foreground_color.as_deref())
            .unwrap_or(&self.foreground_color)
    }
    
    /// Get inactive file pane cursor background color with backward compatibility
    /// Falls back to inactive_background_color, then background_color
    /// **Validates: Requirements 49.10**
    pub fn get_inactive_file_pane_cursor_background(&self) -> &str {
        self.inactive_file_pane_cursor_background_color
            .as_deref()
            .or(self.inactive_background_color.as_deref())
            .unwrap_or(&self.background_color)
    }
    
    /// Get inactive foreground color with backward compatibility
    /// Falls back to foreground_color if not set
    pub fn get_inactive_foreground(&self) -> &str {
        self.inactive_foreground_color
            .as_deref()
            .unwrap_or(&self.foreground_color)
    }
    
    /// Get inactive background color with backward compatibility
    /// Falls back to background_color if not set
    pub fn get_inactive_background(&self) -> &str {
        self.inactive_background_color
            .as_deref()
            .unwrap_or(&self.background_color)
    }
    
    /// Get pane info foreground color with backward compatibility
    /// Falls back to top_separator_foreground_color if not set
    /// **Validates: Requirements 49.9**
    pub fn get_pane_info_foreground(&self) -> &str {
        self.pane_info_foreground_color
            .as_deref()
            .unwrap_or(&self.top_separator_foreground_color)
    }
    
    /// Get pane info background color with backward compatibility
    /// Falls back to top_separator_background_color if not set
    /// **Validates: Requirements 49.10**
    pub fn get_pane_info_background(&self) -> &str {
        self.pane_info_background_color
            .as_deref()
            .unwrap_or(&self.top_separator_background_color)
    }
}

/// Key bindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyBindings {
    #[serde(rename = "NormalMode")]
    pub normal_mode: HashMap<String, Action>,
    #[serde(rename = "SearchMode")]
    pub search_mode: HashMap<String, Action>,
    #[serde(rename = "DialogMode")]
    pub dialog_mode: HashMap<String, Action>,
    #[serde(rename = "ViewerMode")]
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
        normal_mode.insert("Y".to_string(), Action::LaunchConfigurationProgram);
        normal_mode.insert("Ctrl+L".to_string(), Action::SaveLog);
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
    LaunchConfigurationProgram,
    SaveLog,
}

/// File operation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
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
#[serde(rename_all = "PascalCase")]
pub struct SearchConfig {
    /// Case-sensitive search by default
    pub case_sensitive: bool,
    /// Use regex patterns by default
    pub use_regex: bool,
    /// Use migemo search by default
    pub use_migemo: bool,
    /// Path to migemo dictionary file
    #[serde(rename = "DictPath")]
    pub dict_path: Option<String>,
    /// Search debounce interval in milliseconds
    #[serde(rename = "SearchDebounceMs")]
    pub search_debounce_ms: u64,
    /// Pattern rename preview debounce in milliseconds (0 = update every keystroke)
    #[serde(rename = "PatternRenameDebounceMs", default = "default_pattern_rename_debounce_ms")]
    pub pattern_rename_debounce_ms: u64,
    /// Maximum number of search results
    pub max_results: usize,
}

fn default_pattern_rename_debounce_ms() -> u64 { 150 }

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            use_migemo: false,
            dict_path: None,
            search_debounce_ms: 150,
            pattern_rename_debounce_ms: 150,
            max_results: 1000,
        }
    }
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
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

/// File type association: maps an extension to an external command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExtensionAssociation {
    /// File extension without leading dot, e.g. "pdf", "txt" (case-insensitive match)
    pub extension: String,
    /// Command template. Supports macros: $P (dir), $F (filename), $W (stem), $E (ext)
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
}

/// Configuration manager for loading and saving configuration files
/// **Validates: Requirements 17.1, 17.2, 17.3, 17.9, 38.1, 38.9, 38.10**
pub struct ConfigManager {
    config_path: std::path::PathBuf,
    keybindings_path: std::path::PathBuf,
    extension_associations_path: std::path::PathBuf,
    custom_functions_path: std::path::PathBuf,
    context_menu_path: std::path::PathBuf,
}

impl ConfigManager {
    /// Create a new ConfigManager with default paths
    pub fn new() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("rwf");

        Self {
            config_path: config_dir.join("config.json"),
            keybindings_path: config_dir.join("keybindings.json"),
            extension_associations_path: config_dir.join("extension_associations.json"),
            custom_functions_path: config_dir.join("custom_functions.json"),
            context_menu_path: config_dir.join("context_menu.json"),
        }
    }

    /// Create a ConfigManager with custom paths (for testing)
    pub fn with_paths(config_path: std::path::PathBuf, keybindings_path: std::path::PathBuf) -> Self {
        let config_dir = config_path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        Self {
            extension_associations_path: config_dir.join("extension_associations.json"),
            custom_functions_path: config_dir.join("custom_functions.json"),
            context_menu_path: config_dir.join("context_menu.json"),
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
    
    /// Get the config file path
    pub fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }

    /// Get the keybindings file path
    pub fn keybindings_path(&self) -> &std::path::Path {
        &self.keybindings_path
    }

    /// Get the extension_associations.json path
    pub fn extension_associations_path(&self) -> &std::path::Path {
        &self.extension_associations_path
    }

    /// Get the custom_functions.json path
    pub fn custom_functions_path(&self) -> &std::path::Path {
        &self.custom_functions_path
    }

    /// Get the context_menu.json path
    pub fn context_menu_path(&self) -> &std::path::Path {
        &self.context_menu_path
    }

    /// Load extension associations from extension_associations.json.
    /// Returns an empty list if the file does not exist.
    pub fn load_extension_associations(&self) -> Vec<ExtensionAssociation> {
        if !self.extension_associations_path.exists() {
            return Vec::new();
        }
        match std::fs::read_to_string(&self.extension_associations_path) {
            Ok(content) => serde_json::from_str::<Vec<ExtensionAssociation>>(&content)
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to parse extension_associations.json: {}", e);
                    Vec::new()
                }),
            Err(e) => {
                tracing::warn!("Failed to read extension_associations.json: {}", e);
                Vec::new()
            }
        }
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
            "Display": {
                "ShowHiddenFiles": true,
                "ShowSystem": false,
                "DateFormat": "%Y-%m-%d",
                "TimeFormat": "TwentyFourHour",
                "CjkWidth": 2,
                "Colors": {
                    "ForegroundColor": "White",
                    "BackgroundColor": "Black",
                    "HighlightForegroundColor": "Black",
                    "HighlightBackgroundColor": "Cyan",
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
                }
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
            "WorkerPoolSize": 8,
            "LogLevel": "Debug",
            "SessionPersistence": false,
            "KeyRepeatDelayMs": 300,
            "KeyRepeatRateMs": 50,
            "Ellipsis": "…",
            "MaxLogLinesInMemory": 2000,
            "LogSavePath": "logs/session.log",
            "SaveLogOnExit": true,
            "LogFileProgressThresholdMs": 5000,
            "EditorCommand": null,
            "HelpLanguage": "en"
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
