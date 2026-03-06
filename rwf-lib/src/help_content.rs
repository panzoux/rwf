//! Help content loading and management
//!
//! This module handles loading help content from language-specific JSON files.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// A key binding entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    pub description: String,
}

/// A section of help content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpSection {
    pub name: String,
    pub bindings: Vec<KeyBinding>,
}

/// Help content structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpContent {
    pub title: String,
    pub sections: Vec<HelpSection>,
}

impl HelpContent {
    /// Load help content from a language-specific JSON file
    /// 
    /// **Validates: Requirements 48.1**
    pub fn load_from_file(lang: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let filename = format!("help.{}.json", lang);
        let path = Path::new(&filename);
        
        if !path.exists() {
            return Err(format!("Help file not found: {}", filename).into());
        }
        
        let content = std::fs::read_to_string(path)?;
        let help_content: HelpContent = serde_json::from_str(&content)?;
        
        Ok(help_content)
    }
    
    /// Load help content with fallback to English
    /// 
    /// **Validates: Requirements 48.4, 48.7**
    pub fn load_with_fallback(lang: &str) -> Self {
        // Try to load the requested language
        if let Ok(content) = Self::load_from_file(lang) {
            return content;
        }
        
        // Fall back to English
        if lang != "en" {
            if let Ok(content) = Self::load_from_file("en") {
                return content;
            }
        }
        
        // If all else fails, return hardcoded English content
        Self::default_english()
    }
    
    /// Get default English help content (hardcoded fallback)
    fn default_english() -> Self {
        HelpContent {
            title: "Help - Key Bindings".to_string(),
            sections: vec![
                HelpSection {
                    name: "Navigation".to_string(),
                    bindings: vec![
                        KeyBinding { key: "Tab".to_string(), description: "Switch pane".to_string() },
                        KeyBinding { key: "Up/Down, j/k".to_string(), description: "Move cursor".to_string() },
                        KeyBinding { key: "Home/End".to_string(), description: "Jump to first/last entry".to_string() },
                        KeyBinding { key: "PageUp/PageDown".to_string(), description: "Page navigation".to_string() },
                        KeyBinding { key: "Enter".to_string(), description: "Enter directory".to_string() },
                        KeyBinding { key: "Backspace/Left".to_string(), description: "Parent directory".to_string() },
                        KeyBinding { key: "Alt+Left/Right".to_string(), description: "History navigation".to_string() },
                    ],
                },
                HelpSection {
                    name: "File Operations".to_string(),
                    bindings: vec![
                        KeyBinding { key: "C".to_string(), description: "Copy".to_string() },
                        KeyBinding { key: "M".to_string(), description: "Move".to_string() },
                        KeyBinding { key: "D".to_string(), description: "Delete".to_string() },
                        KeyBinding { key: "R".to_string(), description: "Rename".to_string() },
                        KeyBinding { key: "Shift+K".to_string(), description: "Create directory".to_string() },
                    ],
                },
                HelpSection {
                    name: "Marking".to_string(),
                    bindings: vec![
                        KeyBinding { key: "Space".to_string(), description: "Toggle mark".to_string() },
                        KeyBinding { key: "*".to_string(), description: "Mark all".to_string() },
                        KeyBinding { key: "Ctrl+U".to_string(), description: "Unmark all".to_string() },
                        KeyBinding { key: "@".to_string(), description: "Wildcard marking".to_string() },
                        KeyBinding { key: "Ctrl+Space".to_string(), description: "Range marking".to_string() },
                        KeyBinding { key: "Shift+Home".to_string(), description: "Invert marks".to_string() },
                    ],
                },
                HelpSection {
                    name: "Sorting".to_string(),
                    bindings: vec![
                        KeyBinding { key: "s+n".to_string(), description: "Sort by name".to_string() },
                        KeyBinding { key: "s+s".to_string(), description: "Sort by size".to_string() },
                        KeyBinding { key: "s+d".to_string(), description: "Sort by date".to_string() },
                        KeyBinding { key: "s+e".to_string(), description: "Sort by extension".to_string() },
                    ],
                },
                HelpSection {
                    name: "Search & Filter".to_string(),
                    bindings: vec![
                        KeyBinding { key: "/, Ctrl+F".to_string(), description: "Start search".to_string() },
                        KeyBinding { key: "f".to_string(), description: "File mask filter".to_string() },
                        KeyBinding { key: "Ctrl+K".to_string(), description: "Clear search/filter".to_string() },
                        KeyBinding { key: "Escape".to_string(), description: "Exit search mode".to_string() },
                    ],
                },
                HelpSection {
                    name: "Tab Management".to_string(),
                    bindings: vec![
                        KeyBinding { key: "Ctrl+N".to_string(), description: "New tab".to_string() },
                        KeyBinding { key: "Ctrl+T/Ctrl+B".to_string(), description: "Tab selector".to_string() },
                        KeyBinding { key: "Ctrl+W".to_string(), description: "Close tab".to_string() },
                        KeyBinding { key: "Ctrl+Right".to_string(), description: "Next tab".to_string() },
                        KeyBinding { key: "Ctrl+Left".to_string(), description: "Previous tab".to_string() },
                    ],
                },
                HelpSection {
                    name: "Miscellaneous".to_string(),
                    bindings: vec![
                        KeyBinding { key: "Q, Escape".to_string(), description: "Quit application".to_string() },
                        KeyBinding { key: "?, F1".to_string(), description: "Show this help".to_string() },
                        KeyBinding { key: "Ctrl+J".to_string(), description: "Job manager".to_string() },
                        KeyBinding { key: "L".to_string(), description: "Rotate language (in help dialog)".to_string() },
                    ],
                },
            ],
        }
    }
    
