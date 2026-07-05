//! Tests for search and filter key handlers

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action};
    use crate::model::{ActivePane, DialogContent, UIMode};
    use crate::state::{update_state, AppConfig, AppState, Transition};

    #[test]
    fn test_start_search_action() {
        let config = AppConfig::default();
        let state = AppState::new(config);

        let transitions = action_to_transitions(&state, &Action::StartSearch);

        // Search is now inline: ChangeUIMode + ClearSearch (no dialog)
        assert_eq!(transitions.len(), 2);

        match &transitions[0] {
            Transition::ChangeUIMode { mode } => {
                assert_eq!(*mode, UIMode::Search);
            }
            _ => panic!("Expected ChangeUIMode transition"),
        }

        assert!(matches!(transitions[1], Transition::ClearSearch));
    }

    #[test]
    fn test_file_mask_filter_action() {
        let config = AppConfig::default();
        let state = AppState::new(config);

        let transitions = action_to_transitions(&state, &Action::FileMaskFilter);

        assert_eq!(transitions.len(), 1);

        match &transitions[0] {
            Transition::ShowDialog { dialog } => {
                assert_eq!(dialog.title, "File Mask Filter");
                match &dialog.content {
                    DialogContent::FileMask { input, .. } => {
                        assert_eq!(input, "");
                    }
                    _ => panic!("Expected FileMask dialog content"),
                }
            }
            _ => panic!("Expected ShowDialog transition"),
        }
    }

    #[test]
    fn test_file_mask_filter_with_existing_mask() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Set an existing file mask
        state.current_tab_mut().left_pane.file_mask = Some("*.txt".to_string());

        let transitions = action_to_transitions(&state, &Action::FileMaskFilter);

        match &transitions[0] {
            Transition::ShowDialog { dialog } => match &dialog.content {
                DialogContent::FileMask { input, .. } => {
                    assert_eq!(input, "*.txt");
                }
                _ => panic!("Expected FileMask dialog content"),
            },
            _ => panic!("Expected ShowDialog transition"),
        }
    }

    #[test]
    fn test_clear_search_filter_in_normal_mode() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Set a file mask
        state.current_tab_mut().left_pane.file_mask = Some("*.rs".to_string());

        let transitions = action_to_transitions(&state, &Action::ClearSearchFilter);

        // Should only clear file mask (not in search mode)
        assert_eq!(transitions.len(), 1);

        match &transitions[0] {
            Transition::SetFileMask { pane, mask } => {
                assert_eq!(*pane, ActivePane::Left);
                assert!(mask.is_none());
            }
            _ => panic!("Expected SetFileMask transition"),
        }
    }

    #[test]
    fn test_clear_search_filter_in_search_mode() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Enter search mode
        state.ui.mode = UIMode::Search;

        let transitions = action_to_transitions(&state, &Action::ClearSearchFilter);

        // Should clear search and exit search mode
        assert_eq!(transitions.len(), 2);

        match &transitions[0] {
            Transition::ClearSearch => {}
            _ => panic!("Expected ClearSearch transition"),
        }

        match &transitions[1] {
            Transition::ChangeUIMode { mode } => {
                assert_eq!(*mode, UIMode::Normal);
            }
            _ => panic!("Expected ChangeUIMode transition"),
        }
    }

    #[test]
    fn test_clear_search_filter_both() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Enter search mode and set file mask
        state.ui.mode = UIMode::Search;
        state.current_tab_mut().left_pane.file_mask = Some("*.txt".to_string());

        let transitions = action_to_transitions(&state, &Action::ClearSearchFilter);

        // Should clear both search and file mask
        assert_eq!(transitions.len(), 3);

        match &transitions[0] {
            Transition::ClearSearch => {}
            _ => panic!("Expected ClearSearch transition"),
        }

        match &transitions[1] {
            Transition::ChangeUIMode { mode } => {
                assert_eq!(*mode, UIMode::Normal);
            }
            _ => panic!("Expected ChangeUIMode transition"),
        }

        match &transitions[2] {
            Transition::SetFileMask { pane, mask } => {
                assert_eq!(*pane, ActivePane::Left);
                assert!(mask.is_none());
            }
            _ => panic!("Expected SetFileMask transition"),
        }
    }

    #[test]
    fn test_exit_search_mode_when_in_search() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Enter search mode
        state.ui.mode = UIMode::Search;

        let transitions = action_to_transitions(&state, &Action::ExitSearchMode);

        // Should clear search, exit search mode, and close dialog
        assert_eq!(transitions.len(), 3);

        match &transitions[0] {
            Transition::ClearSearch => {}
            _ => panic!("Expected ClearSearch transition"),
        }

        match &transitions[1] {
            Transition::ChangeUIMode { mode } => {
                assert_eq!(*mode, UIMode::Normal);
            }
            _ => panic!("Expected ChangeUIMode transition"),
        }

        match &transitions[2] {
            Transition::CloseDialog => {}
            _ => panic!("Expected CloseDialog transition"),
        }
    }

    #[test]
    fn test_exit_search_mode_when_not_in_search() {
        let config = AppConfig::default();
        let state = AppState::new(config);

        // Not in search mode
        assert_eq!(state.ui.mode, UIMode::Normal);

        let transitions = action_to_transitions(&state, &Action::ExitSearchMode);

        // Should do nothing
        assert_eq!(transitions.len(), 0);
    }

    #[test]
    fn test_next_match_in_search_mode() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Enter search mode
        state.ui.mode = UIMode::Search;

        let transitions = action_to_transitions(&state, &Action::NextMatch);

        assert_eq!(transitions.len(), 1);

        match &transitions[0] {
            Transition::NextSearchResult => {}
            _ => panic!("Expected NextSearchResult transition"),
        }
    }

    #[test]
    fn test_next_match_not_in_search_mode() {
        let config = AppConfig::default();
        let state = AppState::new(config);

        let transitions = action_to_transitions(&state, &Action::NextMatch);

        // Should do nothing when not in search mode
        assert_eq!(transitions.len(), 0);
    }

    #[test]
    fn test_prev_match_in_search_mode() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Enter search mode
        state.ui.mode = UIMode::Search;

        let transitions = action_to_transitions(&state, &Action::PrevMatch);

        assert_eq!(transitions.len(), 1);

        match &transitions[0] {
            Transition::PrevSearchResult => {}
            _ => panic!("Expected PrevSearchResult transition"),
        }
    }

    #[test]
    fn test_prev_match_not_in_search_mode() {
        let config = AppConfig::default();
        let state = AppState::new(config);

        let transitions = action_to_transitions(&state, &Action::PrevMatch);

        // Should do nothing when not in search mode
        assert_eq!(transitions.len(), 0);
    }

    #[test]
    fn test_search_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Start search
        let transitions = action_to_transitions(&state, &Action::StartSearch);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify we're in search mode (inline search — no dialog)
        assert_eq!(state.ui.mode, UIMode::Search);
        assert!(state.dialogs.is_empty());

        // Exit search mode
        let transitions = action_to_transitions(&state, &Action::ExitSearchMode);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify we're back in normal mode
        assert_eq!(state.ui.mode, UIMode::Normal);
    }

    #[test]
    fn test_filter_workflow() {
        let config = AppConfig::default();
        let mut state = AppState::new(config);

        // Show filter dialog
        let transitions = action_to_transitions(&state, &Action::FileMaskFilter);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify dialog is shown
        assert!(!state.dialogs.is_empty());

        // Simulate setting a file mask
        update_state(
            &mut state,
            Transition::SetFileMask {
                pane: ActivePane::Left,
                mask: Some("*.rs".to_string()),
            },
        );

        // Verify file mask is set
        assert_eq!(
            state.current_tab().left_pane.file_mask,
            Some("*.rs".to_string())
        );

        // Clear filter
        let transitions = action_to_transitions(&state, &Action::ClearSearchFilter);
        for transition in transitions {
            update_state(&mut state, transition);
        }

        // Verify file mask is cleared
        assert!(state.current_tab().left_pane.file_mask.is_none());
    }

    #[test]
    fn test_key_bindings_for_search_filter() {
        use crate::input::KeyBindings;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut bindings = KeyBindings::default();

        // Test / key for search
        let event = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert!(
            matches!(action, Some(Action::StartSearch)),
            "Expected StartSearch for '/', got {:?}",
            action
        );

        // Test Ctrl+F for search
        let event = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert!(
            matches!(action, Some(Action::StartSearch)),
            "Expected StartSearch for Ctrl+F, got {:?}",
            action
        );

        // Test f key for filter
        let event = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert!(
            matches!(action, Some(Action::FileMaskFilter)),
            "Expected FileMaskFilter for 'f', got {:?}",
            action
        );

        // Test Ctrl+K for clear
        let event = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert!(
            matches!(action, Some(Action::ClearSearchFilter)),
            "Expected ClearSearchFilter for Ctrl+K, got {:?}",
            action
        );

        // Test Escape for exit search (now maps to Quit, which exits search mode when in search mode)
        let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let action = bindings.map_key(&event);
        assert!(
            matches!(action, Some(Action::Quit)),
            "Expected Quit for Escape, got {:?}",
            action
        );
    }
}
