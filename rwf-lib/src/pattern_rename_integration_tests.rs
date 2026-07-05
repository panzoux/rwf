//! Integration tests for pattern-based rename functionality (TWF-compatible)

use crate::job::{JobKind, OpResult, SuccessData};
use crate::model::{DialogContent, Location};
use crate::state::{update_state, Transition};
use crate::test_utils::{test_state, FileEntryBuilder};
use std::path::PathBuf;

#[test]
fn test_show_pattern_rename_dialog_with_marked_files() {
    let mut state = test_state();

    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntryBuilder::new("file1.txt")
            .path("/test/file1.txt")
            .build(),
        FileEntryBuilder::new("file2.txt")
            .path("/test/file2.txt")
            .build(),
    ];
    update_state(&mut state, Transition::MarkAll);
    update_state(&mut state, Transition::ShowPatternRenameDialog);

    assert!(!state.dialogs.is_empty());
    let dialog = state.dialogs.current().unwrap();
    assert!(matches!(
        dialog.content,
        DialogContent::PatternRename { .. }
    ));
}

#[test]
fn test_show_pattern_rename_dialog_with_cursor_file() {
    let mut state = test_state();

    let pane = state.active_pane_mut();
    pane.entries = vec![FileEntryBuilder::new("document.txt")
        .path("/test/document.txt")
        .build()];
    update_state(&mut state, Transition::ShowPatternRenameDialog);

    assert!(!state.dialogs.is_empty());
    let dialog = state.dialogs.current().unwrap();
    assert!(matches!(
        dialog.content,
        DialogContent::PatternRename { .. }
    ));
}

