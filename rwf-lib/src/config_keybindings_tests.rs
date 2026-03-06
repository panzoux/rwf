//! Tests for key bindings configuration
//! **Validates: Requirements 17.4, 17.5, 18.1, 18.2, 18.3, 18.7**

#[cfg(test)]
mod tests {
    use crate::config::{ConfigManager, KeyBindings, Action};
    use tempfile::TempDir;
    
    #[test]
    fn test_twf_compatible_defaults() {
        let keybindings = KeyBindings::default();
        
        // Verify TWF-compatible default key bindings
        assert_eq!(keybindings.normal_mode.get("C"), Some(&Action::Copy));
        assert_eq!(keybindings.normal_mode.get("M"), Some(&Action::Move));
        assert_eq!(keybindings.normal_mode.get("D"), Some(&Action::Delete));
        assert_eq!(keybindings.normal_mode.get("R"), Some(&Action::Rename));
        assert_eq!(keybindings.normal_mode.get("Shift+K"), Some(&Action::CreateDirectory));
    }
    
    #[test]
    fn test_navigation_key_bindings() {
        let keybindings = KeyBindings::default();
        
        assert_eq!(keybindings.normal_mode.get("Tab"), Some(&Action::SwitchPane));
        assert_eq!(keybindings.normal_mode.get("Up"), Some(&Action::CursorUp));
        assert_eq!(keybindings.normal_mode.get("Down"), Some(&Action::CursorDown));
        assert_eq!(keybindings.normal_mode.get("k"), Some(&Action::CursorUp));
        assert_eq!(keybindings.normal_mode.get("j"), Some(&Action::CursorDown));
        assert_eq!(keybindings.normal_mode.get("Home"), Some(&Action::Home));
        assert_eq!(keybindings.normal_mode.get("End"), Some(&Action::End));
        assert_eq!(keybindings.normal_mode.get("PageUp"), Some(&Action::PageUp));
        assert_eq!(keybindings.normal_mode.get("PageDown"), Some(&Action::PageDown));
    }
    
    #[test]
    fn test_directory_navigation_key_bindings() {
        let keybindings = KeyBindings::default();
        
        assert_eq!(keybindings.normal_mode.get("Enter"), Some(&Action::EnterDirectory));
        assert_eq!(keybindings.normal_mode.get("Backspace"), Some(&Action::ParentDirectory));
        assert_eq!(keybindings.normal_mode.get("Left"), Some(&Action::ParentDirectory));
    }
    
    #[test]
    fn test_marking_key_bindings() {
        let keybindings = KeyBindings::default();
        
        assert_eq!(keybindings.normal_mode.get("Space"), Some(&Action::ToggleMark));
        assert_eq!(keybindings.normal_mode.get("*"), Some(&Action::MarkAll));
        assert_eq!(keybindings.normal_mode.get("Ctrl+U"), Some(&Action::UnmarkAll));
    }
    
    #[test]
    fn test_search_key_bindings() {
        let keybindings = KeyBindings::default();
        
        assert_eq!(keybindings.normal_mode.get("/"), Some(&Action::StartSearch));
        assert_eq!(keybindings.normal_mode.get("Ctrl+F"), Some(&Action::StartSearch));
    }
    
    #[test]
    fn test_history_key_bindings() {
        let keybindings = KeyBindings::default();
        
        assert_eq!(keybindings.normal_mode.get("Alt+Left"), Some(&Action::HistoryBack));
        assert_eq!(keybindings.normal_mode.get("Alt+Right"), Some(&Action::HistoryForward));
    }
    
    #[test]
    fn test_config_reload_key_binding() {
        let keybindings = KeyBindings::default();
        
        assert_eq!(keybindings.normal_mode.get("Shift+Z"), Some(&Action::ReloadConfig));
    }
    
    #[test]
    fn test_quit_key_bindings() {
        let keybindings = KeyBindings::default();
        
        assert_eq!(keybindings.normal_mode.get("Q"), Some(&Action::Quit));
        assert_eq!(keybindings.normal_mode.get("Escape"), Some(&Action::Quit));
    }
    
    #[test]
    fn test_custom_key_mappings() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        // Write custom keybindings
        let keybindings_json = r#"{
            "NormalMode": {
                "c": "Copy",
                "m": "Move",
                "d": "Delete"
            },
            "SearchMode": {},
            "DialogMode": {},
            "ViewerMode": {}
        }"#;
        
        std::fs::write(&keybindings_path, keybindings_json).unwrap();
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let keybindings = manager.load_keybindings().unwrap();
        
        // Verify custom mappings (lowercase keys)
        assert_eq!(keybindings.normal_mode.get("c"), Some(&Action::Copy));
        assert_eq!(keybindings.normal_mode.get("m"), Some(&Action::Move));
        assert_eq!(keybindings.normal_mode.get("d"), Some(&Action::Delete));
    }
    
    #[test]
    fn test_save_and_load_keybindings() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path.clone());
        
        let keybindings = KeyBindings::default();
        manager.save_keybindings(&keybindings).unwrap();
        
        let loaded_keybindings = manager.load_keybindings().unwrap();
        assert_eq!(loaded_keybindings.normal_mode.get("Tab"), Some(&Action::SwitchPane));
    }
    
    #[test]
    fn test_invalid_keybindings_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let keybindings_path = temp_dir.path().join("keybindings.json");
        
        // Write invalid JSON
        std::fs::write(&keybindings_path, "{ invalid json }").unwrap();
        
        let manager = ConfigManager::with_paths(config_path, keybindings_path);
        let result = manager.load_keybindings();
        
        assert!(result.is_err());
    }
}
