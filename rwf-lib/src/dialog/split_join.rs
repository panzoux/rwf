//! Split/join files dialog content.

use super::SplitJoinMode;

#[derive(Debug, Clone)]
pub struct SplitJoinDialogContent {
    pub mode: SplitJoinMode,
    pub chunk_size_mb: u64,
}

impl SplitJoinDialogContent {
    pub fn new() -> Self {
        Self {
            mode: SplitJoinMode::Split,
            chunk_size_mb: 100, // Default 100MB chunks
        }
    }
}

impl Default for SplitJoinDialogContent {
    fn default() -> Self {
        Self::new()
    }
}
