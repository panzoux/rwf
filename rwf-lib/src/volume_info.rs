//! Volume information extraction
//!
//! Platform-specific logic to extract volume names, labels, and mount points
//! **Validates: Requirements 39A.1-39A.14**

use crate::model::{FileEntry, Location};
use crate::model::marking::MarkingModel;
use std::path::Path;

/// Volume information
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct VolumeInfo {
    pub display_name: String,
    pub volume_type: VolumeType,
}

/// Type of volume
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeType {
    Local,
    Network,
    Removable,
    Unknown,
}

/// Marked file statistics
#[derive(Debug, Clone, Default)]
pub struct MarkedFileStats {
    pub dir_count: usize,
    pub file_count: usize,
    pub total_size: u64,
}

/// Get drive or share name for a location
/// **Validates: Requirements 39A.2-39A.8**
pub fn get_drive_or_share_name(location: &Location) -> String {
    match location {
        Location::Local(path) => {
            #[cfg(target_os = "windows")]
            {
                get_windows_volume_name(path)
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                get_unix_volume_name(path)
            }
            #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
            {
                path.display().to_string()
            }
        }
        Location::Ssh { host, .. } => {
            format!("\\\\{}", host)
        }
        Location::Cloud { provider, bucket, .. } => {
            format!("{}://{}", provider, bucket)
        }
        Location::Archive { archive_path, .. } => {
            format!("Archive: {}", get_drive_or_share_name(archive_path))
        }
    }
}

/// Calculate marked file statistics
/// **Validates: Requirements 39A.9-39A.12**
pub fn calculate_marked_stats(
    entries: &[FileEntry],
    marking: &MarkingModel,
) -> MarkedFileStats {
    let mut dir_count = 0;
    let mut file_count = 0;
    let mut total_size = 0;
    
    for entry in entries {
        if marking.is_marked(&entry.location) {
            if entry.is_dir {
                dir_count += 1;
            } else {
                file_count += 1;
            }
            total_size += entry.calculated_size.unwrap_or(entry.size);
        }
    }
    
    MarkedFileStats {
        dir_count,
        file_count,
        total_size,
    }
}

/// Format top separator info with volume name and marked stats
/// **Validates: Requirements 39A.9-39A.13**
pub fn format_top_separator_info(
    volume_name: &str,
    marked_stats: &MarkedFileStats,
) -> String {
    if marked_stats.dir_count == 0 && marked_stats.file_count == 0 {
        // No marked files
        return volume_name.to_string();
    }
    
    let mut parts = Vec::new();
    
    // Add directory count if any
    if marked_stats.dir_count > 0 {
        if marked_stats.dir_count == 1 {
            parts.push("1 Dir".to_string());
        } else {
            parts.push(format!("{} Dirs", marked_stats.dir_count));
        }
    }
    
    // Add file count if any
    if marked_stats.file_count > 0 {
        if marked_stats.file_count == 1 {
            parts.push("1 File".to_string());
        } else {
            parts.push(format!("{} Files", marked_stats.file_count));
        }
    }
    
    // Add total size
    let size_str = format_size(marked_stats.total_size);
    
    // Combine parts
    let marked_info = if parts.is_empty() {
        format!("{} marked", size_str)
    } else {
        format!("{} {} marked", parts.join(" "), size_str)
    };
    
    format!("{} {}", volume_name, marked_info)
}

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
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

// Platform-specific implementations

#[cfg(target_os = "windows")]
fn get_windows_volume_name(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    
    // Check for network path (\\server\share)
    if path_str.starts_with("\\\\") {
        if let Some(server_end) = path_str[2..].find('\\') {
            let server = &path_str[2..2 + server_end];
            return format!("\\\\{}", server);
        }
        return path_str.to_string();
    }
    
    // Extract drive letter for local paths
    if let Some(drive_letter) = path_str.chars().next() {
        if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
            let drive_root = format!("{}:\\", drive_letter);
            
            // Try to get volume label
            if let Some(label) = get_volume_label_windows(&drive_root) {
                if !label.is_empty() {
                    return label;
                }
            }
            
            // Fallback to drive letter in parentheses
            return format!("({}:)", drive_letter);
        }
    }
    
    path_str.to_string()
}

#[cfg(target_os = "windows")]
fn get_volume_label_windows(drive_root: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetVolumeInformationW;
    
    // Convert drive root to wide string (UTF-16)
    let mut wide_path: Vec<u16> = drive_root.encode_utf16().collect();
    wide_path.push(0); // Null terminator
    
    // Buffer for volume name (max 256 characters)
    let mut volume_name_buffer = vec![0u16; 256];
    let mut serial_number = 0u32;
    let mut max_component_length = 0u32;
    let mut file_system_flags = 0u32;
    let mut file_system_name_buffer = vec![0u16; 256];
    
    // Call Windows API
    unsafe {
        let result = GetVolumeInformationW(
            PCWSTR(wide_path.as_ptr()),
            Some(&mut volume_name_buffer),
            Some(&mut serial_number),
            Some(&mut max_component_length),
            Some(&mut file_system_flags),
            Some(&mut file_system_name_buffer),
        );
        
        if result.is_ok() {
            // Find the null terminator
            let len = volume_name_buffer.iter()
                .position(|&c| c == 0)
                .unwrap_or(volume_name_buffer.len());
            
            // Convert from UTF-16 to String
            let volume_name = String::from_utf16_lossy(&volume_name_buffer[..len]);
            
            // Return Some only if the label is not empty
            if !volume_name.is_empty() {
                return Some(volume_name);
            }
        }
    }
    
    // Return None on failure or empty label (triggers fallback)
    None
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn get_unix_volume_name(path: &Path) -> String {
    use std::fs;
    
    // Special case: root path
    if path == Path::new("/") {
        return "Root".to_string();
    }
    
    // Read /proc/mounts (Linux) or /etc/mtab (Unix-like)
    #[cfg(target_os = "linux")]
    let mounts_path = "/proc/mounts";
    #[cfg(target_os = "macos")]
    let mounts_path = "/etc/fstab";
    
    if let Ok(mounts_content) = fs::read_to_string(mounts_path) {
        // Find the mount point that matches this path
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        
        let mut best_match: Option<(String, String, String)> = None; // (device, mount_point, label)
        let mut best_match_len = 0;
        
        for line in mounts_content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let device = parts[0];
                let mount_point = parts[1];
                
                // Check if this mount point is a prefix of our path
                if canonical_path.starts_with(mount_point) {
                    let match_len = mount_point.len();
                    if match_len > best_match_len {
                        // Try to get volume label
                        let label = get_volume_label_unix(device);
                        best_match = Some((
                            device.to_string(),
                            mount_point.to_string(),
                            label.unwrap_or_default(),
                        ));
                        best_match_len = match_len;
                    }
                }
            }
        }
        
        if let Some((device, mount_point, label)) = best_match {
            return format_unix_volume_info(&device, &mount_point, &label);
        }
    }
    
    // Fallback to path display
    path.display().to_string()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn get_volume_label_unix(_device: &str) -> Option<String> {
    // Simplified version - full implementation would use blkid or diskutil
    None
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn format_unix_volume_info(device: &str, mount_point: &str, label: &str) -> String {
    if mount_point == "/" {
        if label.is_empty() {
            return "Root".to_string();
        } else {
            return format!("{} (Root - {})", device, label);
        }
    }
    
    if label.is_empty() {
        format!("{} ({})", device, mount_point)
    } else {
        format!("{} ({} - {})", device, mount_point, label)
    }
}
