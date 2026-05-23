//! File entry representation

use std::path::Path;
use std::time::{SystemTime, Duration};
use super::Location;

/// Represents a file or directory with metadata
#[derive(Debug, Clone, PartialEq)]
pub struct FileEntry {
    pub name: String,
    pub location: Location,
    pub size: u64,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub modified: SystemTime,
    pub marked: bool,
    pub calculated_size: Option<u64>,
}

impl FileEntry {
    /// Get file extension
    pub fn extension(&self) -> Option<&str> {
        Path::new(&self.name)
            .extension()
            .and_then(|s| s.to_str())
    }
    
    /// Get file name without extension
    pub fn name_without_extension(&self) -> &str {
        Path::new(&self.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&self.name)
    }

    /// Create a dummy file entry for testing
    #[cfg(test)]
    pub fn dummy(name: &str) -> Self {
        Self {
            name: name.to_string(),
            location: Location::Local(std::path::PathBuf::from(name)),
            size: 0,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        }
    }

    /// Get formatted size string
    pub fn formatted_size(&self) -> String {
        format_size(self.calculated_size.unwrap_or(self.size))
    }
    
    /// Get formatted date string
    pub fn formatted_date(&self) -> String {
        format_date(self.modified)
    }
}

/// Format file size in human-readable format
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;
    
    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes < TB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    }
}

fn format_date(time: SystemTime) -> String {
    use std::time::Duration;
    
    let now = SystemTime::now();
    
    // Calculate time difference
    let duration_since = now.duration_since(time).unwrap_or(Duration::from_secs(0));
    let seconds = duration_since.as_secs();
    
    // Convert SystemTime to a formatted string
    // For simplicity, we'll use a basic implementation
    // In a real application, you'd use chrono or time crate
    
    // Check if it's today (within last 24 hours)
    if seconds < 86400 {
        // Today - show "Today HH:MM"
        format_time_only(time, "Today")
    } else if seconds < 172800 {
        // Yesterday (24-48 hours ago)
        format_time_only(time, "Yesterday")
    } else {
        // Older - show full date in YYYY-MM-DD HH:MM format
        format_full_date(time)
    }
}

fn format_time_only(time: SystemTime, prefix: &str) -> String {
    // Extract hours and minutes from SystemTime
    // This is a simplified implementation
    let duration = time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    let total_seconds = duration.as_secs();
    let hours = (total_seconds / 3600) % 24;
    let minutes = (total_seconds / 60) % 60;
    
    format!("{} {:02}:{:02}", prefix, hours, minutes)
}

fn format_full_date(time: SystemTime) -> String {
    // Convert SystemTime to YYYY-MM-DD HH:MM format
    let duration = time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    let total_seconds = duration.as_secs();
    
    // Calculate date components (simplified, doesn't account for leap years perfectly)
    let days_since_epoch = total_seconds / 86400;
    let year = 1970 + (days_since_epoch / 365);
    let day_of_year = days_since_epoch % 365;
    
    // Simplified month calculation (assumes 30 days per month for simplicity)
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;
    
    let hours = (total_seconds / 3600) % 24;
    let minutes = (total_seconds / 60) % 60;
    
    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hours, minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_extension() {
        let entry = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        assert_eq!(entry.extension(), Some("txt"));
    }
    
    #[test]
    fn test_extension_none() {
        let entry = FileEntry {
            name: "test".to_string(),
            location: Location::Local(PathBuf::from("/test")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        assert_eq!(entry.extension(), None);
    }
    
    #[test]
    fn test_name_without_extension() {
        let entry = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        assert_eq!(entry.name_without_extension(), "test");
    }
    
    #[test]
    fn test_formatted_size_bytes() {
        let entry = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test.txt")),
            size: 512,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        assert_eq!(entry.formatted_size(), "512 B");
    }
    
    #[test]
    fn test_formatted_size_kb() {
        let entry = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test.txt")),
            size: 2048,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        assert_eq!(entry.formatted_size(), "2.00 KB");
    }
    
    #[test]
    fn test_formatted_size_mb() {
        let entry = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test.txt")),
            size: 5_242_880, // 5 MB
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        assert_eq!(entry.formatted_size(), "5.00 MB");
    }
    
    #[test]
    fn test_formatted_size_gb() {
        let entry = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test.txt")),
            size: 3_221_225_472, // 3 GB
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        assert_eq!(entry.formatted_size(), "3.00 GB");
    }
    
    #[test]
    fn test_formatted_size_uses_calculated_size() {
        let entry = FileEntry {
            name: "test_dir".to_string(),
            location: Location::Local(PathBuf::from("/test_dir")),
            size: 0,
            is_dir: true,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: Some(10_485_760), // 10 MB
        };
        
        assert_eq!(entry.formatted_size(), "10.00 MB");
    }
    
    #[test]
    fn test_formatted_date_returns_string() {
        let entry = FileEntry {
            name: "test.txt".to_string(),
            location: Location::Local(PathBuf::from("/test.txt")),
            size: 100,
            is_dir: false,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        };
        
        let date_str = entry.formatted_date();
        // Just verify it returns a non-empty string
        assert!(!date_str.is_empty());
    }
}
