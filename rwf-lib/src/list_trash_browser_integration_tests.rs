//! Phase 7.7 Task 16: `JobKind::ListTrash` completion must either push the
//! trash-browser dialog (populated with the listed records) or, when the
//! trash is already empty, skip the dialog and just log that fact — same
//! reasoning as Task 19's ScanTrash/EmptyTrash-confirm skip.

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::dialog::DialogContent;
    use crate::model::{Location, TrashLocation, TrashRecord};
    use crate::state::{update_state, StateUpdateResult, Transition};
    use crate::test_utils::test_state;
    use crate::AppState;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn sample_record(name: &str) -> TrashRecord {
        TrashRecord {
            original: Location::Local(PathBuf::from(format!("C:\\{name}"))),
            trash_location: TrashLocation::Fallback {
                trash_path: PathBuf::from(format!("C:\\.rwf-trash\\{name}")),
                trashed_at: 0,
            },
            size: 123,
            modified: SystemTime::UNIX_EPOCH,
        }
    }

    fn complete_list_trash_job(
        fallback_roots: Vec<PathBuf>,
        records: Vec<TrashRecord>,
    ) -> (AppState, StateUpdateResult) {
        let mut state = test_state();
        let job_spec = JobSpec::new(JobKind::ListTrash { fallback_roots });
        update_state(
            &mut state,
            Transition::EnqueueJob {
                spec: job_spec.clone(),
            },
        );
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);
        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::TrashListed(records)),
            },
        );
        (state, result)
    }

    #[test]
    fn list_trash_completion_pushes_browser_dialog_with_records_when_nonempty() {
        let records = vec![sample_record("a.txt"), sample_record("b.txt")];
        let (state, _result) = complete_list_trash_job(vec![PathBuf::from("C:\\")], records);

        let dialog = state
            .dialogs
            .current()
            .expect("a non-empty list should push the trash browser dialog");
        match &dialog.content {
            DialogContent::TrashBrowser(d) => {
                assert_eq!(d.records.len(), 2);
                assert_eq!(d.selected_index, 0);
            }
            other => panic!("expected a TrashBrowser dialog, got {other:?}"),
        }
    }

    #[test]
    fn list_trash_completion_skips_dialog_and_logs_when_already_empty() {
        let (state, result) = complete_list_trash_job(vec![PathBuf::from("C:\\")], vec![]);

        assert!(
            state.dialogs.is_empty(),
            "an empty trash should not open a browser with nothing to show"
        );
        assert!(
            result
                .task_panel_logs
                .iter()
                .any(|l| l.contains("Trash is already empty")),
            "should still tell the user something happened, got: {:?}",
            result.task_panel_logs
        );
    }
}
