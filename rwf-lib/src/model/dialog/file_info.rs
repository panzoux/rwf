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
        }
    }
}
