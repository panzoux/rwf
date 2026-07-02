//! Help content loading and management
//!
//! This module handles loading help content from language-specific JSON files,
//! and provides the dynamic HelpBuilder for the Phase 6.7 help viewer.

use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Action description types ────────────────────────────────────────────────

/// One action's description, as read from action_descriptions.*.json
#[derive(Debug, Clone, Deserialize)]
pub struct ActionDesc {
    pub description: String,
}

/// One category group in the description file
#[derive(Debug, Clone, Deserialize)]
pub struct CategoryDesc {
    pub name: String,
    pub actions: std::collections::HashMap<String, ActionDesc>,
}

/// Top-level structure for one mode section in the description file
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ModeDesc {
    pub categories: Vec<CategoryDesc>,
}

/// Root of action_descriptions.*.json
#[derive(Debug, Clone, Deserialize)]
pub struct ActionDescriptions {
    #[serde(rename = "NormalMode")]
    pub normal_mode: ModeDesc,
    #[serde(rename = "LeapMode", default)]
    pub leap_mode: ModeDesc,
    #[serde(rename = "ViewerMode")]
    pub viewer_mode: ModeDesc,
}

// Embedded English descriptions (always available as fallback)
const EMBEDDED_EN: &str = include_str!("../resources/action_descriptions.en.json");
const EMBEDDED_JP: &str = include_str!("../resources/action_descriptions.jp.json");

/// Embedded default custom_functions.json — exported by `--export-config-files`
pub const DEFAULT_CUSTOM_FUNCTIONS: &str =
    include_str!("../resources/default_custom_functions.json");

/// Embedded default menu_config.json — exported by `--export-config-files`
pub const DEFAULT_MENU_CONFIG: &str =
    include_str!("../resources/default_menu_config.json");

impl ActionDescriptions {
    /// Load action descriptions for the given language.
    /// Checks %APPDATA%\rwf\ first, then falls back to embedded.
    pub fn load(lang: &str) -> Self {
        // Try user-provided override file first
        if let Some(config_dir) = dirs::config_dir() {
            let path = config_dir.join("rwf").join(format!("action_descriptions.{}.json", lang));
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(desc) = serde_json::from_str::<Self>(&content) {
                    return desc;
                }
            }
        }
        // Try embedded
        let embedded = if lang == "jp" { EMBEDDED_JP } else { EMBEDDED_EN };
        serde_json::from_str(embedded)
            .expect("embedded action_descriptions.*.json is always valid JSON")
    }

    pub fn available_languages() -> Vec<String> {
        let mut langs = vec!["en".to_string()];
        // Check for user override files
        if let Some(config_dir) = dirs::config_dir() {
            let jp = config_dir.join("rwf").join("action_descriptions.jp.json");
            if jp.exists() && !langs.contains(&"jp".to_string()) {
                langs.push("jp".to_string());
            }
        }
        // Always include jp since it's embedded
        if !langs.contains(&"jp".to_string()) {
            langs.push("jp".to_string());
        }
        langs
    }

    pub fn next_language(current: &str) -> String {
        let languages = Self::available_languages();
        let idx = languages.iter().position(|l| l == current).unwrap_or(0);
        languages[(idx + 1) % languages.len()].clone()
    }
}

// ── Help builder ─────────────────────────────────────────────────────────────

/// Resolve the effective editor label for display in help entries.
/// Shows the configured editor name and its mode (terminal/GUI/default).
fn active_editor_label(config: &crate::config::AppConfig) -> String {
    if let Some(ref ed) = config.terminal_editor {
        format!("{} (terminal)", ed)
    } else if let Some(ref ed) = config.editor_command {
        format!("{} (GUI)", ed)
    } else {
        #[cfg(target_os = "windows")]
        { "notepad (default)".to_string() }
        #[cfg(not(target_os = "windows"))]
        { "$EDITOR (default)".to_string() }
    }
}

/// Substitute `$ActiveEditor` in a description string with the effective editor label.
fn resolve_description(desc: &str, config: &crate::config::AppConfig) -> String {
    if desc.contains("$ActiveEditor") {
        desc.replace("$ActiveEditor", &active_editor_label(config))
    } else {
        desc.to_string()
    }
}

