//! PipeToAction directive handling
//!
//! This module provides utilities for handling PipeToAction directives
//! after custom function execution completes.

use crate::job::PipeToAction;
use crate::model::Location;
use std::path::PathBuf;

/// Process PipeToAction directive with command output
pub fn process_pipe_to_action(
    action: &PipeToAction,
    output: &str,
) -> Result<PipeToActionResult, String> {
    match action {
        PipeToAction::JumpToPath => {
            // Parse the output as a path
            let path_str = output.trim();
            if path_str.is_empty() {
                return Err("Empty path returned from command".to_string());
            }
            
            let path = PathBuf::from(path_str);
            if !path.exists() {
                return Err(format!("Path does not exist: {}", path_str));
            }
            
            Ok(PipeToActionResult::JumpToPath(Location::Local(path)))
        }
        PipeToAction::ExecuteFile => {
            // Parse the output as a file path to execute
            let file_str = output.trim();
            if file_str.is_empty() {
                return Err("Empty file path returned from command".to_string());
            }
            
            let file_path = PathBuf::from(file_str);
            if !file_path.exists() {
                return Err(format!("File does not exist: {}", file_str));
            }
            
            Ok(PipeToActionResult::ExecuteFile(file_path))
        }
        PipeToAction::ExecuteFileWithEditor => {
            // Parse the output as a file path to open in editor
            let file_str = output.trim();
            if file_str.is_empty() {
                return Err("Empty file path returned from command".to_string());
            }
            
            let file_path = PathBuf::from(file_str);
            // File doesn't need to exist for editor (can create new file)
            
            Ok(PipeToActionResult::ExecuteFileWithEditor(file_path))
        }
    }
}

/// Result of processing a PipeToAction directive
#[derive(Debug, Clone)]
pub enum PipeToActionResult {
    /// Navigate to the specified path
    JumpToPath(Location),
    /// Execute the specified file
    ExecuteFile(PathBuf),
    /// Open the specified file in the configured editor
    ExecuteFileWithEditor(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_jump_to_path() {
        // Test with current directory (should exist)
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
    
    #[test]
    fn test_jump_to_path_nonexistent() {
        let output = "/nonexistent/path/that/does/not/exist";
        let result = process_pipe_to_action(&PipeToAction::JumpToPath, output);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_execute_file_with_editor() {
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
    
    #[test]
    fn test_empty_output() {
        let result = process_pipe_to_action(&PipeToAction::JumpToPath, "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty path"));
    }
}
