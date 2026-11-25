//! Version information dialog content.

/// Fields shown in the "Version Information" dialog. Purely informational — no
/// interactive UI state.
#[derive(Debug, Clone)]
pub struct VersionDialog {
    pub version: String,
    pub build_date: String,
    pub copyright: String,
}

impl VersionDialog {
    pub fn new(version: String, build_date: String, copyright: String) -> Self {
        Self {
            version,
            build_date,
            copyright,
        }
    }
}
