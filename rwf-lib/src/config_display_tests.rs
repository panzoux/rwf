//! Tests for display configuration
//! **Validates: Requirements 17.6, 17.7, 32.3, 32.4, 32.5, 39.8**

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, DisplayConfig, ColorScheme, TimeFormat};
    
    #[test]
    fn test_default_display_config() {
        let config = DisplayConfig::default();
        
        assert_eq!(config.show_hidden, false);
        assert_eq!(config.show_system, false);
        assert_eq!(config.date_format, "%Y-%m-%d %H:%M");
        assert_eq!(config.cjk_width, 2);
        assert!(matches!(config.time_format, TimeFormat::TwentyFourHour));
    }
    
    #[test]
    fn test_default_color_scheme() {
        let colors = ColorScheme::default();
        
        // Verify TWF-compatible color defaults
        assert_eq!(colors.foreground_color, "White");
        assert_eq!(colors.background_color, "Black");
        assert_eq!(colors.highlight_foreground_color, "Black");
        assert_eq!(colors.highlight_background_color, "Cyan");
        assert_eq!(colors.marked_file_color, "Cyan");
        assert_eq!(colors.directory_color, "BrightCyan");
    }
    
    #[test]
    fn test_cjk_width_configuration() {
        let mut config = DisplayConfig::default();
        
        // Test valid CJK widths
        config.cjk_width = 1;
        assert_eq!(config.cjk_width, 1);
        
        config.cjk_width = 2;
        assert_eq!(config.cjk_width, 2);
    }
    
    #[test]
    fn test_custom_color_scheme() {
        let mut colors = ColorScheme::default();
        
        colors.foreground_color = "Green".to_string();
        colors.background_color = "Blue".to_string();
        colors.directory_color = "Yellow".to_string();
        
        assert_eq!(colors.foreground_color, "Green");
        assert_eq!(colors.background_color, "Blue");
        assert_eq!(colors.directory_color, "Yellow");
    }
    
    #[test]
    fn test_time_format_options() {
        let mut config = DisplayConfig::default();
        
        config.time_format = TimeFormat::TwelveHour;
        assert!(matches!(config.time_format, TimeFormat::TwelveHour));
        
        config.time_format = TimeFormat::TwentyFourHour;
        assert!(matches!(config.time_format, TimeFormat::TwentyFourHour));
    }
    
    #[test]
    fn test_date_format_customization() {
        let mut config = DisplayConfig::default();
        
        config.date_format = "%d/%m/%Y".to_string();
        assert_eq!(config.date_format, "%d/%m/%Y");
        
        config.date_format = "%Y-%m-%d %H:%M:%S".to_string();
        assert_eq!(config.date_format, "%Y-%m-%d %H:%M:%S");
    }
    
    #[test]
    fn test_show_hidden_files() {
        let mut config = DisplayConfig::default();
        
        assert_eq!(config.show_hidden, false);
        
        config.show_hidden = true;
        assert_eq!(config.show_hidden, true);
    }
    
    #[test]
    fn test_show_system_files() {
        let mut config = DisplayConfig::default();
        
        assert_eq!(config.show_system, false);
        
        config.show_system = true;
        assert_eq!(config.show_system, true);
    }
    
    #[test]
    fn test_all_color_fields_present() {
        let colors = ColorScheme::default();
        
        // Verify all required color fields are present
        assert!(!colors.foreground_color.is_empty());
        assert!(!colors.background_color.is_empty());
        assert!(!colors.highlight_foreground_color.is_empty());
        assert!(!colors.highlight_background_color.is_empty());
        assert!(!colors.marked_file_color.is_empty());
        assert!(!colors.directory_color.is_empty());
        assert!(!colors.directory_background_color.is_empty());
        assert!(!colors.inactive_directory_color.is_empty());
        assert!(!colors.inactive_directory_background_color.is_empty());
        assert!(!colors.filename_label_foreground_color.is_empty());
        assert!(!colors.filename_label_background_color.is_empty());
        assert!(!colors.pane_border_color.is_empty());
        assert!(!colors.top_separator_foreground_color.is_empty());
        assert!(!colors.top_separator_background_color.is_empty());
        assert!(!colors.dialog_help_foreground_color.is_empty());
        assert!(!colors.dialog_help_background_color.is_empty());
        assert!(!colors.active_tab_foreground_color.is_empty());
        assert!(!colors.active_tab_background_color.is_empty());
        assert!(!colors.inactive_tab_foreground_color.is_empty());
        assert!(!colors.inactive_tab_background_color.is_empty());
        assert!(!colors.tabbar_background_color.is_empty());
        assert!(!colors.ok_color.is_empty());
        assert!(!colors.warning_color.is_empty());
        assert!(!colors.error_color.is_empty());
        assert!(!colors.text_viewer_foreground_color.is_empty());
        assert!(!colors.text_viewer_background_color.is_empty());
        assert!(!colors.text_viewer_status_foreground_color.is_empty());
        assert!(!colors.text_viewer_status_background_color.is_empty());
        assert!(!colors.text_viewer_message_foreground_color.is_empty());
        assert!(!colors.text_viewer_message_background_color.is_empty());
    }
    
    #[test]
    fn test_display_config_in_app_config() {
        let config = AppConfig::default();
        
        assert_eq!(config.display.show_hidden, false);
        assert_eq!(config.display.cjk_width, 2);
        assert_eq!(config.display.colors.foreground_color, "White");
    }
    
    #[test]
    fn test_tab_colors() {
        let colors = ColorScheme::default();
        
        assert_eq!(colors.active_tab_foreground_color, "White");
        assert_eq!(colors.active_tab_background_color, "Blue");
        assert_eq!(colors.inactive_tab_foreground_color, "Gray");
        assert_eq!(colors.inactive_tab_background_color, "Black");
        assert_eq!(colors.tabbar_background_color, "Black");
    }
    
    #[test]
    fn test_status_colors() {
        let colors = ColorScheme::default();
        
        assert_eq!(colors.ok_color, "Green");
        assert_eq!(colors.warning_color, "Yellow");
        assert_eq!(colors.error_color, "Red");
    }
    
    #[test]
    fn test_text_viewer_colors() {
        let colors = ColorScheme::default();
        
        assert_eq!(colors.text_viewer_foreground_color, "White");
        assert_eq!(colors.text_viewer_background_color, "Black");
        assert_eq!(colors.text_viewer_status_foreground_color, "White");
        assert_eq!(colors.text_viewer_status_background_color, "Gray");
        assert_eq!(colors.text_viewer_message_foreground_color, "White");
        assert_eq!(colors.text_viewer_message_background_color, "Blue");
    }
    
    #[test]
    fn test_twf_format_flattened_colors() {
        // Test that TWF format with colors directly under Display works
        let json = r#"{
            "ShowHiddenFiles": false,
            "ShowSystem": false,
            "DateFormat": "%Y-%m-%d %H:%M",
            "TimeFormat": "TwentyFourHour",
            "CjkWidth": 2,
            "ForegroundColor": "White",
            "BackgroundColor": "Black",
            "HighlightForegroundColor": "Black",
            "HighlightBackgroundColor": "Cyan",
            "MarkedFileColor": "Cyan",
            "DirectoryColor": "BrightCyan",
            "DirectoryBackgroundColor": "Black",
            "InactiveDirectoryColor": "Cyan",
            "InactiveDirectoryBackgroundColor": "Black",
            "PaneInfoForegroundColor": "White",
            "PaneInfoBackgroundColor": "Gray",
            "FilenameLabelForegroundColor": "White",
            "FilenameLabelBackgroundColor": "Blue",
            "PaneBorderColor": "Gray",
            "TopSeparatorForegroundColor": "Gray",
            "TopSeparatorBackgroundColor": "Black",
            "DialogHelpForegroundColor": "White",
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
        }"#;
        
        let config: serde_json::Result<DisplayConfig> = serde_json::from_str(json);
        assert!(config.is_ok(), "Failed to deserialize TWF format DisplayConfig: {:?}", config.err());
        
        let config = config.unwrap();
        assert_eq!(config.colors.foreground_color, "White");
        assert_eq!(config.colors.background_color, "Black");
        assert_eq!(config.colors.pane_info_background_color, Some("Gray".to_string()));
        assert_eq!(config.show_hidden, false);
        assert_eq!(config.cjk_width, 2);
    }
    
    // ========================================================================
    // Integration Tests for Color Configuration (Task 59.11)
    // **Validates: Requirements 49.1-49.10**
    // ========================================================================
    
    #[test]
    fn test_tab_bar_colors_requirement_49_1() {
        // **Validates: Requirements 49.1**
        // Tab bar colors (UI area 1)
        let colors = ColorScheme::default();
        
        // Verify all tab bar colors are configured
        assert!(!colors.active_tab_foreground_color.is_empty());
        assert!(!colors.active_tab_background_color.is_empty());
        assert!(!colors.inactive_tab_foreground_color.is_empty());
        assert!(!colors.inactive_tab_background_color.is_empty());
        assert!(!colors.tabbar_background_color.is_empty());
        
        // Verify default values
        assert_eq!(colors.active_tab_foreground_color, "White");
        assert_eq!(colors.active_tab_background_color, "Blue");
        assert_eq!(colors.inactive_tab_foreground_color, "Gray");
        assert_eq!(colors.inactive_tab_background_color, "Black");
        assert_eq!(colors.tabbar_background_color, "Black");
    }
    
    #[test]
    fn test_path_display_colors_requirement_49_2() {
        // **Validates: Requirements 49.2**
        // Path display colors (UI area 2)
        let colors = ColorScheme::default();
        
        // Verify path display uses foreground and background colors
        assert!(!colors.foreground_color.is_empty());
        assert!(!colors.background_color.is_empty());
        
        assert_eq!(colors.foreground_color, "White");
        assert_eq!(colors.background_color, "Black");
    }
    
    #[test]
    fn test_top_separator_colors_requirement_49_3() {
        // **Validates: Requirements 49.3**
        // Top separator colors (UI area 3)
        let colors = ColorScheme::default();
        
        // Verify top separator colors are configured
        assert!(!colors.top_separator_foreground_color.is_empty());
        assert!(!colors.top_separator_background_color.is_empty());
        
        assert_eq!(colors.top_separator_foreground_color, "Black");
        assert_eq!(colors.top_separator_background_color, "Gray");
    }
    
    #[test]
    fn test_active_file_pane_colors_requirement_49_4() {
        // **Validates: Requirements 49.4**
        // Active file pane colors (UI area 4)
        let colors = ColorScheme::default();
        
        // Verify all active file pane colors are configured
        assert!(!colors.foreground_color.is_empty());
        assert!(!colors.background_color.is_empty());
        assert!(!colors.marked_file_color.is_empty());
        assert!(!colors.directory_color.is_empty());
        assert!(!colors.directory_background_color.is_empty());
        
        // Verify cursor colors (with backward compatibility)
        let cursor_fg = colors.get_file_pane_cursor_foreground();
        let cursor_bg = colors.get_file_pane_cursor_background();
        assert!(!cursor_fg.is_empty());
        assert!(!cursor_bg.is_empty());
        
        // Default values
        assert_eq!(colors.foreground_color, "White");
        assert_eq!(colors.background_color, "Black");
        assert_eq!(colors.marked_file_color, "Cyan");
        assert_eq!(colors.directory_color, "BrightCyan");
        assert_eq!(colors.directory_background_color, "Black");
    }
    
    #[test]
    fn test_inactive_file_pane_colors_requirement_49_5() {
        // **Validates: Requirements 49.5**
        // Inactive file pane colors (UI area 4)
        let colors = ColorScheme::default();
        
        // Verify inactive pane colors (with backward compatibility)
        let inactive_fg = colors.get_inactive_foreground();
        let inactive_bg = colors.get_inactive_background();
        assert!(!inactive_fg.is_empty());
        assert!(!inactive_bg.is_empty());
        
        // Verify inactive directory colors
        assert!(!colors.inactive_directory_color.is_empty());
        assert!(!colors.inactive_directory_background_color.is_empty());
        
        // Verify inactive cursor colors (with backward compatibility)
        let inactive_cursor_fg = colors.get_inactive_file_pane_cursor_foreground();
        let inactive_cursor_bg = colors.get_inactive_file_pane_cursor_background();
        assert!(!inactive_cursor_fg.is_empty());
        assert!(!inactive_cursor_bg.is_empty());
        
        // Default values
        assert_eq!(colors.inactive_directory_color, "Cyan");
        assert_eq!(colors.inactive_directory_background_color, "Black");
    }
    
    #[test]
    fn test_pane_info_bar_colors_requirement_49_6() {
        // **Validates: Requirements 49.6**
        // Pane info bar colors (UI area 5)
        let colors = ColorScheme::default();
        
        // Verify pane info bar colors are configured
        assert!(colors.pane_info_foreground_color.is_some());
        assert!(colors.pane_info_background_color.is_some());
        
        // Default values
        assert_eq!(colors.pane_info_foreground_color.as_deref(), Some("Black"));
        assert_eq!(colors.pane_info_background_color.as_deref(), Some("DarkGray"));
    }
    
    #[test]
    fn test_filename_label_colors_requirement_49_7() {
        // **Validates: Requirements 49.7**
        // Filename label colors (UI area 6)
        let colors = ColorScheme::default();
        
        // Verify filename label colors are configured
        assert!(!colors.filename_label_foreground_color.is_empty());
        assert!(!colors.filename_label_background_color.is_empty());
        
        // Default values
        assert_eq!(colors.filename_label_foreground_color, "White");
        assert_eq!(colors.filename_label_background_color, "Blue");
    }
    
    #[test]
    fn test_task_view_colors_requirement_49_8() {
        // **Validates: Requirements 49.8**
        // Task view colors (UI area 7)
        let colors = ColorScheme::default();
        
        // Task view uses foreground and background colors
        assert!(!colors.foreground_color.is_empty());
        assert!(!colors.background_color.is_empty());
        
        assert_eq!(colors.foreground_color, "White");
        assert_eq!(colors.background_color, "Black");
    }
    
    #[test]
    fn test_backward_compatibility_highlight_foreground_requirement_49_9() {
        // **Validates: Requirements 49.9**
        // HighlightForegroundColor as alias for FilePaneCursorForegroundColor
        
        // Test 1: When new property is not set, fall back to old property
        let colors = ColorScheme {
            file_pane_cursor_foreground_color: None,
            highlight_foreground_color: "Yellow".to_string(),
            ..ColorScheme::default()
        };
        
        assert_eq!(colors.get_file_pane_cursor_foreground(), "Yellow");
        
        // Test 2: When new property is set, use it
        let colors = ColorScheme {
            file_pane_cursor_foreground_color: Some("Red".to_string()),
            highlight_foreground_color: "Yellow".to_string(),
            ..ColorScheme::default()
        };
        
        assert_eq!(colors.get_file_pane_cursor_foreground(), "Red");
    }
    
    #[test]
    fn test_backward_compatibility_highlight_background_requirement_49_10() {
        // **Validates: Requirements 49.10**
        // HighlightBackgroundColor as alias for FilePaneCursorBackgroundColor
        
        // Test 1: When new property is not set, fall back to old property
        let colors = ColorScheme {
            file_pane_cursor_background_color: None,
            highlight_background_color: "Magenta".to_string(),
            ..ColorScheme::default()
        };
        
        assert_eq!(colors.get_file_pane_cursor_background(), "Magenta");
        
        // Test 2: When new property is set, use it
        let colors = ColorScheme {
            file_pane_cursor_background_color: Some("Green".to_string()),
            highlight_background_color: "Magenta".to_string(),
            ..ColorScheme::default()
        };
        
        assert_eq!(colors.get_file_pane_cursor_background(), "Green");
    }
    
    #[test]
    fn test_inactive_cursor_backward_compatibility() {
        // Test inactive cursor color fallback chain
        
        // Test 1: All properties set - use specific property
        let colors = ColorScheme {
            inactive_file_pane_cursor_foreground_color: Some("Red".to_string()),
            inactive_foreground_color: Some("Yellow".to_string()),
            foreground_color: "White".to_string(),
            ..ColorScheme::default()
        };
        assert_eq!(colors.get_inactive_file_pane_cursor_foreground(), "Red");
        
        // Test 2: Specific property not set - fall back to inactive_foreground_color
        let colors = ColorScheme {
            inactive_file_pane_cursor_foreground_color: None,
            inactive_foreground_color: Some("Yellow".to_string()),
            foreground_color: "White".to_string(),
            ..ColorScheme::default()
        };
        assert_eq!(colors.get_inactive_file_pane_cursor_foreground(), "Yellow");
        
        // Test 3: Both not set - fall back to foreground_color
        let colors = ColorScheme {
            inactive_file_pane_cursor_foreground_color: None,
            inactive_foreground_color: None,
            foreground_color: "White".to_string(),
            ..ColorScheme::default()
        };
        assert_eq!(colors.get_inactive_file_pane_cursor_foreground(), "White");
    }
    
    #[test]
    fn test_inactive_cursor_background_backward_compatibility() {
        // Test inactive cursor background color fallback chain
        
        // Test 1: All properties set - use specific property
        let colors = ColorScheme {
            inactive_file_pane_cursor_background_color: Some("Blue".to_string()),
            inactive_background_color: Some("Gray".to_string()),
            background_color: "Black".to_string(),
            ..ColorScheme::default()
        };
        assert_eq!(colors.get_inactive_file_pane_cursor_background(), "Blue");
        
        // Test 2: Specific property not set - fall back to inactive_background_color
        let colors = ColorScheme {
            inactive_file_pane_cursor_background_color: None,
            inactive_background_color: Some("Gray".to_string()),
            background_color: "Black".to_string(),
            ..ColorScheme::default()
        };
        assert_eq!(colors.get_inactive_file_pane_cursor_background(), "Gray");
        
        // Test 3: Both not set - fall back to background_color
        let colors = ColorScheme {
            inactive_file_pane_cursor_background_color: None,
            inactive_background_color: None,
            background_color: "Black".to_string(),
            ..ColorScheme::default()
        };
        assert_eq!(colors.get_inactive_file_pane_cursor_background(), "Black");
    }
    
    #[test]
    fn test_json_deserialization_with_old_color_names() {
        // Test that config with only old color names works correctly
        let json = r#"{
            "ShowHiddenFiles": false,
            "ShowSystem": false,
            "DateFormat": "%Y-%m-%d %H:%M",
            "TimeFormat": "TwentyFourHour",
            "CjkWidth": 2,
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
            "PaneBorderColor": "Gray",
            "TopSeparatorForegroundColor": "Gray",
            "TopSeparatorBackgroundColor": "Black",
            "DialogHelpForegroundColor": "White",
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
        }"#;
        
        let config: serde_json::Result<DisplayConfig> = serde_json::from_str(json);
        assert!(config.is_ok(), "Failed to deserialize config with old color names");
        
        let config = config.unwrap();
        
        // Verify backward compatibility - old names should work
        assert_eq!(config.colors.get_file_pane_cursor_foreground(), "Black");
        assert_eq!(config.colors.get_file_pane_cursor_background(), "Cyan");
    }
    
    #[test]
    fn test_json_deserialization_with_new_color_names() {
        // Test that config with new color names works correctly
        let json = r#"{
            "ShowHiddenFiles": false,
            "ShowSystem": false,
            "DateFormat": "%Y-%m-%d %H:%M",
            "TimeFormat": "TwentyFourHour",
            "CjkWidth": 2,
            "ForegroundColor": "White",
            "BackgroundColor": "Black",
            "HighlightForegroundColor": "Black",
            "HighlightBackgroundColor": "Cyan",
            "FilePaneCursorForegroundColor": "Yellow",
            "FilePaneCursorBackgroundColor": "Blue",
            "InactiveFilePaneCursorForegroundColor": "Gray",
            "InactiveFilePaneCursorBackgroundColor": "DarkGray",
            "InactiveForegroundColor": "LightGray",
            "InactiveBackgroundColor": "DarkBlue",
            "PaneInfoForegroundColor": "White",
            "PaneInfoBackgroundColor": "Gray",
            "MarkedFileColor": "Cyan",
            "DirectoryColor": "BrightCyan",
            "DirectoryBackgroundColor": "Black",
            "InactiveDirectoryColor": "Cyan",
            "InactiveDirectoryBackgroundColor": "Black",
            "FilenameLabelForegroundColor": "White",
            "FilenameLabelBackgroundColor": "Blue",
            "PaneBorderColor": "Gray",
            "TopSeparatorForegroundColor": "Gray",
            "TopSeparatorBackgroundColor": "Black",
            "DialogHelpForegroundColor": "White",
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
        }"#;
        
        let config: serde_json::Result<DisplayConfig> = serde_json::from_str(json);
        assert!(config.is_ok(), "Failed to deserialize config with new color names");
        
        let config = config.unwrap();
        
        // Verify new color names are used
        assert_eq!(config.colors.get_file_pane_cursor_foreground(), "Yellow");
        assert_eq!(config.colors.get_file_pane_cursor_background(), "Blue");
        assert_eq!(config.colors.get_inactive_file_pane_cursor_foreground(), "Gray");
        assert_eq!(config.colors.get_inactive_file_pane_cursor_background(), "DarkGray");
        assert_eq!(config.colors.get_inactive_foreground(), "LightGray");
        assert_eq!(config.colors.get_inactive_background(), "DarkBlue");
    }
    
    #[test]
    fn test_json_deserialization_with_mixed_color_names() {
        // Test that config with mix of old and new names works (new takes precedence)
        let json = r#"{
            "ShowHiddenFiles": false,
            "ShowSystem": false,
            "DateFormat": "%Y-%m-%d %H:%M",
            "TimeFormat": "TwentyFourHour",
            "CjkWidth": 2,
            "ForegroundColor": "White",
            "BackgroundColor": "Black",
            "HighlightForegroundColor": "Black",
            "HighlightBackgroundColor": "Cyan",
            "FilePaneCursorForegroundColor": "Red",
            "MarkedFileColor": "Cyan",
            "DirectoryColor": "BrightCyan",
            "DirectoryBackgroundColor": "Black",
            "InactiveDirectoryColor": "Cyan",
            "InactiveDirectoryBackgroundColor": "Black",
            "FilenameLabelForegroundColor": "White",
            "FilenameLabelBackgroundColor": "Blue",
            "PaneBorderColor": "Gray",
            "TopSeparatorForegroundColor": "Gray",
            "TopSeparatorBackgroundColor": "Black",
            "DialogHelpForegroundColor": "White",
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
        }"#;
        
        let config: serde_json::Result<DisplayConfig> = serde_json::from_str(json);
        assert!(config.is_ok(), "Failed to deserialize config with mixed color names");
        
        let config = config.unwrap();
        
        // New property should take precedence over old
        assert_eq!(config.colors.get_file_pane_cursor_foreground(), "Red");
        // Old property should be used when new is not specified
        assert_eq!(config.colors.get_file_pane_cursor_background(), "Cyan");
    }
    
    #[test]
    fn test_all_ui_areas_have_colors() {
        // Comprehensive test that all UI areas have proper color configuration
        let colors = ColorScheme::default();
        
        // UI Area 1: Tab bar
        assert!(!colors.active_tab_foreground_color.is_empty());
        assert!(!colors.active_tab_background_color.is_empty());
        assert!(!colors.inactive_tab_foreground_color.is_empty());
        assert!(!colors.inactive_tab_background_color.is_empty());
        assert!(!colors.tabbar_background_color.is_empty());
        
        // UI Area 2: Path display
        assert!(!colors.foreground_color.is_empty());
        assert!(!colors.background_color.is_empty());
        
        // UI Area 3: Top separator
        assert!(!colors.top_separator_foreground_color.is_empty());
        assert!(!colors.top_separator_background_color.is_empty());
        
        // UI Area 4: Active file pane
        assert!(!colors.get_file_pane_cursor_foreground().is_empty());
        assert!(!colors.get_file_pane_cursor_background().is_empty());
        assert!(!colors.marked_file_color.is_empty());
        assert!(!colors.directory_color.is_empty());
        assert!(!colors.directory_background_color.is_empty());
        
        // UI Area 4: Inactive file pane
        assert!(!colors.get_inactive_foreground().is_empty());
        assert!(!colors.get_inactive_background().is_empty());
        assert!(!colors.get_inactive_file_pane_cursor_foreground().is_empty());
        assert!(!colors.get_inactive_file_pane_cursor_background().is_empty());
        assert!(!colors.inactive_directory_color.is_empty());
        assert!(!colors.inactive_directory_background_color.is_empty());
        
        // UI Area 5: Pane info bar
        assert!(colors.pane_info_foreground_color.is_some());
        assert!(colors.pane_info_background_color.is_some());
        
        // UI Area 6: Filename label
        assert!(!colors.filename_label_foreground_color.is_empty());
        assert!(!colors.filename_label_background_color.is_empty());
        
        // UI Area 7: Task view (uses foreground/background)
        // Already verified above
    }
    
    #[test]
    fn test_color_fallback_chain_completeness() {
        // Test that all fallback chains eventually resolve to a value
        
        // Create a minimal color scheme with only required fields
        let colors = ColorScheme {
            foreground_color: "White".to_string(),
            background_color: "Black".to_string(),
            highlight_foreground_color: "Black".to_string(),
            highlight_background_color: "Cyan".to_string(),
            file_pane_cursor_foreground_color: None,
            file_pane_cursor_background_color: None,
            inactive_file_pane_cursor_foreground_color: None,
            inactive_file_pane_cursor_background_color: None,
            inactive_foreground_color: None,
            inactive_background_color: None,
            pane_info_foreground_color: None,
            pane_info_background_color: None,
            marked_file_color: "Cyan".to_string(),
            directory_color: "BrightCyan".to_string(),
            directory_background_color: "Black".to_string(),
            inactive_directory_color: "Cyan".to_string(),
            inactive_directory_background_color: "Black".to_string(),
            filename_label_foreground_color: "White".to_string(),
            filename_label_background_color: "Blue".to_string(),
            pane_border_color: "Gray".to_string(),
            top_separator_foreground_color: "Gray".to_string(),
            top_separator_background_color: "Black".to_string(),
            dialog_help_foreground_color: "White".to_string(),
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
        };
        
        // All fallback chains should resolve to non-empty strings
        assert_eq!(colors.get_file_pane_cursor_foreground(), "Black");
        assert_eq!(colors.get_file_pane_cursor_background(), "Cyan");
        assert_eq!(colors.get_inactive_file_pane_cursor_foreground(), "White");
        assert_eq!(colors.get_inactive_file_pane_cursor_background(), "Black");
        assert_eq!(colors.get_inactive_foreground(), "White");
        assert_eq!(colors.get_inactive_background(), "Black");
    }

    #[test]
    fn test_viewer_large_file_threshold_default() {
        let config = AppConfig::default();
        assert_eq!(config.viewer_large_file_threshold_mb, 100);
    }

}
