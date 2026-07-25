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
    fn no_match_in_either_config_falls_through_to_content_type_detection() {
        // Phase 7.3 §6: EnterDirectory no longer jumps straight to the text
        // viewer when neither config matches — it detects content type first
        // (CheckFallbackFileType) so binary files aren't force-fed into the
        // viewer. See the fallback_open_* tests below for what happens once
        // that detection job completes.
        let mut state = test_state();
        state.extension_associations = Vec::new();
        state.file_type_map = Vec::new();
        let entry = FileEntryBuilder::new("notes.md").dir(false).build();
        let expected_location = entry.location.clone();
        state.current_tab_mut().left_pane.raw_entries = vec![entry.clone()];
        state.current_tab_mut().left_pane.entries = vec![entry];
        state.current_tab_mut().left_pane.cursor = 0;

        let transitions = action_to_transitions(&state, &Action::EnterDirectory);
        assert_eq!(transitions.len(), 1);
        match &transitions[0] {
            Transition::CheckFallbackFileType { location } => {
                assert_eq!(location, &expected_location);
            }
            other => panic!("expected CheckFallbackFileType, got {:?}", other),
        }
    }

    #[test]
    fn check_fallback_file_type_transition_starts_detect_job() {
        let mut state = test_state();
        let location = Location::Local(PathBuf::from("/test/notes.md"));

        let result = update_state(
            &mut state,
            Transition::CheckFallbackFileType {
                location: location.clone(),
            },
        );

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::DetectFileType { path, purpose } => {
                assert_eq!(path, &PathBuf::from("/test/notes.md"));
                match purpose {
                    DetectFileTypePurpose::FallbackOpen {
                        location: purpose_location,
                    } => {
                        assert_eq!(purpose_location, &location);
                    }
                    other => panic!("expected FallbackOpen, got {:?}", other),
                }
            }
            other => panic!("expected DetectFileType, got {:?}", other),
        }
    }

    /// Drive a `DetectFileType { purpose: FallbackOpen }` job through the real
    /// job-manager lifecycle (enqueue -> start -> complete), mirroring
    /// `run_detect_file_type_job` above but for the fallback-open purpose.
    fn run_fallback_open_job(
        state: &mut crate::AppState,
        location: Location,
        detected: DetectedKind,
    ) -> crate::state::StateUpdateResult {
        let path: PathBuf = location.display_path().into();
        let job_spec = JobSpec::new(JobKind::DetectFileType {
            path,
            purpose: DetectFileTypePurpose::FallbackOpen {
                location: location.clone(),
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

    /// A known non-text kind (PNG magic bytes, but no registered association or
    /// mapping for the extension) opens via the OS default association instead
    /// of the internal text viewer.
    #[test]
    fn fallback_open_known_binary_kind_opens_with_system() {
        use std::io::Write;
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let file_path = temp_dir.path().join("mystery.xyz");
        let mut f = std::fs::File::create(&file_path).expect("create file");
        // Real PNG magic bytes, matching the Task-1 job_executor.rs precedent.
        f.write_all(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
            .expect("write png signature");
        drop(f);

        let mut state = test_state();
        let location = Location::Local(file_path.clone());

        let result = run_fallback_open_job(&mut state, location, DetectedKind::Png);

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::SpawnProcess { .. } => {}
            other => panic!("expected SpawnProcess (OS default open), got {:?}", other),
        }
        assert!(result
            .task_panel_logs
            .iter()
            .any(|l| l.contains("[System]") && l.contains("mystery.xyz")));
    }

    /// Plain-text content (detected as Unknown) falls through to the internal
    /// text viewer exactly as it did before this task's change.
    #[test]
    fn fallback_open_unknown_kind_opens_text_viewer() {
        use std::io::Write;
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let file_path = temp_dir.path().join("notes.xyz");
        let mut f = std::fs::File::create(&file_path).expect("create file");
        f.write_all(b"just some plain text, nothing magic here\n")
            .expect("write text content");
        drop(f);

        let mut state = test_state();
        let location = Location::Local(file_path.clone());

        let result = run_fallback_open_job(&mut state, location.clone(), DetectedKind::Unknown);

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::LoadFileForViewer { location: loc, .. } => {
                assert_eq!(loc, &location);
            }
            other => panic!("expected LoadFileForViewer, got {:?}", other),
        }
        assert_eq!(
            state.viewer.as_ref().map(|v| v.mode),
            Some(crate::model::ViewerMode::Text)
        );
    }

    /// Regression test (post-dabc032 review): if the detect job fails or is
    /// cancelled (e.g. the file became unreadable between listing and
    /// detection), FallbackOpen must NOT silently drop the open — it falls
    /// back to the text viewer, same as an Unknown detection result.
    #[test]
    fn fallback_open_failed_detection_still_opens_text_viewer() {
        let mut state = test_state();
        let location = Location::Local(PathBuf::from("/test/vanished.xyz"));

        let job_spec = JobSpec::new(JobKind::DetectFileType {
            path: PathBuf::from("/test/vanished.xyz"),
            purpose: DetectFileTypePurpose::FallbackOpen {
                location: location.clone(),
            },
        });
        update_state(&mut state, Transition::EnqueueJob { spec: job_spec });
        let job_id = state.jobs.queue[0].id;
        update_state(&mut state, Transition::StartNextJob);

        let result = update_state(
            &mut state,
            Transition::CompleteJob {
                job_id,
                result: OpResult::Failed("file vanished".to_string()),
            },
        );

        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::LoadFileForViewer { location: loc, .. } => {
                assert_eq!(loc, &location);
            }
            other => panic!("expected LoadFileForViewer, got {:?}", other),
        }
        assert_eq!(
            state.viewer.as_ref().map(|v| v.mode),
            Some(crate::model::ViewerMode::Text)
        );
    }

    /// Guard test (post-dabc032 review): a non-Local location (Archive, Ssh,
    /// Cloud) reaching CheckFallbackFileType must skip magic-byte detection
    /// entirely (its display_path() is a synthetic string, not something
    /// std::fs can read) and go straight to the text viewer, with NO detect
    /// job started.
    #[test]
    fn check_fallback_file_type_skips_detection_for_archive_location() {
        let mut state = test_state();
        let location = Location::Archive {
            archive_path: Box::new(Location::Local(PathBuf::from("/test/archive.zip"))),
            inner_path: PathBuf::from("inner/notes.txt"),
        };

        let result = update_state(
            &mut state,
            Transition::CheckFallbackFileType {
                location: location.clone(),
            },
        );

        // No DetectFileType job — went straight to the viewer instead.
        assert_eq!(result.jobs_to_start.len(), 1);
        match &result.jobs_to_start[0].kind {
            JobKind::LoadFileForViewer { location: loc, .. } => {
                assert_eq!(loc, &location);
            }
            other => panic!(
                "expected LoadFileForViewer directly (no detect job), got {:?}",
                other
            ),
        }
        assert_eq!(
            state.viewer.as_ref().map(|v| v.mode),
            Some(crate::model::ViewerMode::Text)
        );
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
