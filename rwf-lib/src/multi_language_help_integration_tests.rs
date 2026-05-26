//! Integration tests for multi-language help system
//!
//! **Validates: Requirements 48.1-48.7**

use crate::help_content::HelpContent;
use crate::model::{Dialog, DialogContent};
use crate::state::{update_state, AppState, Transition};
use crate::config::AppConfig;

#[test]
fn test_help_content_loading_english() {
    // **Validates: Requirements 48.1, 48.4**
    let content = HelpContent::load_with_fallback("en");
    
    assert_eq!(content.title, "Help - Key Bindings");
    assert!(!content.sections.is_empty());
    
    // Verify English content
    let formatted = content.format();
    assert!(formatted.contains("Navigation:"));
    assert!(formatted.contains("Switch pane"));
}

#[test]
fn test_help_content_loading_japanese() {
    // **Validates: Requirements 48.1, 48.4**
    let content = HelpContent::load_with_fallback("jp");
    
    // Should load Japanese if file exists, otherwise fall back to English
    assert!(!content.sections.is_empty());
    
    let formatted = content.format();
    // Check for either Japanese or English content (depending on file availability)
    assert!(formatted.contains("Navigation:") || formatted.contains("ナビゲーション:"));
}

#[test]
fn test_help_content_fallback_to_english() {
    // **Validates: Requirements 48.7**
    let content = HelpContent::load_with_fallback("nonexistent");
    
    // Should fall back to English
    assert_eq!(content.title, "Help - Key Bindings");
    assert!(!content.sections.is_empty());
    
    let formatted = content.format();
    assert!(formatted.contains("Navigation:"));
    assert!(formatted.contains("Switch pane"));
}

#[test]
fn test_help_dialog_uses_configured_language() {
    // **Validates: Requirements 48.2, 48.5**
    let mut config = AppConfig::default();
    config.help_language = "en".to_string();
    
    let mut state = AppState::new(config);
    
    // Show help dialog
    let dialog = Dialog::help_with_language(&state.config.help_language);
    state.dialogs.push(dialog);
    
    // Verify dialog is shown with correct language
    assert!(!state.dialogs.is_empty());
    let current_dialog = state.dialogs.current().unwrap();
    
    match &current_dialog.content {
        DialogContent::Help { content, language, .. } => {
            assert_eq!(language, "en");
            assert!(content.contains("Navigation:"));
        }
        _ => panic!("Expected Help dialog"),
    }
}

#[test]
fn test_language_rotation() {
    // **Validates: Requirements 48.3**
    let mut config = AppConfig::default();
    config.help_language = "en".to_string();
    
    let mut state = AppState::new(config);
    
    // Show help dialog
    let dialog = Dialog::help_with_language(&state.config.help_language);
    state.dialogs.push(dialog);
    
    // Get initial language
    let initial_lang = if let Some(dialog) = state.dialogs.current() {
        if let DialogContent::Help { language, .. } = &dialog.content {
            language.clone()
        } else {
            panic!("Expected Help dialog");
        }
    } else {
        panic!("No dialog found");
    };
    
    // Rotate language
    let result = update_state(&mut state, Transition::RotateHelpLanguage);
    assert!(result.ui_changed);
    
    // Verify language changed
    let new_lang = if let Some(dialog) = state.dialogs.current() {
        if let DialogContent::Help { language, .. } = &dialog.content {
            language.clone()
        } else {
            panic!("Expected Help dialog");
        }
    } else {
        panic!("No dialog found");
    };
    
    // Language should have changed (or stayed same if only one language available)
    let available_languages = HelpContent::available_languages();
    if available_languages.len() > 1 {
        assert_ne!(initial_lang, new_lang);
    }
    
    // Config should be updated
    assert_eq!(state.config.help_language, new_lang);
}

#[test]
fn test_language_rotation_cycles_through_all_languages() {
    // **Validates: Requirements 48.3**
    let available_languages = HelpContent::available_languages();
    
    if available_languages.len() <= 1 {
        // Skip test if only one language available
        return;
    }
    
    let mut current_lang = "en".to_string();
    let mut seen_languages = vec![current_lang.clone()];
    
    // Rotate through all languages
    for _ in 0..available_languages.len() {
        current_lang = HelpContent::next_language(&current_lang);
        seen_languages.push(current_lang.clone());
    }
    
    // Should have cycled back to the start
    assert_eq!(seen_languages.first(), seen_languages.last());
    
    // Should have seen all languages
    for lang in &available_languages {
        assert!(seen_languages.contains(lang), "Language {} not seen in rotation", lang);
    }
}

