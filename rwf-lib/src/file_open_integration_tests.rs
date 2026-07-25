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
            Transition::ShowOpenWithPicker { candidates, paths } => {
                assert_eq!(candidates.len(), 2);
                assert_eq!(paths, &vec![PathBuf::from(expected_path)]);
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
        let paths = vec![PathBuf::from("/test/server.log")];

        let result = update_state(
            &mut state,
            Transition::ShowOpenWithPicker {
                candidates: candidates.clone(),
                paths: paths.clone(),
            },
        );

        assert!(result.jobs_to_start.is_empty());
        assert_eq!(state.dialogs.stack.len(), 1);
        match &state.dialogs.current().expect("dialog pushed").content {
            DialogContent::OpenWithPicker(d) => {
                assert_eq!(d.candidates.len(), 2);
                assert_eq!(d.paths, paths);
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

    // ── Batch "Open With..." on marked files (Phase 7.3 §3, Task 4) ──────────

    #[test]
    fn action_open_with_two_marked_files_starts_batch() {
        let mut state = test_state();
        let entry_a = FileEntryBuilder::new("a.log").dir(false).build();
        let entry_b = FileEntryBuilder::new("b.log").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry_a.clone(), entry_b.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry_a.clone(), entry_b.clone()];
        state.current_tab_mut().left_pane.cursor = 0;
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(entry_a.location.clone());
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(entry_b.location.clone());

        let transitions = action_to_transitions(&state, &Action::OpenWith);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::StartBatchOpenWith { paths } => assert_eq!(paths.len(), 2),
            other => panic!("expected StartBatchOpenWith, got {:?}", other),
        }
    }

    /// Exactly one marked file must NOT trigger the batch flow — it falls
    /// through to the ordinary cursor-file flow, same as zero marked
    /// (matches the `Action::Copy` marked-file convention).
    #[test]
    fn action_open_with_one_marked_file_uses_single_file_flow() {
        let mut state = test_state();
        state.extension_associations = vec![ExtensionAssociation {
            extension: "log".to_string(),
            command: "less $F".to_string(),
            description: None,
            shell: None,
        }];
        let entry = FileEntryBuilder::new("server.log").dir(false).build();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.cursor = 0;
        state
            .current_tab_mut()
            .left_pane
            .marking
            .mark(entry.location.clone());

        let transitions = action_to_transitions(&state, &Action::OpenWith);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::ExecuteAssociationChecked { .. } => {}
            other => panic!(
                "expected ExecuteAssociationChecked (single marked = cursor flow), got {:?}",
                other
            ),
        }
    }

    #[test]
    fn start_batch_open_with_transition_starts_detect_batch_job() {
        let mut state = test_state();
        let paths = vec![PathBuf::from("/test/a.log"), PathBuf::from("/test/b.log")];

        let result = update_state(
            &mut state,
            Transition::StartBatchOpenWith {
                paths: paths.clone(),
            },
        );

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::DetectFileTypesBatch { paths: job_paths } => {
                assert_eq!(job_paths, &paths)
            }
            other => panic!("expected DetectFileTypesBatch, got {:?}", other),
        }
    }

    /// Drive a `DetectFileTypesBatch` job through the real job-manager lifecycle
    /// (enqueue -> start -> complete), mirroring `run_detect_file_type_job` above
    /// but for the batch job kind.
    fn run_detect_file_types_batch_job(
        state: &mut crate::AppState,
        paths: Vec<PathBuf>,
        detections: Vec<(PathBuf, DetectedKind)>,
    ) -> crate::state::StateUpdateResult {
        let job_spec = JobSpec::new(JobKind::DetectFileTypesBatch { paths });
        update_state(state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;
        update_state(state, Transition::StartNextJob);

        update_state(
            state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Success(SuccessData::FileTypesDetected(detections)),
            },
        )
    }

    #[test]
    fn batch_open_with_two_files_same_group_one_candidate_starts_two_jobs() {
        let mut state = test_state();
        state.config.magic_byte_detection_enabled = false; // simpler job-kind assertions
                                                           // No $F/$W/$E macro in the command: `expand_association_command` expands
                                                           // against the active pane's *cursor* entry, not per-group-file (Task 3
                                                           // never needed otherwise, since its single-file flow always targets the
                                                           // cursor). The batch flow reuses that same expansion once per group and
                                                           // applies it to every file in the group unchanged — a real, documented
                                                           // limitation (see the commit message / task report), not something this
                                                           // test is trying to exercise.
        state.extension_associations = vec![ExtensionAssociation {
            extension: "png".to_string(),
            command: "viewer".to_string(),
            description: None,
            shell: None,
        }];

        let paths = vec![PathBuf::from("/test/a.png"), PathBuf::from("/test/b.png")];
        let detections = vec![
            (paths[0].clone(), DetectedKind::Png),
            (paths[1].clone(), DetectedKind::Png),
        ];

        let result = run_detect_file_types_batch_job(&mut state, paths, detections);

        assert!(state.dialogs.is_empty());
        assert_eq!(result.jobs_to_start.len(), 2);
        for job in &result.jobs_to_start {
            match &job.kind {
                JobKind::ExecuteCustomFunction { command, .. } => {
                    assert_eq!(command, "viewer")
                }
                other => panic!("expected ExecuteCustomFunction, got {:?}", other),
            }
        }
    }

    /// Mixed batch: one group resolves to exactly one candidate (auto-runs as a
    /// job per file) and another resolves to 2+ candidates (pushes a picker for
    /// just that group's files) in the same `CompleteJob` result.
    #[test]
    fn batch_open_with_mixed_groups_produces_jobs_and_picker() {
        let mut state = test_state();
        state.config.magic_byte_detection_enabled = false;
        state.extension_associations = vec![
            ExtensionAssociation {
                extension: "png".to_string(),
                command: "viewer".to_string(),
                description: None,
                shell: None,
            },
            ExtensionAssociation {
                extension: "pdf".to_string(),
                command: "reader1".to_string(),
                description: None,
                shell: None,
            },
            ExtensionAssociation {
                extension: "pdf".to_string(),
                command: "reader2".to_string(),
                description: None,
                shell: None,
            },
        ];

        let png_path = PathBuf::from("/test/a.png");
        let pdf_path = PathBuf::from("/test/b.pdf");
        let paths = vec![png_path.clone(), pdf_path.clone()];
        let detections = vec![
            (png_path.clone(), DetectedKind::Png),
            (pdf_path.clone(), DetectedKind::Pdf),
        ];

        let result = run_detect_file_types_batch_job(&mut state, paths, detections);

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::ExecuteCustomFunction { command, .. } => assert_eq!(command, "viewer"),
            other => panic!("expected ExecuteCustomFunction, got {:?}", other),
        }

        assert_eq!(state.dialogs.stack.len(), 1);
        match &state.dialogs.current().expect("picker pushed").content {
            DialogContent::OpenWithPicker(d) => {
                assert_eq!(d.candidates.len(), 2);
                assert_eq!(d.paths, vec![pdf_path]);
            }
            other => panic!("expected OpenWithPicker dialog, got {:?}", other),
        }
    }

    /// A group with no matching `ExtensionAssociation` is skipped (no job, no
    /// picker) and logged to the task panel — Open With is association-only,
    /// same as the single-file `Action::OpenWith` returning nothing on no match.
    #[test]
    fn batch_open_with_no_candidates_skips_and_logs() {
        let mut state = test_state();
        state.extension_associations = Vec::new();

        let path = PathBuf::from("/test/a.zzz");
        let paths = vec![path.clone()];
        let detections = vec![(path.clone(), DetectedKind::Unknown)];

        let result = run_detect_file_types_batch_job(&mut state, paths, detections);

        assert!(result.jobs_to_start.is_empty());
        assert!(state.dialogs.is_empty());
        assert_eq!(result.task_panel_logs.len(), 1);
        assert!(result.task_panel_logs[0].contains("[Skipped]"));
        assert!(result.task_panel_logs[0].contains("zzz"));
    }
}
