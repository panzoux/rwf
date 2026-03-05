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
            "ShowHidden": false,
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
}