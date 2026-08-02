//! Phase 7.7 Task 19: `JobKind::ScanTrash` completion must either push the
//! `EmptyTrash` confirm dialog (populated with real count/size) or, when the
//! trash is already empty, skip the dialog and just log that fact — asking
//! the user to confirm emptying nothing would be pure friction.

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec, OpResult, SuccessData};
    use crate::model::dialog::{ConfirmableAction, DialogContent};
    use crate::state::{update_state, StateUpdateResult, Transition};
    use crate::test_utils::test_state;
    use crate::AppState;
    use std::path::PathBuf;

    fn complete_scan_trash_job(
        fallback_roots: Vec<PathBuf>,
        count: usize,
        total_size: u64,
    ) -> (AppState, StateUpdateResult) {
        let mut state = test_state();
        let job_spec = JobSpec::new(JobKind::ScanTrash { fallback_roots });
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
                result: OpResult::Success(SuccessData::TrashScanned { count, total_size }),
            },
        );
        (state, result)
    }

    #[test]
    fn scan_trash_completion_pushes_confirm_dialog_with_stats_when_nonempty() {
        let (state, _result) = complete_scan_trash_job(vec![PathBuf::from("C:\\")], 3, 4096);

        let dialog = state
            .dialogs
            .current()
            .expect("a non-empty scan should push a confirm dialog");
        match &dialog.content {
            DialogContent::Confirmation(d) => {
                let stats = d.stats.expect("dialog should carry the scanned stats");
                assert_eq!(stats.count, 3);
                assert_eq!(stats.total_size, 4096);
                match &d.action {
                    ConfirmableAction::EmptyTrash { fallback_roots } => {
                        assert_eq!(fallback_roots, &vec![PathBuf::from("C:\\")]);
                    }
                    other => panic!("expected ConfirmableAction::EmptyTrash, got {other:?}"),
                }
            }
            other => panic!("expected a Confirmation dialog, got {other:?}"),
        }
    }

    #[test]
    fn scan_trash_completion_skips_dialog_and_logs_when_already_empty() {
        let (state, result) = complete_scan_trash_job(vec![PathBuf::from("C:\\")], 0, 0);

        assert!(
            state.dialogs.is_empty(),
            "an empty trash should not prompt for confirmation over nothing"
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
