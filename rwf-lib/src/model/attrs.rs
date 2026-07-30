//! File attribute and timestamp change requests, and per-file operation outcomes.
//!
//! `Option<T>` fields represent "leave unchanged" (`None`) vs "set to this value"
//! (`Some(_)`), so a single change can be applied sparsely across a batch of files
//! that don't all share the same current value.

use crate::model::Location;

/// Requested attribute changes. Fields are `None` when left unchanged.
///
/// `hidden` is Windows-only (`FILE_ATTRIBUTE_HIDDEN`) — on Unix, "hidden" is a
/// naming convention (a leading dot), not a settable attribute bit, so it isn't
/// represented here at all. Setting it there would mean renaming the file, which
/// is a different operation (Rename), not an attribute change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttributeChange {
    #[cfg(windows)]
    pub readonly: Option<bool>,
    #[cfg(windows)]
    pub hidden: Option<bool>,
    #[cfg(windows)]
    pub system: Option<bool>,
    #[cfg(windows)]
    pub archive: Option<bool>,
    #[cfg(unix)]
    pub mode: Option<u32>,
}

impl AttributeChange {
    /// True when every field is `None` (nothing was actually touched).
    pub fn is_empty(&self) -> bool {
        #[cfg(windows)]
        {
            self.readonly.is_none()
                && self.hidden.is_none()
                && self.system.is_none()
                && self.archive.is_none()
        }
        #[cfg(unix)]
        {
            self.mode.is_none()
        }
    }
}

/// Requested timestamp changes. Fields are `None` when left unchanged.
///
/// `created` is Windows-only: setting it requires a raw `SetFileTime` FFI
/// call (`volume_info::set_windows_creation_time`) beyond what the
/// cross-platform `filetime` crate exposes for `modified`/`accessed`. Unix
/// has no portable birthtime setter at all, so it isn't represented here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimestampChange {
    pub modified: Option<std::time::SystemTime>,
    pub accessed: Option<std::time::SystemTime>,
    #[cfg(windows)]
    pub created: Option<std::time::SystemTime>,
}

impl TimestampChange {
    /// True when every field is `None` (nothing was actually touched).
    pub fn is_empty(&self) -> bool {
        #[cfg(windows)]
        {
            self.modified.is_none() && self.accessed.is_none() && self.created.is_none()
        }
        #[cfg(not(windows))]
        {
            self.modified.is_none() && self.accessed.is_none()
        }
    }
}

/// Result of applying a change to a single file, retaining the prior value so a
/// future Undo (Phase 7.6) can restore it without re-deriving it from disk.
#[derive(Debug, Clone)]
pub struct FileOpOutcome<T> {
    pub target: Location,
    pub old: Option<T>,
    pub new: T,
    pub result: Result<(), String>,
}

/// Kind of filesystem link to create. `Junction` is Windows-only and only
/// valid for directory targets (NTFS reparse point).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkCreateKind {
    Symlink,
    Hardlink,
    #[cfg(windows)]
    Junction,
}
