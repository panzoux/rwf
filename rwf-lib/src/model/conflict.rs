//! File-conflict pair (moved down from `dialog` so `job`/`worker_pool` need not depend on it).

/// A single file conflict between source and destination
#[derive(Debug, Clone)]
pub struct ConflictPair {
    pub source: crate::model::FileEntry,
    pub dest: crate::model::FileEntry,
    pub source_path: crate::model::Location,
    pub dest_path: crate::model::Location,
    pub is_directory: bool,
}

impl ConflictPair {
    /// Determine the conflict status for display
    pub fn get_status(&self) -> ConflictStatus {
        if self.is_directory {
            ConflictStatus::DirectoryMerge
        } else if self.source.size == self.dest.size {
            if self.source.modified == self.dest.modified {
                ConflictStatus::SameSizeDate
            } else if self.source.modified > self.dest.modified {
                ConflictStatus::SameSizeSourceNewer
            } else {
                ConflictStatus::SameSizeDestNewer
            }
        } else {
            // Different sizes
            if self.source.modified > self.dest.modified {
                ConflictStatus::DifferentSizeSourceNewer
            } else {
                ConflictStatus::DifferentSizeDestNewer
            }
        }
    }

    /// Get display indicator and message for the conflict status
    pub fn get_status_message(&self) -> (&'static str, String) {
        match self.get_status() {
            ConflictStatus::SameSizeDate => ("✓", "Same size and date".to_string()),
            ConflictStatus::SameSizeSourceNewer => ("✓", "Same size, Source is newer".to_string()),
            ConflictStatus::SameSizeDestNewer => {
                ("⚠", "Same size, Destination is newer".to_string())
            }
            ConflictStatus::DifferentSizeSourceNewer => (
                "⚠",
                format!(
                    "Different size (Source: {}, Dest: {})",
                    crate::model::file_entry::format_size(self.source.size),
                    crate::model::file_entry::format_size(self.dest.size)
                ),
            ),
            ConflictStatus::DifferentSizeDestNewer => (
                "⚠",
                format!(
                    "Different size (Source: {}, Dest: {})",
                    crate::model::file_entry::format_size(self.source.size),
                    crate::model::file_entry::format_size(self.dest.size)
                ),
            ),
            ConflictStatus::DirectoryMerge => ("⚠", "Will merge directories".to_string()),
        }
    }
}

/// Status of a file conflict for display
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictStatus {
    SameSizeDate,
    SameSizeSourceNewer,
    SameSizeDestNewer,
    DifferentSizeSourceNewer,
    DifferentSizeDestNewer,
    DirectoryMerge,
}
