//! Integration tests for the EnterDirectory precedence chain:
//! extension_associations.json → file_type_map.json → internal viewer.

#[cfg(test)]
mod tests {
    use crate::config::{ExtensionAssociation, FileOpenAction, FileTypeMapping};
    use crate::input::{action_to_transitions, Action};
    use crate::job::{DetectFileTypePurpose, JobKind, JobSpec, OpResult, SuccessData};
    use crate::magic::DetectedKind;
    use crate::model::dialog::DialogContent;
    use crate::model::Location;
    use crate::state::{update_state, Transition};
    use crate::test_utils::{test_state, FileEntryBuilder};
    use std::path::PathBuf;

    #[test]
    fn extension_association_match_produces_execute_association() {
        let mut state = test_state();
        state.extension_associations = vec![ExtensionAssociation {
            extension: "log".to_string(),
            command: "less $F".to_string(),
            description: None,
            shell: None,
        }];
        let entry = FileEntryBuilder::new("server.log").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ExecuteAssociationChecked { command, .. } => {
                assert!(command.contains("less"))
            }
            other => panic!("expected ExecuteAssociationChecked, got {:?}", other),
        }
    }

    #[test]
    fn two_extension_associations_show_open_with_picker() {
        let mut state = test_state();
        state.extension_associations = vec![
            ExtensionAssociation {
                extension: "log".to_string(),
                command: "less $F".to_string(),
                description: Some("View with less".to_string()),
                shell: None,
            },
            ExtensionAssociation {
                extension: "log".to_string(),
                command: "notepad $F".to_string(),
                description: Some("Edit with Notepad".to_string()),
                shell: None,
            },
        ];
        let entry = FileEntryBuilder::new("server.log").dir(false).build();
        let expected_path = entry.location.display_path();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ShowOpenWithPicker { candidates, path } => {
                assert_eq!(candidates.len(), 2);
                assert_eq!(path, &PathBuf::from(expected_path));
            }
            other => panic!("expected ShowOpenWithPicker, got {:?}", other),
        }
    }

    #[test]
    fn three_extension_associations_show_open_with_picker_with_all_candidates() {
        let mut state = test_state();
        state.extension_associations = vec![
            ExtensionAssociation {
                extension: "txt".to_string(),
                command: "cmd1 $F".to_string(),
                description: None,
                shell: None,
            },
            ExtensionAssociation {
                extension: "TXT".to_string(), // case-insensitive match
                command: "cmd2 $F".to_string(),
                description: None,
                shell: None,
            },
            ExtensionAssociation {
                extension: ".txt".to_string(), // leading dot tolerated
                command: "cmd3 $F".to_string(),
                description: None,
                shell: None,
            },
        ];
        let entry = FileEntryBuilder::new("notes.txt").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ShowOpenWithPicker { candidates, .. } => {
                assert_eq!(candidates.len(), 3);
            }
            other => panic!("expected ShowOpenWithPicker, got {:?}", other),
        }
    }

    #[test]
    fn action_open_with_no_association_returns_empty() {
        let mut state = test_state();
        state.extension_associations = Vec::new();
        let entry = FileEntryBuilder::new("notes.md").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::OpenWith);
        assert!(transitions.is_empty());
    }

    #[test]
    fn action_open_with_single_association_produces_execute_association_checked() {
        let mut state = test_state();
        state.extension_associations = vec![ExtensionAssociation {
            extension: "log".to_string(),
            command: "less $F".to_string(),
            description: None,
            shell: None,
        }];
        let entry = FileEntryBuilder::new("server.log").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::OpenWith);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ExecuteAssociationChecked { command, .. } => {
                assert!(command.contains("less"))
            }
            other => panic!("expected ExecuteAssociationChecked, got {:?}", other),
        }
    }

    #[test]
    fn action_open_with_multiple_associations_shows_picker() {
        let mut state = test_state();
        state.extension_associations = vec![
            ExtensionAssociation {
                extension: "log".to_string(),
                command: "less $F".to_string(),
                description: None,
                shell: None,
            },
            ExtensionAssociation {
                extension: "log".to_string(),
                command: "notepad $F".to_string(),
                description: None,
                shell: None,
            },
        ];
        let entry = FileEntryBuilder::new("server.log").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::OpenWith);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ShowOpenWithPicker { candidates, .. } => {
                assert_eq!(candidates.len(), 2);
            }
            other => panic!("expected ShowOpenWithPicker, got {:?}", other),
        }
    }

    #[test]
    fn show_open_with_picker_transition_pushes_dialog() {
        let mut state = test_state();
        assert!(state.dialogs.is_empty());

        let candidates = vec![
            ExtensionAssociation {
                extension: "log".to_string(),
                command: "less $F".to_string(),
                description: None,
                shell: None,
            },
            ExtensionAssociation {
                extension: "log".to_string(),
                command: "notepad $F".to_string(),
                description: None,
                shell: None,
            },
        ];
        let path = PathBuf::from("/test/server.log");

        let result = update_state(
            &mut state,
            Transition::ShowOpenWithPicker {
                candidates: candidates.clone(),
                path: path.clone(),
            },
        );

        assert!(result.jobs_to_start.is_empty());
        assert_eq!(state.dialogs.stack.len(), 1);
        match &state.dialogs.current().expect("dialog pushed").content {
            DialogContent::OpenWithPicker(d) => {
                assert_eq!(d.candidates.len(), 2);
                assert_eq!(d.path, path);
                assert_eq!(d.selected_index, 0);
            }
            other => panic!("expected OpenWithPicker dialog, got {:?}", other),
        }
    }

    #[test]
    fn no_extension_association_falls_through_to_file_type_map() {
        let mut state = test_state();
        // No extension_associations entries at all.
        state.extension_associations = Vec::new();
        state.file_type_map = vec![FileTypeMapping {
            extension: "png".to_string(),
            file_type: Some("image/png".to_string()),
            actions: vec![FileOpenAction::OsDefault],
        }];
        let entry = FileEntryBuilder::new("screenshot.png").dir(false).build();
        let expected_path = entry.location.display_path();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::OpenWithSystem { path } => assert_eq!(path, &expected_path),
            other => panic!("expected OpenWithSystem, got {:?}", other),
        }
    }

    #[test]
    fn no_match_in_either_config_falls_through_to_internal_viewer() {
        let mut state = test_state();
        state.extension_associations = Vec::new();
        state.file_type_map = Vec::new();
        let entry = FileEntryBuilder::new("notes.md").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::OpenTextViewer { .. } => {}
            other => panic!("expected OpenTextViewer, got {:?}", other),
        }
    }

    // ── ExecuteAssociationChecked gate (Phase 7.3 §5, magic-byte mismatch warning) ──

    #[test]
    fn checked_association_starts_detect_file_type_job_when_enabled() {
        let mut state = test_state();
        assert!(state.config.magic_byte_detection_enabled); // default is true

        let result = update_state(
            &mut state,
            Transition::ExecuteAssociationChecked {
                path: PathBuf::from("/test/notes.txt"),
                command: "notepad $F".to_string(),
                working_dir: Location::Local(PathBuf::from("/test")),
                shell: None,
            },
        );

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::DetectFileType { path, purpose } => {
                assert_eq!(path, &PathBuf::from("/test/notes.txt"));
                match purpose {
                    DetectFileTypePurpose::CheckAssociationMismatch { command, .. } => {
                        assert_eq!(command, "notepad $F");
                    }
                    other => panic!("expected CheckAssociationMismatch, got {:?}", other),
                }
            }
            other => panic!("expected DetectFileType, got {:?}", other),
        }
    }

    #[test]
    fn checked_association_skips_detection_when_disabled() {
        let mut state = test_state();
        state.config.magic_byte_detection_enabled = false;

        let result = update_state(
            &mut state,
            Transition::ExecuteAssociationChecked {
                path: PathBuf::from("/test/notes.txt"),
                command: "notepad $F".to_string(),
                working_dir: Location::Local(PathBuf::from("/test")),
                shell: None,
            },
        );

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::ExecuteCustomFunction { command, .. } => {
                assert_eq!(command, "notepad $F");
            }
            other => panic!("expected ExecuteCustomFunction, got {:?}", other),
        }
    }

    /// Drive a `DetectFileType` job through the real job-manager lifecycle
    /// (enqueue -> start -> complete), the same machinery production code uses,
    /// rather than hand-rolling the completion routing.
    fn run_detect_file_type_job(
        state: &mut crate::AppState,
        path: PathBuf,
        command: &str,
        detected: DetectedKind,
    ) -> crate::state::StateUpdateResult {
        let job_spec = JobSpec::new(JobKind::DetectFileType {
            path,
            purpose: DetectFileTypePurpose::CheckAssociationMismatch {
                command: command.to_string(),
                working_dir: Location::Local(PathBuf::from("/test")),
                shell: None,
            },
        });
        update_state(state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;
        update_state(state, Transition::StartNextJob);

        update_state(
            state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::FileTypeDetected(detected)),
            },
        )
    }

    #[test]
    fn detected_executable_mismatch_shows_warning_dialog() {
        let mut state = test_state();
        assert!(state.dialogs.is_empty());

        let result = run_detect_file_type_job(
            &mut state,
            PathBuf::from("/test/notes.txt"),
            "notepad $F",
            DetectedKind::Pe,
        );

        assert!(result.jobs_to_start.is_empty());
        assert_eq!(state.dialogs.stack.len(), 1);
        match &state.dialogs.current().expect("dialog pushed").content {
            DialogContent::TypeMismatchWarning(d) => {
                assert_eq!(d.command, "notepad $F");
                assert_eq!(d.detected, DetectedKind::Pe);
                assert_eq!(d.path, PathBuf::from("/test/notes.txt"));
            }
            other => panic!("expected TypeMismatchWarning dialog, got {:?}", other),
        }
    }

    #[test]
    fn detected_non_executable_auto_continues_without_dialog() {
        let mut state = test_state();

        let result = run_detect_file_type_job(
            &mut state,
            PathBuf::from("/test/notes.txt"),
            "notepad $F",
            DetectedKind::Png,
        );

        assert!(state.dialogs.is_empty());
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::ExecuteCustomFunction { command, .. } => {
                assert_eq!(command, "notepad $F");
            }
            other => panic!("expected ExecuteCustomFunction, got {:?}", other),
        }
    }

    #[test]
    fn detected_executable_with_matching_extension_auto_continues() {
        let mut state = test_state();

        let result = run_detect_file_type_job(
            &mut state,
            PathBuf::from("/test/setup.exe"),
            "run $F",
            DetectedKind::Pe,
        );

        assert!(state.dialogs.is_empty());
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::ExecuteCustomFunction { .. } => {}
            other => panic!("expected ExecuteCustomFunction, got {:?}", other),
        }
    }

    #[test]
    fn detected_mismatch_but_detection_disabled_auto_continues() {
        let mut state = test_state();
        state.config.magic_byte_detection_enabled = false;

        let result = run_detect_file_type_job(
            &mut state,
            PathBuf::from("/test/notes.txt"),
            "notepad $F",
            DetectedKind::Pe,
        );

        assert!(state.dialogs.is_empty());
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::ExecuteCustomFunction { .. } => {}
            other => panic!("expected ExecuteCustomFunction, got {:?}", other),
        }
    }
}
