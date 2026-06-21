//! Tests for miscellaneous key handlers (quit, help, job manager)

use super::*;
use crate::model::{Dialog, DialogContent};
use crate::state::{AppConfig, AppState, Transition};

#[test]
fn test_quit_action_q_key() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    
    let transitions = action_to_transitions(&state, &Action::Quit);
    
    assert_eq!(transitions.len(), 1);
    assert!(matches!(transitions[0], Transition::Quit));
}

#[test]
fn test_help_action() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    
    let transitions = action_to_transitions(&state, &Action::Help);
    
    assert_eq!(transitions.len(), 1);
    match &transitions[0] {
        Transition::ShowDialog { dialog } => {
            assert!(!dialog.title.is_empty(), "help dialog title must not be empty");
            assert!(matches!(&dialog.content, DialogContent::Help { .. }), "Expected Help dialog content");
        }
        _ => panic!("Expected ShowDialog transition"),
    }
}

#[test]
fn test_job_manager_action() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    
    let transitions = action_to_transitions(&state, &Action::JobManager);
    
    assert_eq!(transitions.len(), 1);
    match &transitions[0] {
        Transition::ShowDialog { dialog } => {
            assert_eq!(dialog.title, "Job Manager");
            match &dialog.content {
                DialogContent::JobManager { selected_index, .. } => {
                    assert_eq!(*selected_index, 0);
                }
                _ => panic!("Expected JobManager dialog content"),
            }
        }
        _ => panic!("Expected ShowDialog transition"),
    }
}

#[test]
fn test_key_bindings_quit_q() {
    let mut bindings = KeyBindings::default();

    // Lowercase 'q' (no modifier) maps to Quit
    let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let action = bindings.map_key(&event);
    assert_eq!(action, Some(Action::Quit));

    // Uppercase 'Q' (Shift+q) maps to ExitAndChangeDirectory
    let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::SHIFT);
    let action = bindings.map_key(&event);
    assert_eq!(action, Some(Action::ExitAndChangeDirectory));
}

#[test]
fn test_key_bindings_quit_escape() {
    let mut bindings = KeyBindings::default();
    
    let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let action = bindings.map_key(&event);
    
    assert_eq!(action, Some(Action::Quit));
}

#[test]
fn test_key_bindings_help_question_mark() {
    let mut bindings = KeyBindings::default();

    // '?' character maps to Help (terminal sends '?' directly, not '/' with SHIFT)
    let event = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    let action = bindings.map_key(&event);

    assert_eq!(action, Some(Action::Help));
}

#[test]
fn test_key_bindings_help_f1() {
    let mut bindings = KeyBindings::default();
    
    let event = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    let action = bindings.map_key(&event);
    
    assert_eq!(action, Some(Action::Help));
}

#[test]
fn test_key_bindings_job_manager_ctrl_j() {
    let mut bindings = KeyBindings::default();
    
    let event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
    let action = bindings.map_key(&event);
    
    assert_eq!(action, Some(Action::JobManager));
}

#[test]
fn test_help_dialog_creation() {
    let dialog = Dialog::help();

    assert!(!dialog.title.is_empty(), "help dialog title must not be empty");
    assert!(matches!(dialog.content, DialogContent::Help { .. }), "Expected Help dialog content");
}

#[test]
fn test_job_manager_dialog_creation() {
    let dialog = Dialog::job_manager();
    
    assert_eq!(dialog.title, "Job Manager");
    match dialog.content {
        DialogContent::JobManager { selected_index, .. } => {
            assert_eq!(selected_index, 0);
        }
        _ => panic!("Expected JobManager dialog content"),
    }
}

#[test]
fn test_help_content_completeness() {
    // Content completeness is verified by Step 8 tests (help_viewer_tests.rs) after the
    // help builder (Step 5) populates entries. For now just verify the dialog opens.
    let dialog = Dialog::help();
    assert!(matches!(dialog.content, DialogContent::Help { .. }), "Expected Help dialog content");
}

#[test]
fn test_quit_action_exits_search_mode_when_in_search() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    
    // Enter search mode
    state.ui.mode = crate::model::UIMode::Search;
    
    let transitions = action_to_transitions(&state, &Action::Quit);
    
    // Should exit search mode, not quit
    assert_eq!(transitions.len(), 3);
    assert!(matches!(transitions[0], Transition::ClearSearch));
    assert!(matches!(transitions[1], Transition::ChangeUIMode { .. }));
    assert!(matches!(transitions[2], Transition::CloseDialog));
}

#[test]
fn test_quit_action_quits_when_in_normal_mode() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    
    // Should be in normal mode by default
    assert_eq!(state.ui.mode, crate::model::UIMode::Normal);

    let transitions = action_to_transitions(&state, &Action::Quit);

    // Should quit
    assert_eq!(transitions.len(), 1);
    assert!(matches!(transitions[0], Transition::Quit));
}

// ── Duplicate key detection tests ────────────────────────────────────────────

#[test]
fn test_no_duplicates_clean_content() {
    let json = r#"{"NormalMode": {"Up": "CursorUp", "Down": "CursorDown"}}"#;
    let warnings = check_keybindings_content_duplicates(json);
    assert!(warnings.is_empty());
}

#[test]
fn test_detects_duplicate_in_normal_mode() {
    let json = r#"{"NormalMode": {"Up": "CursorUp", "Down": "CursorDown", "Up": "CursorDown"}}"#;
    let warnings = check_keybindings_content_duplicates(json);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("NormalMode"));
    assert!(warnings[0].contains("'Up'"));
    assert!(warnings[0].contains("'CursorUp'"), "should show overridden action");
    assert!(warnings[0].contains("'CursorDown'"), "should show winning action");
}

#[test]
fn test_detects_duplicates_in_multiple_modes() {
    let json = r#"{
        "NormalMode": {"j": "CursorDown", "j": "CursorUp"},
        "ViewerMode": {"q": "ViewerClose", "q": "ViewerScrollDown"}
    }"#;
    let warnings = check_keybindings_content_duplicates(json);
    assert_eq!(warnings.len(), 2);
    let nw = warnings.iter().find(|w| w.contains("NormalMode")).unwrap();
    assert!(nw.contains("'j'") && nw.contains("'CursorDown'") && nw.contains("'CursorUp'"));
    let vw = warnings.iter().find(|w| w.contains("ViewerMode")).unwrap();
    assert!(vw.contains("'q'") && vw.contains("'ViewerClose'") && vw.contains("'ViewerScrollDown'"));
}

#[test]
fn test_invalid_json_returns_empty() {
    let warnings = check_keybindings_content_duplicates("not json at all {{{");
    assert!(warnings.is_empty());
}
