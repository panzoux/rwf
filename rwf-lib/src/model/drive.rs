//! Drive listing types (moved down from `dialog` so `volume_info`/`job` need not depend on it).

/// Drive information for drive selection dialog
#[derive(Debug, Clone, PartialEq)]
pub struct DriveInfo {
    pub path: String,
    pub label: String,
    pub drive_type: DriveType,
    pub total_space: Option<u64>,
    pub free_space: Option<u64>,
}

impl DriveInfo {
    /// Returns the display string shown in the drive selection list.
    pub fn display_label(&self) -> String {
        // Home entry: label starts with '~'
        if self.label.starts_with('~') {
            return self.label.clone();
        }
        // Network share: show path
        if self.drive_type == DriveType::Network {
            return self.path.clone();
        }
        let type_str = match self.drive_type {
            DriveType::Local => "Local",
            DriveType::Removable => "Removable",
            _ => "Unknown",
        };
        let path_trimmed = self.path.trim_end_matches(['/', '\\']);
        if self.label.is_empty() {
            format!("{} ({})", path_trimmed, type_str)
        } else {
            format!("{} - {} ({})", path_trimmed, self.label, type_str)
        }
    }
}

/// Drive type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveType {
    Local,
    Network,
    Removable,
    Unknown,
}
