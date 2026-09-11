//! Phase 7.21-B: blocking filesystem work leaves `update_state` and the input thread.
//!
//! Each of these used to stat or enumerate from inside a transition or an Enter
//! handler, where an unreachable network path blocks the UI for a full timeout. Each is
//! now a job whose completion does what the synchronous code did.

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec, OpResult, PipeToAction, SuccessData};
    use crate::model::dialog::{DialogContent, DriveInfo, DriveType};
    use crate::model::{Location, ReversalAction};
    use crate::pipe_to_action::PipeToActionResult;
    use crate::state::{update_state, AppState, Transition};
    use crate::test_utils::test_state;
    use std::path::PathBuf;

    fn complete(
        state: &mut AppState,
        spec: &JobSpec,
        result: OpResult,
    ) -> crate::state::StateUpdateResult {
        state.jobs.start_job(spec.clone());
        update_state(
            state,
            Transition::CompleteJob {
                job_id: spec.id,
                result,
            },
        )
    }

    fn drive_dialog(state: &AppState) -> &crate::model::dialog::DriveSelectionDialog {
        match &state.dialogs.current().expect("drive dialog").content {
            DialogContent::DriveSelection(d) => d,
            other => panic!("expected DriveSelection, got {other:?}"),
        }
    }

    // ── drive list ───────────────────────────────────────────────────────────

    #[test]
    fn the_drive_dialog_opens_at_once_and_lists_drives_on_a_worker() {
        let mut state = test_state();

        let result = update_state(&mut state, Transition::ShowDriveChangeDialog);

        let job = result
            .jobs_to_start
            .iter()
            .find(|j| matches!(j.kind, JobKind::ListDrives))
            .cloned()
            .expect("the drives are listed by a job");
        assert_eq!(drive_dialog(&state).loading_job_id, Some(job.id));
        let before = drive_dialog(&state).drives.len();

        complete(
            &mut state,
            &job,
            OpResult::Success(SuccessData::Drives(vec![DriveInfo {
                path: "Z:\\".to_string(),
                label: "Zed".to_string(),
                drive_type: DriveType::Local,
                total_space: None,
                free_space: None,
            }])),
        );

        let dialog = drive_dialog(&state);
        assert_eq!(dialog.drives.len(), before + 1);
        assert_eq!(dialog.loading_job_id, None);
    }

    #[test]
    fn a_failed_drive_listing_still_ends_the_listing_row_without_a_modal() {
        let mut state = test_state();
        let result = update_state(&mut state, Transition::ShowDriveChangeDialog);
        let job = result
            .jobs_to_start
            .into_iter()
            .find(|j| matches!(j.kind, JobKind::ListDrives))
            .expect("job");

        complete(&mut state, &job, OpResult::Failed("boom".to_string()));

        assert_eq!(state.dialogs.stack.len(), 1, "no error dialog on top");
        assert_eq!(drive_dialog(&state).loading_job_id, None);
    }

    // ── PipeToAction target ─────────────────────────────────────────────────

    fn custom_function(action: PipeToAction) -> JobSpec {
        JobSpec::new(JobKind::ExecuteCustomFunction {
            command: "echo".to_string(),
            working_dir: Location::Local(PathBuf::from("/work")),
            pipe_to_action: Some(action),
            shell: None,
            suspend: false,
        })
    }

    #[test]
    fn a_printed_path_is_resolved_on_a_worker_then_navigated_to() {
        let mut state = test_state();
        let command = custom_function(PipeToAction::JumpToPath);

        let finished = complete(
            &mut state,
            &command,
            OpResult::Success(SuccessData::CustomFunctionOutput(
                "\"/some/dir\"\n".to_string(),
            )),
        );

        let resolve = finished
            .jobs_to_start
            .iter()
            .find(|j| matches!(j.kind, JobKind::ResolvePipeTarget { .. }))
            .cloned()
            .expect("the path is resolved by a job, not stat-ed in update_state");
        assert_eq!(
            resolve.kind,
            JobKind::ResolvePipeTarget {
                action: PipeToAction::JumpToPath,
                output: "/some/dir".to_string(),
                working_dir: PathBuf::from("/work"),
            },
            "trimmed and unquoted exactly as before"
        );

        let target = Location::Local(PathBuf::from("/some/dir"));
        let resolved = complete(
            &mut state,
            &resolve,
            OpResult::Success(SuccessData::PipeTarget(PipeToActionResult::JumpToPath {
                location: target.clone(),
                cursor_name: None,
            })),
        );

        let read = resolved
            .jobs_to_start
            .iter()
            .find(|j| matches!(j.kind, JobKind::ReadDirectory { .. }))
            .expect("the pane reads the target");
        assert_eq!(state.active_pane().current_location, target);
        assert_eq!(state.active_pane().active_job_id, Some(read.id));
    }

    #[test]
    fn clip_text_output_goes_straight_to_the_clipboard() {
        let mut state = test_state();
        let command = custom_function(PipeToAction::ClipText);

        let finished = complete(
            &mut state,
            &command,
            OpResult::Success(SuccessData::CustomFunctionOutput("hello\n".to_string())),
        );

        assert!(finished.jobs_to_start.iter().any(|j| j.kind
            == JobKind::SetClipboard {
                text: "hello".to_string()
            }));
        assert!(!finished
            .jobs_to_start
            .iter()
            .any(|j| matches!(j.kind, JobKind::ResolvePipeTarget { .. })));
    }

    #[test]
    fn a_target_that_does_not_resolve_is_logged_not_raised() {
        let mut state = test_state();
        let resolve = JobSpec::new(JobKind::ResolvePipeTarget {
            action: PipeToAction::JumpToPath,
            output: "/nope".to_string(),
            working_dir: PathBuf::from("/work"),
        });

        let result = complete(
            &mut state,
            &resolve,
            OpResult::Failed("Path does not exist: /nope".to_string()),
        );

        assert!(state.dialogs.is_empty());
        assert!(result
            .task_panel_logs
            .iter()
            .any(|l| l.contains("PipeToAction error") && l.contains("/nope")));
    }

    // ── Undo/Redo pre-flight ────────────────────────────────────────────────

    fn delete(path: &str) -> ReversalAction {
        ReversalAction::Delete {
            target: Location::Local(PathBuf::from(path)),
            recreate: None,
        }
    }

    #[test]
    fn a_clean_preflight_starts_the_reversal_under_its_own_name() {
        let mut state = test_state();
        let preflight = JobSpec::new(JobKind::PreflightReversal {
            actions: vec![delete("/x/copy.txt")],
            operation_name: "Copy".to_string(),
            resulting_is_undo: true,
        });

        let result = complete(
            &mut state,
            &preflight,
            OpResult::Success(SuccessData::ReversalPreflight {
                ready: vec![delete("/x/copy.txt")],
                blocked: Vec::new(),
            }),
        );

        let job = result
            .jobs_to_start
            .iter()
            .find(|j| matches!(j.kind, JobKind::ExecuteReversal { .. }))
            .expect("the reversal starts");
        assert_eq!(
            state
                .background_jobs
                .get_job(job.id)
                .map(|j| j.name.as_str()),
            Some("Undo Copy")
        );
        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn a_preflight_with_blocked_rows_asks_before_running_the_rest() {
        let mut state = test_state();
        let preflight = JobSpec::new(JobKind::PreflightReversal {
            actions: vec![delete("/x/a.txt"), delete("/x/b.txt")],
            operation_name: "Delete".to_string(),
            resulting_is_undo: true,
        });

        let result = complete(
            &mut state,
            &preflight,
            OpResult::Success(SuccessData::ReversalPreflight {
                ready: vec![delete("/x/a.txt")],
                blocked: vec![(delete("/x/b.txt"), "/x/b.txt no longer exists".to_string())],
            }),
        );

        assert!(!result
            .jobs_to_start
            .iter()
            .any(|j| matches!(j.kind, JobKind::ExecuteReversal { .. })));
        let DialogContent::Confirmation(d) = &state.dialogs.current().expect("summary").content
        else {
            panic!("expected the blocked-rows summary");
        };
        assert!(
            d.message.contains("1 of 2 rows can be undone"),
            "{}",
            d.message
        );
        assert!(
            d.message.contains("/x/b.txt no longer exists"),
            "{}",
            d.message
        );
    }
}
