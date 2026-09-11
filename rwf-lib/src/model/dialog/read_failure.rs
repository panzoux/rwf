//! A pane directory that could not be read — and, by the time this dialog shows,
//! nothing above it could be read either (Phase 7.22 §3.4).
//!
//! It replaces the generic "Operation Failed" modal for pane reads. That one had the
//! wrong title (at startup the user has operated nothing), repeated three layers of
//! prefixes ("Read directory failed: Failed to read directory …") and offered only
//! OK. This one answers the four questions the report asked — *what* failed (the
//! title, from a structured [`FailureKind`]), *where* (tab and pane), *when* (the
//! job's [`JobOrigin`]) and *why* (the path, the OS's own words, a hint) — and turns
//! the dead end into a choice: `[Retry]` or `[Dismiss]`.
//!
//! There is deliberately no `[Open parent]` (§3.5). A pane read only reaches this
//! dialog after `ResolveFallbackPath` walked every ancestor and found none readable,
//! so a parent to open does not exist by construction.

use crate::job::{FailureKind, JobOrigin};
use crate::model::{ActivePane, Location};

/// Button index of `[Retry]`.
pub const READ_FAILURE_RETRY: usize = 0;
/// Button index of `[Dismiss]` — the default focus. Retrying a dead network path
/// blocks a worker for another full timeout, so `Enter` must not mean that unless
/// the user moved focus there on purpose.
pub const READ_FAILURE_DISMISS: usize = 1;

#[derive(Debug, Clone)]
pub struct ReadFailureDialog {
    /// The tab's **id**, not its position: the dialog can outlive a tab close, and a
    /// position would then name a different tab.
    pub tab_id: usize,
    pub side: ActivePane,
    /// "Tab 5, right pane", resolved when the dialog was built.
    pub where_label: String,
    pub origin: JobOrigin,
    pub location: Location,
    pub kind: FailureKind,
    /// The OS's wording, verbatim and possibly localized — what a user pastes into a
    /// search. The backend's own "Failed to read directory <path>: " is removed, since
    /// the path already has a line of its own.
    pub cause: String,
    pub focused_button: usize,
}

impl ReadFailureDialog {
    pub fn new(
        tab_id: usize,
        side: ActivePane,
        where_label: String,
        origin: JobOrigin,
        location: Location,
        error_message: &str,
    ) -> Self {
        let kind = FailureKind::classify(error_message);
        let prefix = format!("Failed to read directory {}: ", location.display_path());
        let cause = error_message
            .strip_prefix(prefix.as_str())
            .unwrap_or(error_message)
            .to_string();
        Self {
            tab_id,
            side,
            where_label,
            origin,
            location,
            kind,
            cause,
            focused_button: READ_FAILURE_DISMISS,
        }
    }

    /// "while restoring your session" — the difference between an alarming error and
    /// an explicable one. `None` for a keypress: the user knows what they just did.
    pub fn when_label(&self) -> Option<&'static str> {
        match self.origin {
            JobOrigin::SessionRestore => Some("while restoring your session"),
            JobOrigin::Refresh => Some("while refreshing the pane"),
            JobOrigin::UserAction => None,
        }
    }

    /// "Tab 5, right pane — while restoring your session".
    pub fn context_line(&self) -> String {
        match self.when_label() {
            Some(when) => format!("{} — {}", self.where_label, when),
            None => self.where_label.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dialog(message: &str, origin: JobOrigin) -> ReadFailureDialog {
        let location = Location::Local(PathBuf::from("/mnt/share"));
        ReadFailureDialog::new(
            7,
            ActivePane::Right,
            "Tab 2, right pane".to_string(),
            origin,
            location,
            message,
        )
    }

    #[test]
    fn the_backend_prefix_is_dropped_from_the_cause() {
        let path = Location::Local(PathBuf::from("/mnt/share")).display_path();
        let d = dialog(
            &format!("Failed to read directory {path}: No such file or directory (os error 2)"),
            JobOrigin::UserAction,
        );
        assert_eq!(d.cause, "No such file or directory (os error 2)");
    }

    #[test]
    fn a_message_without_the_prefix_is_kept_whole() {
        let d = dialog("network name not resolved", JobOrigin::UserAction);
        assert_eq!(d.cause, "network name not resolved");
    }

    #[test]
    fn focus_starts_on_dismiss_so_enter_never_retries_by_accident() {
        let d = dialog("x", JobOrigin::SessionRestore);
        assert_eq!(d.focused_button, READ_FAILURE_DISMISS);
    }

    #[test]
    fn the_context_line_says_when_for_a_restore_but_not_for_a_keypress() {
        assert_eq!(
            dialog("x", JobOrigin::SessionRestore).context_line(),
            "Tab 2, right pane — while restoring your session"
        );
        assert_eq!(
            dialog("x", JobOrigin::UserAction).context_line(),
            "Tab 2, right pane"
        );
    }
}
