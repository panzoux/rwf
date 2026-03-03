//! Integration tests for custom functions
//!
//! Tests macro expansion, function execution, and PipeToAction directives.
//! **Validates: Requirements 28.1-28.15**

#[cfg(test)]
mod tests {
    use crate::macro_expander::MacroExpander;
    use crate::model::{CustomFunction, FileEntry, Location};
    use crate::state::{AppState, AppConfig};
    use crate::job::{JobSpec, JobKind, PipeToAction};
    use crate::pipe_to_action::{process_pipe_to_action, PipeToActionResult};
    use std::path::PathBuf;
    use std::time::SystemTime;
    
    fn create_test_state_with_files() -> AppState {
        let config = AppConfig::default();
        let mut state = AppState::new(config);
        
        // Add test files
        let test_location = Location::Local(PathBuf::from("/test"));
        state.tabs.tabs[0].left_pane.entries = vec![
            FileEntry {
                name: "file1.txt".to_string(),
                location: test_location.join("file1.txt"),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            FileEntry {
                name: "file2.rs".to_string(),
                location: test_location.join("file2.rs"),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: true,
                calculated_size: None,
            },
            FileEntry {
                name: "document.pdf".to_string(),
                location: test_location.join("document.pdf"),
                size: 5000,
                is_dir: false,
                is_hidden: false,
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        
        state
    }
    
    /// Test macro expansion with various macros
    /// **Validates: Requirements 28.2**
    #[test]
    fn test_macro_expansion_integration() {
        let state = create_test_state_with_files();
        let expander = MacroExpander::new();
        
        // Test $F macro (cursor file name)
        let func = CustomFunction::new("test", "echo $F");
        let result = expander.expand(&state, &func).unwrap();
        assert!(result.contains("file1.txt"));
        
        // Test $W macro (file name without extension)
        let func = CustomFunction::new("test", "echo $W");
        let result = expander.expand(&state, &func).unwrap();
        assert!(result.contains("file1"));
        assert!(!result.contains(".txt"));
        
        // Test $E macro (file extension)
        let func = CustomFunction::new("test", "echo $E");
        let result = expander.expand(&state, &func).unwrap();
        assert!(result.contains("txt"));
        
        // Test $# macro (file count)
        let func = CustomFunction::new("test", "echo $#");
        let result = expander.expand(&state, &func).unwrap();
        assert!(result.contains("3"));
        
        // Test $M macro (marked files)
        let func = CustomFunction::new("test", "process $M");
        let result = expander.expand(&state, &func).unwrap();
        assert!(result.contains("file2.rs"));
    }
    
    /// Test custom function job creation
    /// **Validates: Requirements 28.12**
    #[test]
    fn test_custom_function_job_creation() {
        let state = create_test_state_with_files();
        let expander = MacroExpander::new();
        
        let function = CustomFunction::new("test", "echo $F")
            .with_shell("bash")
            .with_description("Test function");
        
        // Expand the command
        let expanded_command = expander.expand(&state, &function).unwrap();
        
        // Create a job spec
        let working_dir = state.active_pane().current_location.clone();
        let job_spec = JobSpec::new(JobKind::ExecuteCustomFunction {
            command: expanded_command.clone(),
            working_dir: working_dir.clone(),
            pipe_to_action: None,
            shell: Some("bash".to_string()),
        });
        
        // Verify the job spec
        match &job_spec.kind {
            JobKind::ExecuteCustomFunction { command, working_dir: wd, shell, .. } => {
                assert_eq!(command, &expanded_command);
                assert_eq!(wd, &working_dir);
                assert_eq!(shell, &Some("bash".to_string()));
            }
            _ => panic!("Expected ExecuteCustomFunction job kind"),
        }
    }
    
    /// Test per-OS shell configuration
    /// **Validates: Requirements 28.8, 28.9**
    #[test]
    fn test_os_specific_shell_configuration() {
        let function = CustomFunction::new("test", "echo default");
        
        // Add OS-specific configuration
        #[cfg(target_os = "linux")]
        {
            use std::collections::HashMap;
            use crate::model::OsConfig;
            
            let mut os_specific = HashMap::new();
            os_specific.insert("linux".to_string(), OsConfig {
                command: "echo linux".to_string(),
                shell: Some("bash".to_string()),
            });
            function.os_specific = os_specific;
            
            assert_eq!(function.get_command(), "echo linux");
            assert_eq!(function.get_shell(), Some("bash"));
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            assert_eq!(function.get_command(), "echo default");
        }
    }
    
    /// Test PipeToAction: JumpToPath
    /// **Validates: Requirements 28.10, 28.13**
    #[test]
    fn test_pipe_to_action_jump_to_path() {
        let current_dir = std::env::current_dir().unwrap();
        let output = current_dir.to_str().unwrap();
        
        let result = process_pipe_to_action(&PipeToAction::JumpToPath, output);
        assert!(result.is_ok());
        
        match result.unwrap() {
            PipeToActionResult::JumpToPath(location) => {
                match location {
                    Location::Local(path) => assert_eq!(path, current_dir),
                    _ => panic!("Expected Local location"),
                }
            }
            _ => panic!("Expected JumpToPath result"),
        }
    }
    
    /// Test PipeToAction: ExecuteFile
    /// **Validates: Requirements 28.10, 28.14**
    #[test]
    fn test_pipe_to_action_execute_file() {
        // Use a file that exists (the current executable or a test file)
        let test_file = std::env::current_exe().unwrap();
        let output = test_file.to_str().unwrap();
        
        let result = process_pipe_to_action(&PipeToAction::ExecuteFile, output);
        assert!(result.is_ok());
        
        match result.unwrap() {
            PipeToActionResult::ExecuteFile(path) => {
                assert_eq!(path, test_file);
            }
            _ => panic!("Expected ExecuteFile result"),
        }
    }
    
    /// Test PipeToAction: ExecuteFileWithEditor
    /// **Validates: Requirements 28.10, 28.15**
    #[test]
    fn test_pipe_to_action_execute_file_with_editor() {
        let output = "/tmp/newfile.txt";
        
        let result = process_pipe_to_action(&PipeToAction::ExecuteFileWithEditor, output);
        assert!(result.is_ok());
        
        match result.unwrap() {
            PipeToActionResult::ExecuteFileWithEditor(path) => {
                assert_eq!(path, PathBuf::from("/tmp/newfile.txt"));
            }
            _ => panic!("Expected ExecuteFileWithEditor result"),
        }
    }
    
    /// Test environment variable expansion
    /// **Validates: Requirements 28.11**
    #[test]
    fn test_environment_variable_expansion() {
        let state = create_test_state_with_files();
        let expander = MacroExpander::new();
        
        // Set a test environment variable with unique name for this test
        std::env::set_var("TEST_VAR_CUSTOM_FUNC_ENV", "test_value");
        
        #[cfg(target_os = "windows")]
        let func = CustomFunction::new("test", "echo %TEST_VAR_CUSTOM_FUNC_ENV%");
        
        #[cfg(not(target_os = "windows"))]
        let func = CustomFunction::new("test", "echo $TEST_VAR_CUSTOM_FUNC_ENV");
        
        let result = expander.expand(&state, &func).unwrap();
        assert!(result.contains("test_value"));
        
        // Clean up
        std::env::remove_var("TEST_VAR_CUSTOM_FUNC_ENV");
    }
    
    /// Test user input macro detection
    /// **Validates: Requirements 28.5**
    #[test]
    fn test_user_input_macro_detection() {
        let expander = MacroExpander::new();
        
        let func_with_input = CustomFunction::new("test", "echo $I");
        assert!(expander.requires_user_input(&func_with_input));
        
        let func_without_input = CustomFunction::new("test", "echo $F");
        assert!(!expander.requires_user_input(&func_without_input));
    }
    
    /// Test user input macro expansion
    /// **Validates: Requirements 28.5**
    #[test]
    fn test_user_input_macro_expansion() {
        let state = create_test_state_with_files();
        let expander = MacroExpander::new();
        
        let func = CustomFunction::new("test", "echo $I");
        let result = expander.expand_with_user_input(&state, &func, "user_input");
        
        assert!(result.is_ok());
        assert!(result.unwrap().contains("user_input"));
    }
    
    /// Test custom function loading from JSON
    /// **Validates: Requirements 28.1**
    #[test]
    fn test_custom_function_loading() {
        use crate::model::dialog::load_custom_functions;
        use std::io::Write;
        use tempfile::NamedTempFile;
        
        // Create a temporary JSON file
        let mut temp_file = NamedTempFile::new().unwrap();
        let json = r#"[
            {
                "name": "Test Function",
                "command": "echo $F",
                "description": "Test description",
                "shell": "bash"
            }
        ]"#;
        temp_file.write_all(json.as_bytes()).unwrap();
        temp_file.flush().unwrap();
        
        // Load functions
        let functions = load_custom_functions(temp_file.path()).unwrap();
        
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "Test Function");
        assert_eq!(functions[0].command, "echo $F");
        assert_eq!(functions[0].description, Some("Test description".to_string()));
        assert_eq!(functions[0].shell, Some("bash".to_string()));
    }
    
    /// Test custom function selector filtering
    /// **Validates: Requirements 28.3**
    #[test]
    fn test_custom_function_selector_filtering() {
        use crate::model::dialog::CustomFunctionSelector;
        
        let functions = vec![
            CustomFunction::new("Copy File", "cp $F $O"),
            CustomFunction::new("Move File", "mv $F $O"),
            CustomFunction::new("Delete File", "rm $F"),
        ];
        
        let mut selector = CustomFunctionSelector::new(functions);
        
        // Test initial state
        assert_eq!(selector.filtered_count(), 3);
        
        // Test filtering by name
        selector.set_filter("Copy".to_string());
        assert_eq!(selector.filtered_count(), 1);
        assert_eq!(selector.selected_function().unwrap().name, "Copy File");
        
        // Test filtering with no matches
        selector.set_filter("NonExistent".to_string());
        assert_eq!(selector.filtered_count(), 0);
        assert!(selector.is_empty());
        
        // Test clearing filter
        selector.set_filter(String::new());
        assert_eq!(selector.filtered_count(), 3);
    }
}
