//! File information dialog content.

#[derive(Debug, Clone)]
pub struct FileInfoDialog {
    pub file_name: String,
    pub file_path: String,
    pub size: u64,
    pub created: Option<std::time::SystemTime>,
    pub modified: std::time::SystemTime,
    pub accessed: Option<std::time::SystemTime>,
    pub is_dir: bool,
    pub is_readonly: bool,
    #[cfg(unix)]
    pub permissions: Option<u32>,
    #[cfg(unix)]
    pub owner: Option<String>,
    #[cfg(unix)]
    pub group: Option<String>,
    pub link_target: Option<String>,
    pub link_kind: Option<crate::model::LinkKind>,
    pub detected_type: Option<String>,
    pub detecting: bool,
    pub detected_type_job_id: Option<crate::job::JobId>,
    /// Leading bytes (up to 64) used for content-type detection, for audit
    /// display alongside `detected_type` (Phase 7.3b, Task 10). `None` until
    /// detection completes successfully; stays `None` on failure/cancel.
    pub header_bytes: Option<Vec<u8>>,
    /// true = hex/offset/ASCII view, false = raw text view. Defaults to true.
    pub header_hex_mode: bool,
    /// True when the entry's `Location` is `Location::Local` (Phase 7.3 §7).
    /// On-demand content-type detection does real filesystem I/O, which is
    /// meaningless for archive-internal or remote entries — `false` here
    /// makes the 'd' handler report "not available" instead of starting a
    /// doomed job.
    pub is_local: bool,
}

impl FileInfoDialog {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        file_name: String,
        file_path: String,
        size: u64,
        created: Option<std::time::SystemTime>,
        modified: std::time::SystemTime,
        accessed: Option<std::time::SystemTime>,
        is_dir: bool,
        is_readonly: bool,
        #[cfg(unix)] permissions: Option<u32>,
        #[cfg(unix)] owner: Option<String>,
        #[cfg(unix)] group: Option<String>,
        link_target: Option<String>,
        link_kind: Option<crate::model::LinkKind>,
        is_local: bool,
    ) -> Self {
        Self {
            file_name,
            file_path,
            size,
            created,
            modified,
            accessed,
            is_dir,
            is_readonly,
            #[cfg(unix)]
            permissions,
            #[cfg(unix)]
            owner,
            #[cfg(unix)]
            group,
            link_target,
            link_kind,
            detected_type: None,
            detecting: false,
            detected_type_job_id: None,
            header_bytes: None,
            header_hex_mode: true,
            is_local,
        }
    }
}
