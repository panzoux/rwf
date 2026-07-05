//! Session log management
//!
//! This module provides session log management with:
//! - In-memory log buffer with configurable max lines
//! - Manual log saving to file with timestamps
//! - Automatic log saving on exit (configurable)
//! - Log file rotation
//! - Slow operation logging

use chrono::{DateTime, Local};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Session log manager
#[derive(Debug)]
pub struct LogManager {
    /// In-memory log entries
    entries: VecDeque<LogEntry>,
    /// Maximum lines to keep in memory
    max_lines: usize,
    /// Path where logs are saved
    log_path: PathBuf,
    /// Threshold for slow operation logging (in milliseconds)
    slow_op_threshold_ms: u64,
}

/// A single log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp when the entry was created
    pub timestamp: SystemTime,
    /// Log message
    pub message: String,
    /// Log level
    pub level: LogEntryLevel,
}

/// Log entry level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogEntryLevel {
    Info,
    Warning,
    Error,
}

impl LogManager {
    /// Create a new log manager
    pub fn new(max_lines: usize, log_path: PathBuf, slow_op_threshold_ms: u64) -> Self {
        Self {
            entries: VecDeque::new(),
            max_lines,
            log_path,
            slow_op_threshold_ms,
        }
    }

    /// Return the path of the log file
    pub fn log_path(&self) -> &std::path::Path {
        &self.log_path
    }

    /// Add a log entry
    pub fn log(&mut self, level: LogEntryLevel, message: String) {
        let entry = LogEntry {
            timestamp: SystemTime::now(),
            message,
            level,
        };

        self.entries.push_back(entry);

        // Flush oldest entries if we exceed max lines
        while self.entries.len() > self.max_lines {
            self.entries.pop_front();
        }
    }

    /// Add a log entry and flush to file if memory limit is reached
    pub fn log_with_auto_flush(&mut self, level: LogEntryLevel, message: String) -> io::Result<()> {
        let was_at_limit = self.entries.len() >= self.max_lines;

        self.log(level, message);

        // If we were at the limit, flush to file
        if was_at_limit {
            self.save_to_file()?;
        }

        Ok(())
    }

    /// Log an info message
    pub fn info(&mut self, message: String) {
        self.log(LogEntryLevel::Info, message);
    }

    /// Log a warning message
    pub fn warn(&mut self, message: String) {
        self.log(LogEntryLevel::Warning, message);
    }

    /// Log an error message
    pub fn error(&mut self, message: String) {
        self.log(LogEntryLevel::Error, message);
    }

    /// Log a file operation if it exceeds the slow threshold
    pub fn log_operation_if_slow(&mut self, operation: &str, duration: Duration, file_path: &str) {
        let duration_ms = duration.as_millis() as u64;
        if duration_ms >= self.slow_op_threshold_ms {
            let message = format!(
                "Slow operation: {} took {}ms for file: {}",
                operation, duration_ms, file_path
            );
            self.warn(message);
        }
    }

    /// Get all log entries
    pub fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the log is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Save the current session log to file
    pub fn save_to_file(&self) -> io::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Rotate existing log file if it exists
        self.rotate_log_file()?;

        // Create new log file
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.log_path)?;

        // Write header
        writeln!(
            file,
            "Session Log - {}",
            format_timestamp(SystemTime::now())
        )?;
        writeln!(file, "=")?;
        writeln!(file)?;

        // Write all entries
        for entry in &self.entries {
            let level_str = match entry.level {
                LogEntryLevel::Info => "INFO",
                LogEntryLevel::Warning => "WARN",
                LogEntryLevel::Error => "ERROR",
            };

            writeln!(
                file,
                "[{}] {}: {}",
                format_timestamp(entry.timestamp),
                level_str,
                entry.message
            )?;
        }

        file.flush()?;
        Ok(())
    }

    /// Save log on exit if configured to do so
    pub fn save_on_exit_if_configured(&self, save_on_exit: bool) -> io::Result<()> {
        if save_on_exit && !self.is_empty() {
            self.save_to_file()?;
        }
        Ok(())
    }

    /// Rotate log file by renaming it with a timestamp
    fn rotate_log_file(&self) -> io::Result<()> {
        if !self.log_path.exists() {
            return Ok(());
        }

        // Generate timestamped filename
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let file_stem = self
            .log_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session");
        let extension = self
            .log_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("log");

        let rotated_name = format!("{}_{}.{}", file_stem, timestamp, extension);
        let rotated_path = self.log_path.with_file_name(rotated_name);

        // Rename existing file
        fs::rename(&self.log_path, &rotated_path)?;

        Ok(())
    }

    /// Clear all log entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Format a SystemTime as a human-readable timestamp
