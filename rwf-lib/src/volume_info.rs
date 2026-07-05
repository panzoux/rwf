//! Volume information extraction
//!
//! Platform-specific logic to extract volume names, labels, and mount points
//! **Validates: Requirements 39A.1-39A.14**

// Win32 volume APIs require raw FFI. This is the only module in the workspace
// allowed to use unsafe; every unsafe block must carry a SAFETY comment.
#![allow(unsafe_code)]

use crate::model::marking::MarkingModel;
use crate::model::{FileEntry, Location};
use std::path::Path;

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
        Location::Cloud {
            provider, bucket, ..
        } => {
            format!("{}://{}", provider, bucket)
        }
        Location::Archive { archive_path, .. } => {
            format!("Archive: {}", get_drive_or_share_name(archive_path))
        }
    }
}

/// Calculate marked file statistics
/// **Validates: Requirements 39A.9-39A.12**
pub fn calculate_marked_stats(entries: &[FileEntry], marking: &MarkingModel) -> MarkedFileStats {
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
pub fn format_top_separator_info(volume_name: &str, marked_stats: &MarkedFileStats) -> String {
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
    if let Some(stripped) = path_str.strip_prefix("\\\\") {
        if let Some(server_end) = stripped.find('\\') {
            let server = &stripped[..server_end];
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

    // SAFETY: wide_path is null-terminated and outlives the call; all out-buffers
    // are live local slices/values passed as Option<&mut _> per the windows crate API.
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
            let len = volume_name_buffer
                .iter()
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

/// Get all available drives and network shares
/// **Validates: Requirements 42.4, 42.6**
pub fn get_all_drives() -> Vec<crate::model::dialog::DriveInfo> {
    #[cfg(target_os = "windows")]
    {
        get_windows_drives()
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        get_unix_drives()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "windows")]
fn get_windows_drives() -> Vec<crate::model::dialog::DriveInfo> {
    use crate::model::dialog::{DriveInfo, DriveType};
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
    };

    let mut drives = Vec::new();

    // Get logical drive bitmask
    // SAFETY: GetLogicalDrives takes no arguments and only returns a bitmask.
    let drive_mask = unsafe { GetLogicalDrives() };

    // Iterate through all possible drive letters (A-Z)
    for i in 0..26 {
        if (drive_mask & (1 << i)) != 0 {
            let drive_letter = (b'A' + i) as char;
            let drive_path = format!("{}:\\", drive_letter);

            // Convert to wide string
            let mut wide_path: Vec<u16> = drive_path.encode_utf16().collect();
            wide_path.push(0);

            // Get drive type
            // SAFETY: wide_path is null-terminated and outlives the call.
            let drive_type_raw = unsafe { GetDriveTypeW(PCWSTR(wide_path.as_ptr())) };
            let drive_type = match drive_type_raw {
                2 => DriveType::Removable, // DRIVE_REMOVABLE
                3 => DriveType::Local,     // DRIVE_FIXED
                4 => DriveType::Network,   // DRIVE_REMOTE
                _ => DriveType::Unknown,
            };

            // Get volume label
            let label = get_volume_label_windows(&drive_path)
                .unwrap_or_else(|| format!("({}:)", drive_letter));

            // Get disk space information
            let mut free_bytes = 0u64;
            let mut total_bytes = 0u64;
            let mut total_free_bytes = 0u64;

            // SAFETY: wide_path is null-terminated and outlives the call; the three
            // out-pointers reference live local u64s.
            let space_result = unsafe {
                GetDiskFreeSpaceExW(
                    PCWSTR(wide_path.as_ptr()),
                    Some(&mut free_bytes),
                    Some(&mut total_bytes),
                    Some(&mut total_free_bytes),
                )
            };

            let (total_space, free_space) = if space_result.is_ok() {
                (Some(total_bytes), Some(free_bytes))
            } else {
                (None, None)
            };

            drives.push(DriveInfo {
                path: drive_path,
                label,
                drive_type,
                total_space,
                free_space,
            });
        }
    }

    drives
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn get_unix_drives() -> Vec<crate::model::dialog::DriveInfo> {
    use crate::model::dialog::{DriveInfo, DriveType};
    use std::fs;

    let mut drives = Vec::new();

    // Read mount points
    #[cfg(target_os = "linux")]
    let mounts_path = "/proc/mounts";
    #[cfg(target_os = "macos")]
    let mounts_path = "/etc/fstab";

    if let Ok(mounts_content) = fs::read_to_string(mounts_path) {
        for line in mounts_content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let device = parts[0];
                let mount_point = parts[1];

                // Skip special filesystems
                if device.starts_with("/dev/") || mount_point == "/" {
                    let drive_type = if device.contains("nfs") || device.contains("cifs") {
                        DriveType::Network
                    } else if device.contains("usb") || device.contains("sd") {
                        DriveType::Removable
                    } else {
                        DriveType::Local
                    };

                    let label =
                        get_volume_label_unix(device).unwrap_or_else(|| mount_point.to_string());

                    // Try to get disk space
                    let (total_space, free_space) = get_unix_disk_space(mount_point);

                    drives.push(DriveInfo {
                        path: mount_point.to_string(),
                        label,
                        drive_type,
                        total_space,
                        free_space,
                    });
                }
            }
        }
    }

    // Always include root if not already present
    if !drives.iter().any(|d| d.path == "/") {
        let (total_space, free_space) = get_unix_disk_space("/");
        drives.push(DriveInfo {
            path: "/".to_string(),
            label: "Root".to_string(),
            drive_type: DriveType::Local,
            total_space,
            free_space,
        });
    }

    drives
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn get_unix_disk_space(path: &str) -> (Option<u64>, Option<u64>) {
    use std::ffi::CString;
    use std::mem;

    #[repr(C)]
    struct StatVfs {
        f_bsize: u64,
        f_frsize: u64,
        f_blocks: u64,
        f_bfree: u64,
        f_bavail: u64,
        f_files: u64,
        f_ffree: u64,
        f_favail: u64,
        f_fsid: u64,
        f_flag: u64,
        f_namemax: u64,
    }

    extern "C" {
        fn statvfs(path: *const i8, buf: *mut StatVfs) -> i32;
    }

    let c_path = match CString::new(path) {
        Ok(p) => p,
        Err(_) => return (None, None),
    };

    let mut stat: StatVfs = unsafe { mem::zeroed() };
    let result = unsafe { statvfs(c_path.as_ptr(), &mut stat) };

    if result == 0 {
        let total = stat.f_blocks * stat.f_frsize;
        let free = stat.f_bavail * stat.f_frsize;
        (Some(total), Some(free))
    } else {
        (None, None)
    }
}