#[test]
fn test_update_pattern_rename_fields_regex() {
    let mut state = test_state();

    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntryBuilder::new("file1.txt")
            .path("/test/file1.txt")
            .build(),
        FileEntryBuilder::new("file2.txt")
            .path("/test/file2.txt")
            .build(),
    ];
    update_state(&mut state, Transition::MarkAll);
    update_state(&mut state, Transition::ShowPatternRenameDialog);

    // Regex: prepend "backup_"
    update_state(
        &mut state,
        Transition::UpdatePatternRenameFields {
            find: "^(.+)$".to_string(),
            replace: "backup_${1}".to_string(),
            use_regex: true,
            case_sensitive: true,
        },
    );

    let dialog = state.dialogs.current().unwrap();
    if let Some((find, replace, use_regex, case_sensitive, preview)) =
        dialog.content.as_pattern_rename()
    {
        assert_eq!(find, "^(.+)$");
        assert_eq!(replace, "backup_${1}");
        assert!(use_regex);
        assert!(case_sensitive);
        assert_eq!(preview.len(), 2);
        assert_eq!(
            preview[0],
            ("file1.txt".to_string(), "backup_file1.txt".to_string())
        );
        assert_eq!(
            preview[1],
            ("file2.txt".to_string(), "backup_file2.txt".to_string())
        );
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_execute_pattern_rename_creates_job() {
    let mut state = test_state();

    let pane = state.active_pane_mut();
    let loc1 = Location::Local(PathBuf::from("/test/file1.txt"));
    let loc2 = Location::Local(PathBuf::from("/test/file2.txt"));
    pane.entries = vec![
        FileEntryBuilder::new("file1.txt")
            .path("/test/file1.txt")
            .build(),
        FileEntryBuilder::new("file2.txt")
            .path("/test/file2.txt")
            .build(),
    ];
    update_state(&mut state, Transition::MarkAll);

    let result = update_state(
        &mut state,
        Transition::ExecutePatternRename {
            find: r"\.txt$".to_string(),
            replace: ".bak".to_string(),
            use_regex: true,
            case_sensitive: true,
            targets: vec![loc1, loc2],
        },
    );

    assert_eq!(result.jobs_to_start.len(), 1);
    let job_spec = &result.jobs_to_start[0];
    match &job_spec.kind {
        JobKind::PatternRename {
            targets,
            find,
            replace,
            use_regex,
            case_sensitive,
        } => {
            assert_eq!(targets.len(), 2);
            assert_eq!(find, r"\.txt$");
            assert_eq!(replace, ".bak");
            assert!(*use_regex);
            assert!(*case_sensitive);
        }
        _ => panic!("Expected PatternRename job kind"),
    }
    assert!(state.dialogs.is_empty());
}

#[test]
fn test_pattern_rename_plain_mode_replace() {
    let mut state = test_state();

    let pane = state.active_pane_mut();
    pane.entries = vec![FileEntryBuilder::new("hello_world.txt")
        .path("/test/hello_world.txt")
        .build()];
    update_state(&mut state, Transition::ShowPatternRenameDialog);

    // Plain mode: replace _ with -
    update_state(
        &mut state,
        Transition::UpdatePatternRenameFields {
            find: "_".to_string(),
            replace: "-".to_string(),
            use_regex: false,
            case_sensitive: true,
        },
    );

    let dialog = state.dialogs.current().unwrap();
    if let Some((_, _, _, _, preview)) = dialog.content.as_pattern_rename() {
        assert_eq!(preview.len(), 1);
        assert_eq!(
            preview[0],
            ("hello_world.txt".to_string(), "hello-world.txt".to_string())
        );
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_pattern_rename_all_files_in_preview() {
    // TWF behaviour: unchanged files still appear in the preview (just colored differently)
    let mut state = test_state();

    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntryBuilder::new("file1.txt")
            .path("/test/file1.txt")
            .build(),
        FileEntryBuilder::new("file2.pdf")
            .path("/test/file2.pdf")
            .build(),
    ];
    update_state(&mut state, Transition::MarkAll);
    update_state(&mut state, Transition::ShowPatternRenameDialog);

    // Only matches .txt files
    update_state(
        &mut state,
        Transition::UpdatePatternRenameFields {
            find: r"\.txt$".to_string(),
            replace: ".bak".to_string(),
            use_regex: true,
            case_sensitive: true,
        },
    );

    let dialog = state.dialogs.current().unwrap();
    if let Some((_, _, _, _, preview)) = dialog.content.as_pattern_rename() {
        // Both files appear in preview (TWF shows all)
        assert_eq!(preview.len(), 2);
        let txt = preview.iter().find(|(o, _)| o == "file1.txt").unwrap();
        assert_eq!(txt.1, "file1.bak");
        let pdf = preview.iter().find(|(o, _)| o == "file2.pdf").unwrap();
        assert_eq!(pdf.0, pdf.1, "unchanged file has same name in both columns");
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_pattern_rename_s_command() {
    let mut state = test_state();

    let pane = state.active_pane_mut();
    pane.entries = vec![FileEntryBuilder::new("Photo_001.JPG")
        .path("/test/Photo_001.JPG")
        .build()];
    update_state(&mut state, Transition::ShowPatternRenameDialog);

    // s/ command: case-insensitive global replace
    update_state(
        &mut state,
        Transition::UpdatePatternRenameFields {
            find: "s/photo/img/i".to_string(),
            replace: String::new(),
            use_regex: false,
            case_sensitive: true,
        },
    );

    let dialog = state.dialogs.current().unwrap();
    if let Some((_, _, _, _, preview)) = dialog.content.as_pattern_rename() {
        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0].1, "img_001.JPG");
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_no_marks_shows_all_pane_entries_in_preview() {
    // Without any marks, UpdatePatternRenameFields should use ALL pane entries (not just cursor)
    let mut state = test_state();

    let pane = state.active_pane_mut();
    pane.entries = vec![
        FileEntryBuilder::new("alpha.txt")
            .path("/test/alpha.txt")
            .build(),
        FileEntryBuilder::new("beta.txt")
            .path("/test/beta.txt")
            .build(),
        FileEntryBuilder::new("gamma.pdf")
            .path("/test/gamma.pdf")
            .build(),
    ];
    // No MarkAll — no marks at all
    update_state(&mut state, Transition::ShowPatternRenameDialog);
    update_state(
        &mut state,
        Transition::UpdatePatternRenameFields {
            find: r"\.txt$".to_string(),
            replace: ".bak".to_string(),
            use_regex: true,
            case_sensitive: true,
        },
    );

    let dialog = state.dialogs.current().unwrap();
    if let Some((_, _, _, _, preview)) = dialog.content.as_pattern_rename() {
        // All 3 files appear — not just cursor
        assert_eq!(
            preview.len(),
            3,
            "all pane entries must appear when no marks"
        );
        let alpha = preview.iter().find(|(o, _)| o == "alpha.txt").unwrap();
        assert_eq!(alpha.1, "alpha.bak");
        let beta = preview.iter().find(|(o, _)| o == "beta.txt").unwrap();
        assert_eq!(beta.1, "beta.bak");
        let gamma = preview.iter().find(|(o, _)| o == "gamma.pdf").unwrap();
        assert_eq!(gamma.0, gamma.1, "unchanged file unchanged in both columns");
    } else {
        panic!("Expected PatternRename dialog content");
    }
}

#[test]
fn test_pattern_rename_job_completion_refreshes_pane() {
    let mut state = test_state();

    let pane = state.active_pane_mut();
    let file_location = Location::Local(PathBuf::from("/test/file1.txt"));
    pane.entries = vec![FileEntryBuilder::new("file1.txt")
        .path("/test/file1.txt")
        .build()];

    let result = update_state(
        &mut state,
        Transition::ExecutePatternRename {
            find: r"\.txt$".to_string(),
            replace: ".bak".to_string(),
            use_regex: true,
            case_sensitive: true,
            targets: vec![file_location],
        },
    );

    let job_spec = result.jobs_to_start[0].clone();
    let job_id = job_spec.id;

    update_state(
        &mut state,
        Transition::EnqueueJob {
            spec: job_spec.clone(),
        },
    );
    update_state(&mut state, Transition::StartNextJob);
    state.jobs.start_job(job_spec);

    let result = update_state(
        &mut state,
        Transition::CompleteJob {
            job_id,
            result: OpResult::Success(SuccessData::None),
        },
    );

    assert_eq!(result.panes_to_refresh.len(), 1);
}
