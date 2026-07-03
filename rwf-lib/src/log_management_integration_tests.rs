//! Integration tests for log management
//!
//! Tests Requirements 44.1-44.7:
//! - Manual log saving
//! - Log rotation
//! - Slow operation logging
//! - Log on exit
//! - Memory management

#[cfg(test)]
mod tests {
    use crate::state::{AppState, update_state, Transition};
    use crate::config::AppConfig;
    use crate::log_manager::LogEntryLevel;
    use tempfile::TempDir;
    use std::time::Duration;

    fn create_test_state_with_log_config(temp_dir: &TempDir) -> AppState {
        let config = AppConfig {
            max_log_lines_in_memory: 10,
            log_save_path: temp_dir.path().join("session.log").to_string_lossy().to_string(),
            save_log_on_exit: true,
            log_file_progress_threshold_ms: 100, // Low threshold for testing
            ..Default::default()
        };

        AppState::new(config)
    }

    #[test]
    fn test_manual_log_saving() {
        // **Validates: Requirements 44.1, 44.2, 44.3**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        // Add some log entries
        state.log_manager.info("Test info message".to_string());
        state.log_manager.warn("Test warning message".to_string());
        state.log_manager.error("Test error message".to_string());
        
        assert_eq!(state.log_manager.len(), 3);
        
        // Trigger manual save via SaveLog transition
        let result = update_state(&mut state, Transition::SaveLog);
        assert!(result.ui_changed);
        
        // Verify log file was created
        let log_path = temp_dir.path().join("session.log");
        assert!(log_path.exists());
        
        // Verify log file contains entries with timestamps
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Session Log"));
        assert!(content.contains("INFO: Test info message"));
        assert!(content.contains("WARN: Test warning message"));
        assert!(content.contains("ERROR: Test error message"));
        
        // Verify timestamps are present (format: [YYYY-MM-DD HH:MM:SS])
        assert!(content.contains("[20")); // Year starts with 20
        assert!(content.contains(":")); // Time separator
    }

    #[test]
    fn test_log_memory_management() {
        // **Validates: Requirements 44.4**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        // Max is 10, add 15 entries
        for i in 0..15 {
            state.log_manager.info(format!("Message {}", i));
        }
        
        // Should only have 10 entries (oldest dropped)
        assert_eq!(state.log_manager.len(), 10);
        
        // Should have messages 5-14
        let entries = state.log_manager.entries();
        assert!(entries[0].message.contains("Message 5"));
        assert!(entries[9].message.contains("Message 14"));
    }

    #[test]
    fn test_log_memory_auto_flush() {
        // **Validates: Requirements 44.4**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        // Fill to capacity
        for i in 0..10 {
            state.log_manager.info(format!("Message {}", i));
        }
        
        let log_path = temp_dir.path().join("session.log");
        assert!(!log_path.exists());
        
        // Add one more with auto-flush
        state.log_manager.log_with_auto_flush(
            LogEntryLevel::Info,
            "Message 10".to_string()
        ).unwrap();
        
        // File should now exist
        assert!(log_path.exists());
    }

    #[test]
    fn test_log_on_exit_enabled() {
        // **Validates: Requirements 44.5**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        state.log_manager.info("Exit test message".to_string());
        
        // Simulate exit with save_on_exit = true
        state.log_manager.save_on_exit_if_configured(state.config.save_log_on_exit).unwrap();
        
        let log_path = temp_dir.path().join("session.log");
        assert!(log_path.exists());
        
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Exit test message"));
    }

    #[test]
    fn test_log_on_exit_disabled() {
        // **Validates: Requirements 44.5**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        state.config.save_log_on_exit = false;
        
        state.log_manager.info("Exit test message".to_string());
        
        // Simulate exit with save_on_exit = false
        state.log_manager.save_on_exit_if_configured(state.config.save_log_on_exit).unwrap();
        
        let log_path = temp_dir.path().join("session.log");
        assert!(!log_path.exists());
    }