fn format_timestamp(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tempfile::TempDir;

    #[test]
    fn test_log_manager_basic() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let mut manager = LogManager::new(100, log_path, 5000);

        manager.info("Test info message".to_string());
        manager.warn("Test warning message".to_string());
        manager.error("Test error message".to_string());

        assert_eq!(manager.len(), 3);
        assert_eq!(manager.entries()[0].level, LogEntryLevel::Info);
        assert_eq!(manager.entries()[1].level, LogEntryLevel::Warning);
        assert_eq!(manager.entries()[2].level, LogEntryLevel::Error);
    }

    #[test]
    fn test_log_manager_max_lines() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let mut manager = LogManager::new(5, log_path, 5000);

        for i in 0..10 {
            manager.info(format!("Message {}", i));
        }

        assert_eq!(manager.len(), 5);
        // Should have messages 5-9
        assert!(manager.entries()[0].message.contains("Message 5"));
        assert!(manager.entries()[4].message.contains("Message 9"));
    }

    #[test]
    fn test_save_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let mut manager = LogManager::new(100, log_path.clone(), 5000);

        manager.info("First message".to_string());
        manager.warn("Second message".to_string());
        manager.error("Third message".to_string());

        manager.save_to_file().unwrap();

        // Verify file was created
        assert!(log_path.exists());

        // Read and verify content
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Session Log"));
        assert!(content.contains("INFO: First message"));
        assert!(content.contains("WARN: Second message"));
        assert!(content.contains("ERROR: Third message"));
    }

    #[test]
    fn test_log_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let mut manager = LogManager::new(100, log_path.clone(), 5000);

        // Create first log
        manager.info("First log".to_string());
        manager.save_to_file().unwrap();
        assert!(log_path.exists());

        // Wait a bit to ensure different timestamp
        thread::sleep(Duration::from_millis(1100));

        // Create second log
        manager.clear();
        manager.info("Second log".to_string());
        manager.save_to_file().unwrap();

        // Verify both files exist
        assert!(log_path.exists());
        let rotated_files: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("log"))
            .collect();

        assert_eq!(rotated_files.len(), 2);
    }

    #[test]
    fn test_slow_operation_logging() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let mut manager = LogManager::new(100, log_path, 5000);

        // Fast operation - should not log
        manager.log_operation_if_slow("copy", Duration::from_millis(1000), "/path/to/file");
        assert_eq!(manager.len(), 0);

        // Slow operation - should log
        manager.log_operation_if_slow("copy", Duration::from_millis(6000), "/path/to/file");
        assert_eq!(manager.len(), 1);
        assert!(manager.entries()[0].message.contains("Slow operation"));
        assert!(manager.entries()[0].message.contains("6000ms"));
    }

    #[test]
    fn test_clear() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let mut manager = LogManager::new(100, log_path, 5000);

        manager.info("Test message".to_string());
        assert_eq!(manager.len(), 1);

        manager.clear();
        assert_eq!(manager.len(), 0);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_log_with_auto_flush() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let mut manager = LogManager::new(5, log_path.clone(), 5000);

        // Fill to capacity
        for i in 0..5 {
            manager.info(format!("Message {}", i));
        }
        assert_eq!(manager.len(), 5);
        assert!(!log_path.exists());

        // Add one more with auto-flush - should trigger flush
        manager
            .log_with_auto_flush(LogEntryLevel::Info, "Message 5".to_string())
            .unwrap();

        // File should now exist
        assert!(log_path.exists());

        // Should still have 5 entries in memory
        assert_eq!(manager.len(), 5);
    }

    #[test]
    fn test_save_on_exit_if_configured() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let mut manager = LogManager::new(100, log_path.clone(), 5000);

        manager.info("Test message".to_string());

        // Test with save_on_exit = true
        manager.save_on_exit_if_configured(true).unwrap();
        assert!(log_path.exists());

        // Remove the file
        fs::remove_file(&log_path).unwrap();

        // Test with save_on_exit = false
        manager.save_on_exit_if_configured(false).unwrap();
        assert!(!log_path.exists());
    }

    #[test]
    fn test_save_on_exit_empty_log() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("session.log");

        let manager = LogManager::new(100, log_path.clone(), 5000);

        // Empty log should not create file even with save_on_exit = true
        manager.save_on_exit_if_configured(true).unwrap();
        assert!(!log_path.exists());
    }
}