/// Build a complete list of `HelpEntry` from runtime key bindings, action descriptions,
/// and custom functions. `config` is used to resolve `$ActiveEditor` in descriptions.
pub fn build_help_entries(
    key_bindings: &crate::input::KeyBindings,
    descriptions: &ActionDescriptions,
    custom_functions: &[crate::model::dialog::CustomFunction],
    show_unbound: bool,
    config: &crate::config::AppConfig,
) -> Vec<crate::model::dialog::HelpEntry> {
    use crate::model::dialog::{HelpEntry, HelpTab};

    let mut entries: Vec<HelpEntry> = Vec::new();

    let normal_map = key_bindings.normal_action_to_keys();
    let viewer_map = key_bindings.viewer_action_to_keys();
    let dialog_map = key_bindings.dialog_action_to_keys();
    let leap_map   = key_bindings.leap_action_to_keys();

    // Normal mode actions
    for cat in &descriptions.normal_mode.categories {
        for (action_name, action_desc) in &cat.actions {
            let keys = normal_map.get(action_name).cloned().unwrap_or_default();
            if !show_unbound && keys.is_empty() {
                continue;
            }
            entries.push(HelpEntry {
                category: cat.name.clone(),
                description: resolve_description(&action_desc.description, config),
                keys,
                action_name: action_name.clone(),
                tab: HelpTab::NormalMode,
            });
        }
    }

    // Viewer mode actions
    for cat in &descriptions.viewer_mode.categories {
        for (action_name, action_desc) in &cat.actions {
            let keys = viewer_map.get(action_name).cloned().unwrap_or_default();
            if !show_unbound && keys.is_empty() {
                continue;
            }
            entries.push(HelpEntry {
                category: cat.name.clone(),
                description: resolve_description(&action_desc.description, config),
                keys,
                action_name: action_name.clone(),
                tab: HelpTab::ViewerMode,
            });
        }
    }

    // Leap mode actions
    for cat in &descriptions.leap_mode.categories {
        for (action_name, action_desc) in &cat.actions {
            let keys = leap_map.get(action_name).cloned().unwrap_or_default();
            if !show_unbound && keys.is_empty() {
                continue;
            }
            entries.push(HelpEntry {
                category: cat.name.clone(),
                description: resolve_description(&action_desc.description, config),
                keys,
                action_name: action_name.clone(),
                tab: HelpTab::LeapMode,
            });
        }
    }

    // Dialog mode actions (use normal_map as dialog_map is typically empty)
    if !dialog_map.is_empty() {
        for (action_name, keys) in &dialog_map {
            entries.push(HelpEntry {
                category: "Dialog".to_string(),
                description: action_name.clone(),
                keys: keys.clone(),
                action_name: action_name.clone(),
                tab: HelpTab::DialogMode,
            });
        }
    }

    // Custom functions tab
    for func in custom_functions {
        let category = func.category.clone()
            .unwrap_or_else(|| "Custom Functions".to_string());
        let description = if func.is_menu() {
            let menu_name = func.menu.as_ref()
                .and_then(|m| match m {
                    crate::model::dialog::MenuContent::File(f) => Some(f.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| func.name.clone());
            format!("opens menu {}", menu_name)
        } else {
            func.description.clone().unwrap_or_else(|| func.name.clone())
        };
        // Look up whether this custom function has a bound key
        let keys = normal_map.get(&func.name).cloned().unwrap_or_default();
        if !show_unbound && keys.is_empty() {
            continue;
        }
        entries.push(HelpEntry {
            category,
            description,
            keys,
            action_name: func.name.clone(),
            tab: HelpTab::CustomFunctions,
        });
    }

    entries
}

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
    
    /// Get list of available languages (delegates to ActionDescriptions)
    pub fn available_languages() -> Vec<String> {
        ActionDescriptions::available_languages()
    }

    /// Get the next language in rotation (delegates to ActionDescriptions)
    pub fn next_language(current: &str) -> String {
        ActionDescriptions::next_language(current)
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
        // "jp" is an embedded default resource (action_descriptions.jp.json,
        // added in the dynamic help viewer work) and is always available
        // alongside "en", regardless of any on-disk override files. So
        // rotation always cycles en -> jp -> en; see also
        // help_viewer_tests::test_next_language_cycles for the same
        // assertion via ActionDescriptions::next_language.
        assert_eq!(HelpContent::next_language("en"), "jp");
        assert_eq!(HelpContent::next_language("jp"), "en");
    }
    
    #[test]
    fn test_load_with_fallback() {
        // Should always succeed with fallback
        let content = HelpContent::load_with_fallback("nonexistent");
        assert_eq!(content.title, "Help - Key Bindings");
    }
}
