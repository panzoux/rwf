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
    let (operation_name, is_undo) = match &spec.kind {
        JobKind::ExecuteReversal {
            operation_name,
            resulting_is_undo,
            ..
        } => (operation_name.clone(), *resulting_is_undo),
        other => (operation_name_for(other)?, false),
    };

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
        is_undo,
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
        JobKind::ExecuteReversal { .. } => return None, // handled by the caller before this fn runs
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
        JobKind::ExecuteReversal { actions, .. } => actions
            .first()
            .map(reversal_action_locations)
            .unwrap_or((None, None)),
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

/// Best-effort (source, destination) for a single `ReversalAction`, used by
/// `single_failure_record` when a whole `ExecuteReversal` job fails outright
/// (not per-action) — there's no single obvious source/destination for a
/// heterogeneous batch, so this just reports the first action's locations,
/// mirroring the shape each variant's own successful `OperationRecord` would
/// have used (see `JobExecutor::execute_reversal`).
fn reversal_action_locations(
    action: &ReversalAction,
) -> (
    Option<crate::model::Location>,
    Option<crate::model::Location>,
) {
    match action {
        ReversalAction::Copy { from, to } | ReversalAction::Move { from, to } => {
            (Some(from.clone()), Some(to.clone()))
        }
        ReversalAction::Rename { from, to } => (Some(from.clone()), Some(to.clone())),
        ReversalAction::Delete { target, .. } => (Some(target.clone()), None),
        ReversalAction::Mkdir { location } | ReversalAction::CreateFile { location } => {
            (None, Some(location.clone()))
        }
        ReversalAction::CreateLink {
            target, link_path, ..
        } => (Some(target.clone()), Some(link_path.clone())),
        ReversalAction::RestoreAttributes { target, .. }
        | ReversalAction::RestoreTimestamps { target, .. } => (None, Some(target.clone())),
        ReversalAction::MoveToTrash { target } => (Some(target.clone()), None),
        ReversalAction::RestoreFromTrash { record } => (None, Some(record.original.clone())),
        ReversalAction::CreateArchive { dest, .. } => (None, Some(dest.clone())),
    }
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
    fn timestamp_change_translates_old_value_into_restore_action() {
        let spec = JobSpec::new(JobKind::ChangeTimestamps {
            targets: vec![Location::Local("a.txt".into())],
            times: crate::model::TimestampChange::default(),
        });
        let outcome = FileOpOutcome {
            target: Location::Local("a.txt".into()),
            old: Some(crate::model::TimestampChange::default()),
            new: crate::model::TimestampChange::default(),
            result: Ok(()),
        };
        let result = OpResult::Success(SuccessData::TimestampsChanged(vec![outcome]));

        let report = build_operation_report(&spec, &result, 8).expect("report");
        assert_eq!(report.operation_name, "Change Timestamps");
        assert_eq!(report.records.len(), 1);
        assert!(matches!(
            report.records[0].undo,
            UndoAvailability::Available(ReversalAction::RestoreTimestamps { .. })
        ));
    }

    #[test]
    fn attribute_change_error_branch_is_not_applicable() {
        let spec = JobSpec::new(JobKind::ChangeAttributes {
            targets: vec![Location::Local("a.txt".into())],
            attrs: crate::model::AttributeChange::default(),
        });
        let outcome = FileOpOutcome {
            target: Location::Local("a.txt".into()),
            old: Some(crate::model::AttributeChange::default()),
            new: crate::model::AttributeChange::default(),
            result: Err("access denied".to_string()),
        };
        let result = OpResult::Success(SuccessData::AttributesChanged(vec![outcome]));

        let report = build_operation_report(&spec, &result, 9).expect("report");
        assert_eq!(report.records.len(), 1);
        assert!(!report.records[0].succeeded);
        assert_eq!(
            report.records[0].failure_reason.as_deref(),
            Some("access denied")
        );
        assert!(matches!(
            report.records[0].undo,
            UndoAvailability::NotApplicable
        ));
    }

    #[test]
    fn trash_restored_translates_into_move_to_trash_action() {
        let spec = JobSpec::new(JobKind::RestoreFromTrash { records: vec![] });
        let outcome = crate::model::RestoreOutcome {
            original: Location::Local("a.txt".into()),
            result: Ok(()),
        };
        let result = OpResult::Success(SuccessData::TrashRestored(vec![outcome]));

        let report = build_operation_report(&spec, &result, 10).expect("report");
        assert_eq!(report.operation_name, "Restore from Trash");
        assert_eq!(report.records.len(), 1);
        assert!(report.records[0].succeeded);
        assert_eq!(
            report.records[0].destination,
            Some(Location::Local("a.txt".into()))
        );
        assert!(matches!(
            &report.records[0].undo,
            UndoAvailability::Available(ReversalAction::MoveToTrash { target })
                if *target == Location::Local("a.txt".into())
        ));
    }

    #[test]
    fn trash_restored_error_branch_is_not_applicable() {
        let spec = JobSpec::new(JobKind::RestoreFromTrash { records: vec![] });
        let outcome = crate::model::RestoreOutcome {
            original: Location::Local("a.txt".into()),
            result: Err("target already exists".to_string()),
        };
        let result = OpResult::Success(SuccessData::TrashRestored(vec![outcome]));

        let report = build_operation_report(&spec, &result, 11).expect("report");
        assert_eq!(report.records.len(), 1);
        assert!(!report.records[0].succeeded);
        assert!(matches!(
            report.records[0].undo,
            UndoAvailability::NotApplicable
        ));
    }

    #[test]
    fn execute_reversal_flips_is_undo_and_keeps_base_operation_name() {
        let spec = JobSpec::new(JobKind::ExecuteReversal {
            actions: vec![],
            operation_name: "Copy".to_string(),
            resulting_is_undo: true,
        });
        let result = OpResult::Success(SuccessData::OperationRecords(vec![]));
        let report = build_operation_report(&spec, &result, 12).expect("report");
        assert_eq!(report.operation_name, "Copy");
        assert!(report.is_undo);
        assert_eq!(report.title(), "Undo Copy Report");
    }

    #[test]
    fn execute_reversal_whole_job_failure_extracts_first_action_locations() {
        let spec = JobSpec::new(JobKind::ExecuteReversal {
            actions: vec![ReversalAction::Move {
                from: Location::Local("b.txt".into()),
                to: Location::Local("a.txt".into()),
            }],
            operation_name: "Move".to_string(),
            resulting_is_undo: true,
        });
        let result = OpResult::Failed("disk full".to_string());
        let report = build_operation_report(&spec, &result, 13).expect("report");
        assert_eq!(report.operation_name, "Move");
        assert!(report.is_undo);
        assert_eq!(report.records.len(), 1);
        assert!(!report.records[0].succeeded);
        assert_eq!(
            report.records[0].source,
            Some(Location::Local("b.txt".into()))
        );
        assert_eq!(
            report.records[0].destination,
            Some(Location::Local("a.txt".into()))
        );
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
