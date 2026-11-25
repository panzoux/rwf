//! Delete confirmation dialog content.

#[derive(Debug, Clone)]
pub struct DeleteConfirmDialog {
    /// (location, is_dir) pairs to be deleted
    pub targets: Vec<(crate::model::Location, bool)>,
    pub scroll_offset: usize,
    /// Intended to be resolved at dialog-construction time from
    /// `AppConfig::trash.enabled` (see `Dialog::delete_confirm`), so
    /// confirm-handlers don't each have to re-derive it from config and
    /// risk drifting out of sync. Already drives the rendered header/hint
    /// text; the caller that builds this dialog (`Action::Delete`) and the
    /// confirm-handlers that read it back out still pass/ignore a
    /// hardcoded placeholder until later tasks in Phase 7.7 wire them to
    /// real config.
    pub to_trash: bool,
    /// Intended to be resolved from `AppConfig::trash.force_fallback` at
    /// construction time, same rationale and same current placeholder
    /// status as `to_trash`.
    pub force_fallback: bool,
}

impl DeleteConfirmDialog {
    pub fn new(
        targets: Vec<(crate::model::Location, bool)>,
        to_trash: bool,
        force_fallback: bool,
    ) -> Self {
        Self {
            targets,
            scroll_offset: 0,
            to_trash,
            force_fallback,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_confirm_dialog_carries_to_trash_and_force_fallback_flags() {
        let dialog = DeleteConfirmDialog::new(vec![], true, true);
        assert!(dialog.to_trash);
        assert!(dialog.force_fallback);

        let dialog = DeleteConfirmDialog::new(vec![], false, false);
        assert!(!dialog.to_trash);
        assert!(!dialog.force_fallback);
    }
}
