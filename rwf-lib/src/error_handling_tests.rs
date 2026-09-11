//! Integration tests for error handling
//!
//! Tests error dialog display and error logging for various failure scenarios

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec, OpResult};
    use crate::model::{Dialog, DialogContent, ErrorDialog, ErrorType, Location};
    use crate::state::{update_state, Transition};
    use crate::test_utils::test_state;
    use std::path::PathBuf;

    #[test]
    fn test_permission_error_shows_dialog() {
        let mut state = test_state();

        // Create a job
        let job_spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/root/protected")),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);

        // Complete job with permission error
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("Permission denied".to_string()),
            },
        );

        // Verify error dialog was shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "Permission Denied");

        if let DialogContent::Error(ErrorDialog {
            error_type,
            details,
            ..
        }) = &dialog.content
        {
            assert_eq!(*error_type, ErrorType::Permission);
            assert!(details.as_ref().unwrap().contains("elevated privileges"));
        } else {
            panic!("Expected Error dialog content");
        }
    }

    #[test]
    fn test_file_not_found_error_shows_dialog() {
        let mut state = test_state();

        // Create a copy job
        let job_spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/nonexistent/file.txt"))],
            dest: Location::Local(PathBuf::from("/tmp")),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);

        // Complete job with file not found error
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("File not found: /nonexistent/file.txt".to_string()),
            },
        );

        // Verify error dialog was shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "File Not Found");

        if let DialogContent::Error(ErrorDialog {
            error_type,
            message,
            ..
        }) = &dialog.content
        {
            assert_eq!(*error_type, ErrorType::FileNotFound);
            assert!(message.contains("Copy failed"));
        } else {
            panic!("Expected Error dialog content");
        }
    }

    #[test]
    fn test_invalid_path_error_shows_dialog() {
        let mut state = test_state();

        // Create a mkdir job
        let job_spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(PathBuf::from("/tmp/invalid\0name")),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);

        // Complete job with invalid path error
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("Invalid path: contains null character".to_string()),
            },
        );

        // Verify error dialog was shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "Invalid Path");

        if let DialogContent::Error(ErrorDialog { error_type, .. }) = &dialog.content {
            assert_eq!(*error_type, ErrorType::InvalidPath);
        } else {
            panic!("Expected Error dialog content");
        }
    }

    #[test]
    fn test_operation_failed_error_shows_dialog() {
        let mut state = test_state();

        // Create a delete job
        let job_spec = JobSpec::new(JobKind::Delete {
            targets: vec![Location::Local(PathBuf::from("/tmp/file.txt"))],
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);

        // Complete job with generic error
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("Disk full".to_string()),
            },
        );

        // Verify error dialog was shown
        assert!(!state.dialogs.is_empty());
        let dialog = state.dialogs.current().unwrap();
        assert_eq!(dialog.title, "Operation Failed");

        if let DialogContent::Error(ErrorDialog {
            error_type,
            message,
            ..
        }) = &dialog.content
        {
            assert_eq!(*error_type, ErrorType::OperationFailed);
            assert!(message.contains("Delete failed"));
            assert!(message.contains("Disk full"));
        } else {
            panic!("Expected Error dialog content");
        }
    }

    #[test]
    fn test_error_dialog_can_be_dismissed() {
        let mut state = test_state();

        // Show error dialog
        let error_dialog = Dialog::error("Test error");
        state.dialogs.push(error_dialog);

        assert!(!state.dialogs.is_empty());

        // Dismiss dialog
        update_state(&mut state, Transition::CloseDialog);

        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn test_multiple_errors_stack_dialogs() {
        let mut state = test_state();

        // Show first error dialog directly
        let error1 = Dialog::error("Error 1");
        state.dialogs.push(error1);

        assert_eq!(state.dialogs.stack.len(), 1);

        // Show second error dialog
        let error2 = Dialog::error("Error 2");
        state.dialogs.push(error2);

        // Verify both error dialogs are stacked
        assert_eq!(state.dialogs.stack.len(), 2);
    }

    #[test]
    fn test_error_dialog_helper_methods() {
        // Test error dialog creation
        let error = Dialog::error("Simple error");
        assert_eq!(error.title, "Error");

        // Test error with details
        let error_with_details = Dialog::error_with_details("Error message", "Additional details");
        if let DialogContent::Error(ErrorDialog { details, .. }) = &error_with_details.content {
            assert_eq!(details.as_ref().unwrap(), "Additional details");
        } else {
            panic!("Expected Error dialog content");
        }

        // Test permission error
        let perm_error = Dialog::permission_error("Cannot access file");
        assert_eq!(perm_error.title, "Permission Denied");
        if let DialogContent::Error(ErrorDialog { error_type, .. }) = &perm_error.content {
            assert_eq!(*error_type, ErrorType::Permission);
        } else {
            panic!("Expected Error dialog content");
        }

        // Test file not found error
        let not_found = Dialog::file_not_found_error("/path/to/file");
        assert_eq!(not_found.title, "File Not Found");

        // Test invalid path error
        let invalid = Dialog::invalid_path_error("/invalid\0path");
        assert_eq!(invalid.title, "Invalid Path");
    }

    #[test]
    fn test_from_job_failure_detects_error_types() {
        // Test permission detection
        let perm_dialog = Dialog::from_job_failure("Copy", "Permission denied");
        assert_eq!(perm_dialog.title, "Permission Denied");

        let access_dialog = Dialog::from_job_failure("Move", "Access denied");
        assert_eq!(access_dialog.title, "Permission Denied");

        // Test file not found detection
        let not_found_dialog = Dialog::from_job_failure("Delete", "File not found");
        assert_eq!(not_found_dialog.title, "File Not Found");

        // Test invalid path detection
        let invalid_dialog = Dialog::from_job_failure("Mkdir", "Invalid path");
        assert_eq!(invalid_dialog.title, "Invalid Path");

        // Test generic error
        let generic_dialog = Dialog::from_job_failure("Rename", "Disk full");
        assert_eq!(generic_dialog.title, "Operation Failed");
    }

    #[test]
    fn test_successful_job_does_not_show_error_dialog() {
        let mut state = test_state();

        // Create a job
        let job_spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(PathBuf::from("/tmp/newdir")),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);

        // Complete job successfully
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(crate::job::SuccessData::None),
            },
        );

        // Verify no error dialog was shown
        assert!(state.dialogs.is_empty());
    }

    #[test]
    fn test_cancelled_job_does_not_show_error_dialog() {
        let mut state = test_state();

        // Create a job
        let job_spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(PathBuf::from("/file.txt"))],
            dest: Location::Local(PathBuf::from("/tmp")),
        });
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);

        // Cancel job
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Cancelled,
            },
        );

        // Verify no error dialog was shown
        assert!(state.dialogs.is_empty());
    }

    /// A failed job that belongs to a specific pane must say *which* pane in the
    /// dialog. Reported from a diagnostic bundle: with five tabs restored at
    /// startup and one dead network share, the modal read
    /// "Read directory failed: Failed to read directory \\host\share: ..." —
    /// the path was there only because the backend happens to put it in its
    /// `with_context` string, and nothing said which of the ten panes had asked.
    /// `JobSpec::requesting_pane` carries exactly that and was being discarded.
    #[test]
    fn job_failure_dialog_names_the_tab_and_pane_that_asked() {
        let mut state = test_state();
        // Two more tabs so the reported number is a position, not always "Tab 1".
        state.tabs.tabs.push(crate::model::TabState::new(42));
        state.tabs.tabs.push(crate::model::TabState::new(99));

        // Deliberately not ReadDirectory: a failed *pane read* defers its modal to
        // `ResolveFallbackPath` (see the two tests below). Every other kind still
        // raises immediately, and must name the pane just the same.
        let job_spec = JobSpec::new(JobKind::Delete {
            targets: vec![Location::Local(PathBuf::from("/mnt/share/doomed.txt"))],
        })
        .with_requesting_pane(99, crate::model::ActivePane::Right);
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);

        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("network name not resolved".to_string()),
            },
        );

        let dialog = state
            .dialogs
            .current()
            .expect("failure should show a dialog");
        let DialogContent::Error(ErrorDialog { message, .. }) = &dialog.content else {
            panic!("Expected Error dialog content");
        };
        assert!(
            message.contains("Tab 3") && message.contains("right pane"),
            "dialog should name the third tab's right pane, got: {message}"
        );
        assert!(
            message.contains("network name not resolved"),
            "dialog must still carry the underlying error, got: {message}"
        );
    }

    /// A failed `ReadDirectory` must release the pane's `active_job_id`, not just
    /// its `is_loading` flag. The failure arm cleared only the latter, so the
    /// diagnostic bundle for the dead-share report showed the pane as
    /// `is_loading: false` with a job id still attached — a state
    /// `docs/DIAGNOSTIC_BUNDLES.md` teaches readers to read as "still working".
    #[test]
    fn failed_read_directory_releases_the_panes_job_id() {
        let mut state = test_state();
        let tab_id = state.tabs.tabs[0].id;

        let job_spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/mnt/share")),
        })
        .with_requesting_pane(tab_id, crate::model::ActivePane::Right);
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.tabs.tabs[0].right_pane.is_loading = true;
        state.tabs.tabs[0].right_pane.active_job_id = Some(job_spec.id);
        state.jobs.start_job(job_spec);

        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("network name not resolved".to_string()),
            },
        );

        let pane = &state.tabs.tabs[0].right_pane;
        assert!(!pane.is_loading, "a failed read must stop the spinner");
        assert_eq!(
            pane.active_job_id, None,
            "a failed read must release the pane's job id"
        );
    }

    /// A failed pane read no longer raises a modal on the spot: it asks
    /// `ResolveFallbackPath` (a worker job) whether somewhere readable exists first.
    /// Before this, the only way to know a saved path had vanished was a synchronous
    /// `exists()` walk in `session::restore_tabs`, which ran before the first frame
    /// and froze startup for the full network timeout on an unreachable host.
    #[test]
    fn failed_pane_read_asks_for_a_fallback_before_raising_a_dialog() {
        let mut state = test_state();
        let tab_id = state.tabs.tabs[0].id;

        let job_spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/mnt/share")),
        })
        .with_requesting_pane(tab_id, crate::model::ActivePane::Right);
        let job_id = state.jobs.enqueue(job_spec.clone());
        state.jobs.start_job(job_spec);

        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("network name not resolved".to_string()),
            },
        );

        assert!(
            state.dialogs.is_empty(),
            "no modal until the fallback search has answered"
        );
        let fallback = result
            .jobs_to_start
            .iter()
            .find(|j| matches!(j.kind, JobKind::ResolveFallbackPath { .. }))
            .expect("a fallback job must be started");
        assert_eq!(
            fallback.requesting_pane,
            Some((tab_id, crate::model::ActivePane::Right)),
            "the fallback must stay attached to the pane that asked"
        );
        assert_eq!(
            state
                .pending_read_failures
                .get(&fallback.id)
                .map(String::as_str),
            Some("network name not resolved"),
            "the original error must survive the round trip"
        );
    }

    /// …and when nothing in the chain is readable either, the modal finally appears,
    /// still naming the pane and the original error rather than the fallback's.
    #[test]
    fn fallback_finding_nothing_raises_the_original_error() {
        let mut state = test_state();
        let tab_id = state.tabs.tabs[0].id;

        let fallback = JobSpec::new(JobKind::ResolveFallbackPath {
            requested: Location::Local(PathBuf::from("/mnt/share")),
        })
        .with_requesting_pane(tab_id, crate::model::ActivePane::Right);
        let job_id = state.jobs.enqueue(fallback.clone());
        state
            .pending_read_failures
            .insert(fallback.id, "network name not resolved".to_string());
        state.jobs.start_job(fallback);

        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(crate::job::SuccessData::FallbackPath(None)),
            },
        );

        let dialog = state.dialogs.current().expect("a dialog must appear now");
        let DialogContent::ReadFailure(d) = &dialog.content else {
            panic!(
                "Expected ReadFailure dialog content, got {:?}",
                dialog.content
            );
        };
        assert_eq!(d.where_label, "Tab 1, right pane", "still names the pane");
        assert_eq!(
            (d.tab_id, d.side),
            (tab_id, crate::model::ActivePane::Right)
        );
        assert!(
            d.cause.contains("network name not resolved"),
            "reports the original read failure, not the fallback's, got: {}",
            d.cause
        );
        assert!(
            state.pending_read_failures.is_empty(),
            "the stashed error must be consumed"
        );
    }

    /// Run a pane read that fails and whose fallback search finds nothing, returning
    /// the state with the resulting dialog (if any) on the stack.
    fn read_fails_and_nothing_survives(
        origin: crate::job::JobOrigin,
        error: &str,
    ) -> crate::state::AppState {
        let mut state = test_state();
        let tab_id = state.tabs.tabs[0].id;
        let read = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(PathBuf::from("/mnt/share")),
        })
        .with_requesting_pane(tab_id, crate::model::ActivePane::Left)
        .with_origin(origin);
        let read_id = state.jobs.enqueue(read.clone());
        state.jobs.start_job(read);
        let failed = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: read_id,
                result: OpResult::Failed(error.to_string()),
            },
        );
        let fallback = failed
            .jobs_to_start
            .into_iter()
            .find(|j| matches!(j.kind, JobKind::ResolveFallbackPath { .. }))
            .expect("fallback job");
        assert_eq!(
            fallback.origin, origin,
            "the fallback must carry the read's origin, or the dialog cannot say when"
        );
        let fallback_id = state.jobs.enqueue(fallback.clone());
        state.jobs.start_job(fallback);
        update_state(
            &mut state,
            Transition::CompleteJob {
                job_id: fallback_id,
                result: OpResult::Success(crate::job::SuccessData::FallbackPath(None)),
            },
        );
        state
    }

    /// Phase 7.22 §3.3: at startup the user has operated nothing, and the dialog must
    /// say so rather than imply an operation of theirs failed.
    #[test]
    fn a_read_failing_during_session_restore_says_so() {
        let state = read_fails_and_nothing_survives(
            crate::job::JobOrigin::SessionRestore,
            "network name not resolved",
        );
        let DialogContent::ReadFailure(d) = &state.dialogs.current().expect("dialog").content
        else {
            panic!("expected ReadFailure");
        };
        assert_eq!(
            d.context_line(),
            "Tab 1, left pane — while restoring your session"
        );
    }

    /// Phase 7.22 §3.2, the reported bug: the OS text is localized, so the old
    /// substring match titled every failure on a Japanese Windows "Operation Failed".
    #[cfg(windows)]
    #[test]
    fn a_localized_network_error_gets_a_real_title() {
        // The backend's context names the same path the pane asked for.
        let path = Location::Local(PathBuf::from("/mnt/share")).display_path();
        let message = format!(
            "Failed to read directory {path}: ネットワーク名が見つかりません。 (os error 67)"
        );
        let generic = Dialog::from_job_failure("Copy", &message);
        assert_eq!(generic.title, "Network Location Unavailable");

        let state = read_fails_and_nothing_survives(crate::job::JobOrigin::UserAction, &message);
        let dialog = state.dialogs.current().expect("dialog");
        assert_eq!(dialog.title, "Directory Unavailable");
        let DialogContent::ReadFailure(d) = &dialog.content else {
            panic!("expected ReadFailure");
        };
        assert_eq!(
            d.cause, "ネットワーク名が見つかりません。 (os error 67)",
            "the localized text stays verbatim — it is what the user searches for"
        );
    }

    /// `[Retry]` re-reads the pane in the tab that failed, which need not be the
    /// active one, and puts that pane back into its loading state.
    #[test]
    fn retry_rereads_the_failed_pane_in_its_own_tab() {
        let mut state = test_state();
        state.tabs.create_tab();
        let other = state.tabs.tabs[1].id;
        state.tabs.tabs[1].right_pane.current_location =
            Location::Local(PathBuf::from("/mnt/share"));

        let result = update_state(
            &mut state,
            Transition::RetryPaneRead {
                tab_id: other,
                side: crate::model::ActivePane::Right,
            },
        );

        let job = result.jobs_to_start.first().expect("a read must start");
        assert_eq!(
            job.kind,
            JobKind::ReadDirectory {
                location: Location::Local(PathBuf::from("/mnt/share"))
            }
        );
        assert_eq!(
            job.requesting_pane,
            Some((other, crate::model::ActivePane::Right))
        );
        let pane = &state.tabs.tabs[1].right_pane;
        assert!(pane.is_loading);
        assert_eq!(
            pane.active_job_id,
            Some(job.id),
            "ReadDirectory contract: the pane must own the job or it loads forever"
        );
    }

    #[test]
    fn retry_for_a_tab_that_has_closed_does_nothing() {
        let mut state = test_state();
        let result = update_state(
            &mut state,
            Transition::RetryPaneRead {
                tab_id: 9999,
                side: crate::model::ActivePane::Left,
            },
        );
        assert!(result.jobs_to_start.is_empty());
    }

    /// A tab closed while the fallback search ran has no pane to retry into: the
    /// failure goes to the task panel instead of a modal nobody can act on.
    #[test]
    fn a_fallback_for_a_closed_tab_logs_instead_of_raising_a_dialog() {
        let mut state = test_state();
        let fallback = JobSpec::new(JobKind::ResolveFallbackPath {
            requested: Location::Local(PathBuf::from("/mnt/share")),
        })
        .with_requesting_pane(9999, crate::model::ActivePane::Right);
        let job_id = state.jobs.enqueue(fallback.clone());
        state
            .pending_read_failures
            .insert(fallback.id, "network name not resolved".to_string());
        state.jobs.start_job(fallback);

        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(crate::job::SuccessData::FallbackPath(None)),
            },
        );

        assert!(state.dialogs.is_empty());
        assert!(
            result
                .task_panel_logs
                .iter()
                .any(|l| l.contains("[FAIL]") && l.contains("network name not resolved")),
            "{:?}",
            result.task_panel_logs
        );
    }
}
