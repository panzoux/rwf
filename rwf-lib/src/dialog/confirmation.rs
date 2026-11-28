//! Generic action-confirmation dialog content.
//!
//! One dialog type for every "are you sure?" prompt whose only job is to gate
//! a follow-up action behind a Yes/No — as opposed to `DeleteConfirm`,
//! `ExtractionConfirm`, etc., which carry per-dialog execution state (target
//! lists, archive paths) that a plain yes/no can't hold.

/// Item count / total byte size shown alongside the message, so the user can
/// judge whether to proceed before an irreversible action (e.g. "12 items,
/// 340 MB will be permanently deleted").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfirmStats {
    pub count: usize,
    pub total_size: u64,
}

/// What happens when the user confirms. Each variant carries exactly the
/// data its follow-up transition/job needs — add a variant here for each new
/// confirm-only action rather than growing `ActionConfirmDialog` itself.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmableAction {
    /// Reload config.json / keybindings / custom functions / etc. from disk.
    ReloadConfig,
    /// Permanently purge the trash (OS-managed + `.rwf-trash` fallback dirs).
    EmptyTrash {
        fallback_roots: Vec<std::path::PathBuf>,
    },
    /// Run a reversal batch after pre-flight found some rows blocked — the
    /// user confirmed "proceed with the N that are ready" (Phase 7.6).
    ExecuteReversal {
        actions: Vec<crate::model::ReversalAction>,
        operation_name: String,
        resulting_is_undo: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionConfirmDialog {
    pub message: String,
    pub stats: Option<ConfirmStats>,
    pub action: ConfirmableAction,
}

impl ActionConfirmDialog {
    pub fn new(message: String, stats: Option<ConfirmStats>, action: ConfirmableAction) -> Self {
        Self {
            message,
            stats,
            action,
        }
    }
}