    /// Format help content as a string for display
    pub fn format(&self) -> String {
        let mut content = String::new();
        
        for section in &self.sections {
            content.push_str(&format!("{}:\n", section.name));
            for binding in &section.bindings {
                content.push_str(&format!("  {:20} - {}\n", binding.key, binding.description));
            }
            content.push('\n');
        }
        
        content
    }
    
    /// Get list of available languages
    pub fn available_languages() -> Vec<String> {
        let mut languages = vec!["en".to_string()];
        
        // Check for other language files
        if Path::new("help.jp.json").exists() {
            languages.push("jp".to_string());
        }
        
        languages
    }
    
    /// Get the next language in rotation
    /// 
    /// **Validates: Requirements 48.3**
    pub fn next_language(current: &str) -> String {
        let languages = Self::available_languages();
        let current_index = languages.iter().position(|l| l == current).unwrap_or(0);
        let next_index = (current_index + 1) % languages.len();
        languages[next_index].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_english_content() {
        let content = HelpContent::default_english();
        assert_eq!(content.title, "Help - Key Bindings");
        assert!(!content.sections.is_empty());
        
        // Verify all expected sections are present
        let section_names: Vec<&str> = content.sections.iter().map(|s| s.name.as_str()).collect();
        assert!(section_names.contains(&"Navigation"));
        assert!(section_names.contains(&"File Operations"));
        assert!(section_names.contains(&"Marking"));
        assert!(section_names.contains(&"Sorting"));
        assert!(section_names.contains(&"Search & Filter"));
        assert!(section_names.contains(&"Tab Management"));
        assert!(section_names.contains(&"Miscellaneous"));
    }
    
    #[test]
    fn test_format_help_content() {
        let content = HelpContent::default_english();
        let formatted = content.format();
        
        assert!(formatted.contains("Navigation:"));
        assert!(formatted.contains("Tab"));
        assert!(formatted.contains("Switch pane"));
    }
    
    #[test]
    fn test_next_language_rotation() {
        // Test rotation with single language
        assert_eq!(HelpContent::next_language("en"), "en");
        
        // If jp file exists, test rotation
        if Path::new("help.jp.json").exists() {
            assert_eq!(HelpContent::next_language("en"), "jp");
            assert_eq!(HelpContent::next_language("jp"), "en");
        }
    }
    
    #[test]
    fn test_load_with_fallback() {
        // Should always succeed with fallback
        let content = HelpContent::load_with_fallback("nonexistent");
        assert_eq!(content.title, "Help - Key Bindings");
    }
}
