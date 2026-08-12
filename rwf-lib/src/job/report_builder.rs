//! Builds `OperationReport`s from completed Jobs (Phase 7.6).

use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
use crate::model::{OperationRecord, OperationReport, ReversalAction, UndoAvailability};

/// Converts a completed job into an `OperationReport`, if that job's kind is
/// one Phase 7.6 tracks. Returns `None` for job kinds with no Operation
/// Report concept (navigation, search, viewer jobs, etc.) and for
/// `OpResult::Cancelled` (nothing meaningful happened).
pub fn build_operation_report(
    spec: &JobSpec,
    result: &OpResult,
    id: u64,
) -> Option<OperationReport> {
    let operation_name = operation_name_for(&spec.kind)?;

    let records = match result {
        OpResult::Success(SuccessData::OperationRecords(records)) => records.clone(),
        OpResult::Success(SuccessData::AttributesChanged(outcomes)) => outcomes
            .iter()
            .map(|o| OperationRecord {
                source: None,
                destination: Some(o.target.clone()),
                succeeded: o.result.is_ok(),
                failure_reason: o.result.as_ref().err().cloned(),
                undo: match (&o.result, &o.old) {
                    (Ok(()), Some(old)) => {
                        UndoAvailability::Available(ReversalAction::RestoreAttributes {
                            target: o.target.clone(),
                            attrs: old.clone(),
                        })
                    }
                    (Ok(()), None) => UndoAvailability::Unavailable(
                        "original attributes not captured".to_string(),
                    ),
                    (Err(_), _) => UndoAvailability::NotApplicable,
                },
            })
            .collect(),
        OpResult::Success(SuccessData::TimestampsChanged(outcomes)) => outcomes
            .iter()
            .map(|o| OperationRecord {
                source: None,
                destination: Some(o.target.clone()),
                succeeded: o.result.is_ok(),
                failure_reason: o.result.as_ref().err().cloned(),
                undo: match (&o.result, &o.old) {
                    (Ok(()), Some(old)) => {
                        UndoAvailability::Available(ReversalAction::RestoreTimestamps {
                            target: o.target.clone(),
                            times: old.clone(),
                        })
                    }
                    (Ok(()), None) => UndoAvailability::Unavailable(
                        "original timestamps not captured".to_string(),
                    ),
                    (Err(_), _) => UndoAvailability::NotApplicable,
                },
            })
            .collect(),
        OpResult::Success(SuccessData::TrashMoved(outcomes)) => outcomes
            .iter()
            .map(|o| OperationRecord {
                source: Some(o.target.clone()),
                destination: None,
                succeeded: o.result.is_ok(),
                failure_reason: o.result.as_ref().err().cloned(),
                undo: match (&o.result, &o.record) {
                    (Ok(()), Some(record)) => {
                        UndoAvailability::Available(ReversalAction::RestoreFromTrash {
                            record: record.clone(),
                        })
                    }
                    (Ok(()), None) => {
                        UndoAvailability::Unavailable("trash record not captured".to_string())
                    }
                    (Err(_), _) => UndoAvailability::NotApplicable,
                },
            })
            .collect(),
        OpResult::Success(SuccessData::TrashRestored(outcomes)) => outcomes
            .iter()
            .map(|o| OperationRecord {
                source: None,
                destination: Some(o.original.clone()),
                succeeded: o.result.is_ok(),
                failure_reason: o.result.as_ref().err().cloned(),
                undo: if o.result.is_ok() {
                    UndoAvailability::Available(ReversalAction::MoveToTrash {
                        target: o.original.clone(),
                    })
                } else {
                    UndoAvailability::NotApplicable
                },
            })
            .collect(),
        OpResult::Success(_) => {
            // ExtractArchive (SuccessData::None) and any other tracked kind
            // whose executor has no per-file breakdown.
            if let JobKind::ExtractArchive { archive, dest } = &spec.kind {
                vec![OperationRecord {
                    source: Some(archive.clone()),
                    destination: Some(dest.clone()),
                    succeeded: true,
                    failure_reason: None,
                    undo: UndoAvailability::NotApplicable,
                }]
            } else {
                return None;
            }
        }
        OpResult::Failed(e) => {
            vec![single_failure_record(&spec.kind, e.clone())?]
        }
        OpResult::Cancelled => return None,
    };

    Some(OperationReport {
        id,
        operation_name,
        records,
        finished_at: std::time::SystemTime::now(),
        is_undo: false,
    })
}

/// Base display name for a tracked JobKind, or `None` if this kind has no
/// Operation Report concept.
fn operation_name_for(kind: &JobKind) -> Option<String> {
    let name = match kind {
        JobKind::Copy { .. } => "Copy",
        JobKind::Move { .. } => "Move",
        JobKind::Rename { .. } => "Rename",
        JobKind::Delete { .. } => "Delete",
        JobKind::Mkdir { .. } => "Create Directory",
        JobKind::CreateFile { .. } => "Create File",
        JobKind::CreateLink { .. } => "Create Link",
        JobKind::CreateArchive { .. } => "Create Archive",
        JobKind::ChangeAttributes { .. } => "Change Attributes",
        JobKind::ChangeTimestamps { .. } => "Change Timestamps",
        JobKind::MoveToTrash { .. } => "Move to Trash",
        JobKind::RestoreFromTrash { .. } => "Restore from Trash",
        JobKind::ExtractArchive { .. } => "Extract Archive",
        _ => return None,
    };
    Some(name.to_string())
}