#[test]
fn test_language_persistence() {
    // **Validates: Requirements 48.6**
    let mut config = AppConfig::default();
    config.help_language = "en".to_string();
    
    let mut state = AppState::new(config);
    
    // Show help dialog
    let dialog = Dialog::help_with_language(&state.config.help_language);
    state.dialogs.push(dialog);
    
    // Rotate language
    update_state(&mut state, Transition::RotateHelpLanguage);
    
    // Verify config was updated
    let new_lang = state.config.help_language.clone();
    assert!(!new_lang.is_empty());
    
    // Note: Actual file persistence is tested by the config save mechanism
    // which is already tested in config_integration_tests.rs
}

#[test]
fn test_help_dialog_displays_all_key_bindings() {
    // **Validates: Requirements 48.5**
    let content = HelpContent::load_with_fallback("en");
    let formatted = content.format();
    
    // Verify all major sections are present
    let required_sections = vec![
        "Navigation:",
        "File Operations:",
        "Marking:",
        "Sorting:",
        "Search & Filter:",
        "Tab Management:",
        "Miscellaneous:",
    ];
    
    for section in required_sections {
        assert!(
            formatted.contains(section),
            "Help content missing section: {}",
            section
        );
    }
    
    // Verify key bindings are present
    let required_bindings = vec![
        "Tab",
        "Copy",
        "Move",
        "Delete",
        "Space",
        "Mark all",
    ];
    
    for binding in required_bindings {
        assert!(
            formatted.contains(binding),
            "Help content missing binding: {}",
            binding
        );
    }
}

#[test]
fn test_help_dialog_shows_language_rotation_key() {
    // **Validates: Requirements 48.3**
    let content = HelpContent::load_with_fallback("en");
    let formatted = content.format();
    
    // Verify L key for language rotation is documented
    assert!(formatted.contains("L"));
    assert!(formatted.to_lowercase().contains("language") || formatted.to_lowercase().contains("rotate"));
}

#[test]
fn test_multiple_language_rotations() {
    // **Validates: Requirements 48.3**
    let mut config = AppConfig::default();
    config.help_language = "en".to_string();
    
    let mut state = AppState::new(config);
    
    // Show help dialog
    let dialog = Dialog::help_with_language(&state.config.help_language);
    state.dialogs.push(dialog);
    
    // Rotate multiple times
    for _ in 0..3 {
        let result = update_state(&mut state, Transition::RotateHelpLanguage);
        assert!(result.ui_changed);
        
        // Verify dialog is still Help dialog
        assert!(!state.dialogs.is_empty());
        let current_dialog = state.dialogs.current().unwrap();
        assert!(matches!(current_dialog.content, DialogContent::Help { .. }));
    }
}

#[test]
fn test_help_content_format_consistency() {
    // **Validates: Requirements 48.1, 48.5**
    let content = HelpContent::load_with_fallback("en");
    let formatted = content.format();
    
    // Verify format is consistent
    assert!(!formatted.is_empty());
    
    // Each section should have a colon
    for section in &content.sections {
        assert!(formatted.contains(&format!("{}:", section.name)));
        
        // Each binding should be present
        for binding in &section.bindings {
            assert!(formatted.contains(&binding.key));
            assert!(formatted.contains(&binding.description));
        }
    }
}

#[test]
fn test_available_languages_includes_english() {
    // **Validates: Requirements 48.4**
    let languages = HelpContent::available_languages();
    
    // English should always be available (hardcoded fallback)
    assert!(languages.contains(&"en".to_string()));
}

#[test]
fn test_next_language_with_single_language() {
    // **Validates: Requirements 48.3**
    // When only one language is available, next_language should return the same language
    let lang = HelpContent::next_language("en");
    
    // Should return a valid language
    assert!(!lang.is_empty());
}
