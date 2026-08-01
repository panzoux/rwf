//! Records identifying an item moved to the OS trash, with enough detail to
//! restore it later (Phase 7.6 Undo will drive `RestoreFromTrash` using
//! these records — see `plan/7.6.transactional_rollback.md` §9).

use crate::model::Location;
use std::path::PathBuf;
use std::time::SystemTime;

/// Where a trashed item's platform-level entry lives, so it can be restored
/// through the same mechanism that trashed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrashLocation {
    /// Windows/Linux: tracked by the OS trash via the `trash` crate's
    /// `os_limited` module. Fields mirror `trash::TrashItem` (which isn't
    /// `PartialEq`, so we copy its fields rather than store it directly).
    OsManaged {
        id: std::ffi::OsString,
        name: std::ffi::OsString,
        original_parent: PathBuf,
        time_deleted: i64,
    },
    /// The primary OS trash call was skipped (`force_fallback`) or failed
    /// (e.g. cross-device move on a non-standard mount), and the item was
    /// moved into a `.rwf-trash` sidecar directory at the volume root
    /// instead, tracked by our own JSON metadata file next to it.
    Fallback { trash_path: PathBuf },
    /// Deleted successfully via `trash::delete()`, but RWF has no way to
    /// restore it from within the app. Two cases land here: (a) this
    /// platform doesn't expose `os_limited::list`/`restore` at all (macOS),
    /// or (b) it does (Windows/Linux), but the post-delete lookup in
    /// `os_trash_entry_for` didn't find a matching `TrashItem` — e.g. the
    /// `list()` call itself errored, or no parent+name match was found. In
    /// both cases the file *is* still sitting in the OS trash and can be
    /// restored manually from the OS's own trash UI (Finder / Recycle Bin);
    /// RWF just can't drive that restore itself for this record.
    Untracked,
}

/// Everything needed to identify a trashed item: for restoring it, and for
/// Phase 7.6's Operation Report / Undo journal (which wants source,
/// destination-equivalent, size, and mtime for its pre-flight validation —
/// see `plan/7.6.operation_report_ui.md` "Undo情報").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashRecord {
    pub original: Location,
    pub trash_location: TrashLocation,
    pub size: u64,
    pub modified: SystemTime,
}

/// Per-target result of a `MoveToTrash` job. Not `FileOpOutcome<T>`: that
/// type's `new: T` field represents "the change that was requested" (valid
/// even on failure, e.g. `AttributeChange`), but a trash move has no
/// meaningful "requested value" — only an output that may not exist on
/// failure. Modelled separately instead of forcing an awkward fit.
#[derive(Debug, Clone)]
pub struct TrashOutcome {
    pub target: Location,
    pub record: Option<TrashRecord>,
    pub result: Result<(), String>,
}

/// Per-target result of a `RestoreFromTrash` job.
#[derive(Debug, Clone)]
pub struct RestoreOutcome {
    pub original: Location,
    pub result: Result<(), String>,
}

/// Which part of the trash an `EmptyTrash` job should purge. Kept separate
/// from a single monolithic purge function so a future OS-specific
/// implementation (e.g. macOS via AppleScript, since `os_limited` doesn't
/// cover it) can be added to the `OsManaged` path without touching the
/// `.rwf-trash` fallback path, and vice versa. Lives here (not `job.rs`) so
/// both `backend.rs`'s trait and `job.rs`'s `JobKind` can use it without a
/// `backend → job` dependency — same reason `LinkCreateKind` lives in
/// `model` rather than `job.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyTrashScope {
    OsManaged,
    Fallback,
    All,
}
