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
            assert_eq!(dialog.title, "Help - Key Bindings");
            match &dialog.content {
                DialogContent::Help { content, .. } => {
                    // Verify help content contains key sections
                    assert!(content.contains("Navigation:"));
                    assert!(content.contains("File Operations:"));
                    assert!(content.contains("Marking:"));
                    assert!(content.contains("Sorting:"));
                    assert!(content.contains("Search & Filter:"));
                    assert!(content.contains("Tab Management:"));
                    assert!(content.contains("Miscellaneous:"));
                    
                    // Verify some specific key bindings
                    assert!(content.contains("Tab"));
                    assert!(content.contains("Switch pane"));
                    assert!(content.contains("C"));
                    assert!(content.contains("Copy"));
                    assert!(content.contains("Q, Escape"));
                    assert!(content.contains("Quit application"));
                    assert!(content.contains("?, F1"));
                    assert!(content.contains("Show this help"));
                    assert!(content.contains("Ctrl+J"));
                    assert!(content.contains("Job manager"));
                }
                _ => panic!("Expected Help dialog content"),
            }
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
    
    assert_eq!(dialog.title, "Help - Key Bindings");
    match dialog.content {
        DialogContent::Help { content, .. } => {
            assert!(!content.is_empty());
            assert!(content.contains("Navigation:"));
            assert!(content.contains("Miscellaneous:"));
        }
        _ => panic!("Expected Help dialog content"),
    }
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
    let dialog = Dialog::help();
    
    match dialog.content {
        DialogContent::Help { content, .. } => {
            // Verify all major sections are present
            let sections = vec![
                "Navigation:",
                "File Operations:",
                "Marking:",
                "Sorting:",
                "Search & Filter:",
                "Tab Management:",
                "Miscellaneous:",
            ];
            
            for section in sections {
                assert!(
                    content.contains(section),
                    "Help content missing section: {}",
                    section
                );
            }
            
            // Verify key bindings mentioned in requirements
            let required_bindings = vec![
                ("Q, Escape", "Quit"),
                ("?, F1", "help"),
                ("Ctrl+J", "Job manager"),
            ];
            
            for (keys, description) in required_bindings {
                assert!(
                    content.contains(keys),
                    "Help content missing key binding: {}",
                    keys
                );
                assert!(
                    content.to_lowercase().contains(&description.to_lowercase()),
                    "Help content missing description: {}",
                    description
                );
            }
        }
        _ => panic!("Expected Help dialog content"),
    }
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
