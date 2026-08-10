//! Operation Report data model (Phase 7.6).
//!
//! Every completed file-mutating Job is converted into an `OperationReport`:
//! a per-file breakdown of what happened, whether it can be undone, and (if
//! so) the exact `ReversalAction` that would undo it. See
//! `plan/7.6.operation_report_ui.md` for the UI/UX this backs.

use crate::model::{AttributeChange, LinkCreateKind, Location, TimestampChange, TrashRecord};

/// The concrete filesystem action that would reverse (or replay) a single
/// `OperationRecord`. Also used, by extension, as the "redo the original
/// action" step after an undo — see `Delete::recreate`.
#[derive(Debug, Clone, PartialEq)]
pub enum ReversalAction {
    Copy {
        from: Location,
        to: Location,
    },
    Move {
        from: Location,
        to: Location,
    },
    Rename {
        from: Location,
        to: Location,
    },
    /// Permanently removes `target`. Not itself mechanically invertible (data
    /// is gone), so it carries `recreate`: the action that would recreate
    /// `target` from scratch, precomputed at the point this `Delete` was
    /// built as the undo of a Copy/Mkdir/CreateFile/CreateLink/CreateArchive.
    /// `None` for a genuine forward permanent-delete record (never reachable
    /// via undo, since permanent Delete's own `undo` is always
    /// `UndoAvailability::NotApplicable` — see Task 5).
    Delete {
        target: Location,
        recreate: Option<Box<ReversalAction>>,
    },
    Mkdir {
        location: Location,
    },
    CreateFile {
        location: Location,
    },
    CreateLink {
        target: Location,
        link_path: Location,
        kind: LinkCreateKind,
    },
    RestoreAttributes {
        target: Location,
        attrs: AttributeChange,
    },
    RestoreTimestamps {
        target: Location,
        times: TimestampChange,
    },
    MoveToTrash {
        target: Location,
    },
    RestoreFromTrash {
        record: TrashRecord,
    },
    CreateArchive {
        sources: Vec<Location>,
        dest: Location,
    },
}

/// Whether — and how — a single `OperationRecord` can be undone/redone.
#[derive(Debug, Clone, PartialEq)]
pub enum UndoAvailability {
    /// Pressing the trigger key on this row runs `ReversalAction`.
    Available(ReversalAction),
    /// This operation type supports undo, but this specific row succeeded
    /// without the executor being able to capture enough data to build a
    /// `ReversalAction` (defensive — see module docs).
    Unavailable(String),
    /// This operation type never supports undo (permanent Delete, Extract
    /// Archive), or this row's operation failed (nothing happened to undo).
    NotApplicable,
}

/// One row of an `OperationReport`: what happened to a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationRecord {
    pub source: Option<Location>,
    pub destination: Option<Location>,
    pub succeeded: bool,
    pub failure_reason: Option<String>,
    pub undo: UndoAvailability,
}

/// The result of one completed Job, ready to display in the Operation Report
/// dialog. `is_undo` is true iff this report is itself the result of running
/// an Undo/Redo — it flips every time the user triggers the action on a
/// report, which is what makes the last column alternate between "Undo" and
/// "Redo" (see `action_column_label`).
#[derive(Debug, Clone, PartialEq)]
pub struct OperationReport {
    pub id: u64,
    /// Base operation name ("Copy", "Move", "Change Attributes", ...) —
    /// never prefixed with "Undo"/"Redo"; that's derived at display time.
    pub operation_name: String,
    pub records: Vec<OperationRecord>,
    pub finished_at: std::time::SystemTime,
    pub is_undo: bool,
}

impl OperationReport {
    /// "Undo" for a normal report, "Redo" for the result of running an undo.
    pub fn action_column_label(&self) -> &'static str {
        if self.is_undo {
            "Redo"
        } else {
            "Undo"
        }
    }

    /// Display title, e.g. "Copy Report" / "Undo Copy Report".
    pub fn title(&self) -> String {
        if self.is_undo {
            format!("Undo {} Report", self.operation_name)
        } else {
            format!("{} Report", self.operation_name)
        }
    }

    /// Number of rows whose `undo` is `Available` — used to decide whether
    /// the dialog's trigger key does anything at all.
    pub fn undoable_count(&self) -> usize {
        self.records
            .iter()
            .filter(|r| matches!(r.undo, UndoAvailability::Available(_)))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(undo: UndoAvailability) -> OperationRecord {
        OperationRecord {
            source: Some(Location::Local("a.txt".into())),
            destination: Some(Location::Local("b.txt".into())),
            succeeded: true,
            failure_reason: None,
            undo,
        }
    }

    #[test]
    fn forward_report_labels_action_as_undo() {
        let report = OperationReport {
            id: 1,
            operation_name: "Copy".to_string(),
            records: vec![sample_record(UndoAvailability::Available(
                ReversalAction::Delete {
                    target: Location::Local("b.txt".into()),
                    recreate: None,
                },
            ))],
            finished_at: std::time::SystemTime::now(),
            is_undo: false,
        };
        assert_eq!(report.action_column_label(), "Undo");
        assert_eq!(report.title(), "Copy Report");
        assert_eq!(report.undoable_count(), 1);
    }

    #[test]
    fn undo_report_labels_action_as_redo() {
        let report = OperationReport {
            id: 2,
            operation_name: "Copy".to_string(),
            records: vec![sample_record(UndoAvailability::NotApplicable)],
            finished_at: std::time::SystemTime::now(),
            is_undo: true,
        };
        assert_eq!(report.action_column_label(), "Redo");
        assert_eq!(report.title(), "Undo Copy Report");
        assert_eq!(report.undoable_count(), 0);
    }
}
