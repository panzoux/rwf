//! Integration tests for error handling
//!
//! Tests error dialog display and error logging for various failure scenarios

#[cfg(test)]
mod tests {
    use crate::job::{JobKind, JobSpec, OpResult};
    use crate::model::{Dialog, DialogContent, ErrorType, Location};
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

        if let DialogContent::Error {
            error_type,
            details,
            ..
        } = &dialog.content
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

        if let DialogContent::Error {
            error_type,
            message,
            ..
        } = &dialog.content
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

        if let DialogContent::Error { error_type, .. } = &dialog.content {
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

        if let DialogContent::Error {
            error_type,
            message,
            ..
        } = &dialog.content
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
        if let DialogContent::Error { details, .. } = &error_with_details.content {
            assert_eq!(details.as_ref().unwrap(), "Additional details");
        } else {
            panic!("Expected Error dialog content");
        }

        // Test permission error
        let perm_error = Dialog::permission_error("Cannot access file");
        assert_eq!(perm_error.title, "Permission Denied");
        if let DialogContent::Error { error_type, .. } = &perm_error.content {
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
}