    #[test]
    fn test_log_rotation() {
        // **Validates: Requirements 44.6**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        // Create first log
        state.log_manager.info("First log".to_string());
        update_state(&mut state, Transition::SaveLog);
        
        let log_path = temp_dir.path().join("session.log");
        assert!(log_path.exists());
        
        // Wait a bit to ensure different timestamp
        std::thread::sleep(Duration::from_millis(1100));
        
        // Create second log
        state.log_manager.clear();
        state.log_manager.info("Second log".to_string());
        update_state(&mut state, Transition::SaveLog);
        
        // Both files should exist (original rotated, new created)
        assert!(log_path.exists());
        
        let rotated_files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().and_then(|s| s.to_str()) == Some("log")
            })
            .collect();
        
        assert_eq!(rotated_files.len(), 2);
        
        // Verify current log has second message
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Second log"));
        assert!(!content.contains("First log"));
    }

    #[test]
    fn test_slow_operation_logging() {
        // **Validates: Requirements 44.7**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        // Fast operation - should not log
        state.log_manager.log_operation_if_slow(
            "copy",
            Duration::from_millis(50),
            "/path/to/file1.txt"
        );
        assert_eq!(state.log_manager.len(), 0);
        
        // Slow operation - should log (threshold is 100ms)
        state.log_manager.log_operation_if_slow(
            "copy",
            Duration::from_millis(150),
            "/path/to/file2.txt"
        );
        assert_eq!(state.log_manager.len(), 1);
        
        let entries = state.log_manager.entries();
        assert!(entries[0].message.contains("Slow operation"));
        assert!(entries[0].message.contains("copy"));
        assert!(entries[0].message.contains("150ms"));
        assert!(entries[0].message.contains("/path/to/file2.txt"));
        assert_eq!(entries[0].level, LogEntryLevel::Warning);
    }

    #[test]
    fn test_log_save_with_empty_log() {
        // **Validates: Requirements 44.1**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        // Try to save empty log
        let result = update_state(&mut state, Transition::SaveLog);
        assert!(result.ui_changed);
        
        // File should still be created (even if empty)
        let log_path = temp_dir.path().join("session.log");
        assert!(log_path.exists());
    }

    #[test]
    fn test_log_path_configuration() {
        // **Validates: Requirements 44.2**
        let temp_dir = TempDir::new().unwrap();
        let custom_path = temp_dir.path().join("custom").join("my_session.log");
        
        let config = AppConfig {
            log_save_path: custom_path.to_string_lossy().to_string(),
            ..Default::default()
        };

        let mut state = AppState::new(config);
        state.log_manager.info("Test message".to_string());
        
        update_state(&mut state, Transition::SaveLog);
        
        // Verify custom path was used
        assert!(custom_path.exists());
    }

    #[test]
    fn test_log_entries_have_timestamps() {
        // **Validates: Requirements 44.3**
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        state.log_manager.info("Message 1".to_string());
        std::thread::sleep(Duration::from_millis(10));
        state.log_manager.info("Message 2".to_string());
        
        let entries = state.log_manager.entries();
        assert_eq!(entries.len(), 2);
        
        // Verify timestamps exist and are different
        let time1 = entries[0].timestamp;
        let time2 = entries[1].timestamp;
        assert!(time2 > time1);
    }

    #[test]
    fn test_log_manager_integration_with_state() {
        // Integration test: verify log manager is properly integrated with AppState
        let temp_dir = TempDir::new().unwrap();
        let mut state = create_test_state_with_log_config(&temp_dir);
        
        // Log manager should be accessible from state
        state.log_manager.info("Integration test".to_string());
        assert_eq!(state.log_manager.len(), 1);
        
        // SaveLog transition should work
        let result = update_state(&mut state, Transition::SaveLog);
        assert!(result.ui_changed);
        
        // Verify log was saved
        let log_path = temp_dir.path().join("session.log");
        assert!(log_path.exists());
    }
}