/// A whole-job failure (no per-file breakdown available) still produces a
/// one-row report so the user sees *something* went wrong, rather than the
/// job silently vanishing. `None` for kinds Task 9 doesn't track.
fn single_failure_record(kind: &JobKind, reason: String) -> Option<OperationRecord> {
    let (source, destination) = match kind {
        JobKind::Copy { sources, dest } | JobKind::Move { sources, dest } => {
            (sources.first().cloned(), Some(dest.clone()))
        }
        JobKind::Rename { from, to } => (Some(from.clone()), Some(to.clone())),
        JobKind::Delete { targets } => (targets.first().cloned(), None),
        JobKind::Mkdir { location } | JobKind::CreateFile { location } => {
            (None, Some(location.clone()))
        }
        JobKind::CreateLink {
            target, link_path, ..
        } => (Some(target.clone()), Some(link_path.clone())),
        JobKind::CreateArchive { dest, .. } => (None, Some(dest.clone())),
        JobKind::ChangeAttributes { targets, .. } | JobKind::ChangeTimestamps { targets, .. } => {
            (None, targets.first().cloned())
        }
        JobKind::MoveToTrash { targets, .. } => (targets.first().cloned(), None),
        JobKind::RestoreFromTrash { records } => {
            (None, records.first().map(|r| r.original.clone()))
        }
        JobKind::ExtractArchive { archive, dest } => (Some(archive.clone()), Some(dest.clone())),
        _ => return None,
    };
    Some(OperationRecord {
        source,
        destination,
        succeeded: false,
        failure_reason: Some(reason),
        undo: UndoAvailability::NotApplicable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobSpec;
    use crate::model::{FileOpOutcome, Location, TrashOutcome};

    #[test]
    fn copy_records_pass_through_unchanged() {
        let spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local("a.txt".into())],
            dest: Location::Local("dest".into()),
        });
        let record = OperationRecord {
            source: Some(Location::Local("a.txt".into())),
            destination: Some(Location::Local("dest/a.txt".into())),
            succeeded: true,
            failure_reason: None,
            undo: UndoAvailability::Available(ReversalAction::Delete {
                target: Location::Local("dest/a.txt".into()),
                recreate: None,
            }),
        };
        let result = OpResult::Success(SuccessData::OperationRecords(vec![record.clone()]));

        let report = build_operation_report(&spec, &result, 1).expect("report");
        assert_eq!(report.operation_name, "Copy");
        assert!(!report.is_undo);
        assert_eq!(report.records, vec![record]);
    }

    #[test]
    fn attribute_change_translates_old_value_into_restore_action() {
        let spec = JobSpec::new(JobKind::ChangeAttributes {
            targets: vec![Location::Local("a.txt".into())],
            attrs: crate::model::AttributeChange::default(),
        });
        let outcome = FileOpOutcome {
            target: Location::Local("a.txt".into()),
            old: Some(crate::model::AttributeChange::default()),
            new: crate::model::AttributeChange::default(),
            result: Ok(()),
        };
        let result = OpResult::Success(SuccessData::AttributesChanged(vec![outcome]));

        let report = build_operation_report(&spec, &result, 2).expect("report");
        assert_eq!(report.operation_name, "Change Attributes");
        assert_eq!(report.records.len(), 1);
        assert!(matches!(
            report.records[0].undo,
            UndoAvailability::Available(ReversalAction::RestoreAttributes { .. })
        ));
    }

    #[test]
    fn trash_move_without_captured_record_is_unavailable_not_notapplicable() {
        let spec = JobSpec::new(JobKind::MoveToTrash {
            targets: vec![Location::Local("a.txt".into())],
            force_fallback: false,
        });
        let outcome = TrashOutcome {
            target: Location::Local("a.txt".into()),
            record: None,
            result: Ok(()),
        };
        let result = OpResult::Success(SuccessData::TrashMoved(vec![outcome]));

        let report = build_operation_report(&spec, &result, 3).expect("report");
        assert!(matches!(
            report.records[0].undo,
            UndoAvailability::Unavailable(_)
        ));
    }

    #[test]
    fn extract_archive_is_always_not_applicable() {
        let spec = JobSpec::new(JobKind::ExtractArchive {
            archive: Location::Local("a.zip".into()),
            dest: Location::Local("out".into()),
        });
        let result = OpResult::Success(SuccessData::None);

        let report = build_operation_report(&spec, &result, 4).expect("report");
        assert_eq!(report.operation_name, "Extract Archive");
        assert_eq!(report.records.len(), 1);
        assert!(matches!(
            report.records[0].undo,
            UndoAvailability::NotApplicable
        ));
    }

    #[test]
    fn untracked_job_kind_produces_no_report() {
        let spec = JobSpec::new(JobKind::CalculateSize {
            location: Location::Local("dir".into()),
        });
        let result = OpResult::Success(SuccessData::SizeCalculated(123));
        assert!(build_operation_report(&spec, &result, 5).is_none());
    }

    #[test]
    fn cancelled_job_produces_no_report() {
        let spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local("a.txt".into())],
            dest: Location::Local("dest".into()),
        });
        assert!(build_operation_report(&spec, &OpResult::Cancelled, 6).is_none());
    }

    #[test]
    fn whole_job_failure_produces_single_failure_record() {
        let spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local("new_dir".into()),
        });
        let result = OpResult::Failed("permission denied".to_string());
        let report = build_operation_report(&spec, &result, 7).expect("report");
        assert_eq!(report.records.len(), 1);
        assert!(!report.records[0].succeeded);
        assert_eq!(
            report.records[0].failure_reason.as_deref(),
            Some("permission denied")
        );
    }
}
