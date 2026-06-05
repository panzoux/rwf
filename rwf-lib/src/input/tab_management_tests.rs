//! Tests for tab management key handlers

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action, KeyBindings};
    use crate::model::DialogContent;
    use crate::state::{update_state, AppConfig, Transition};
    use crate::AppState;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn create_test_state() -> AppState {
        let config = AppConfig::default();
        AppState::new(config)
    }

    fn create_tab(state: &mut AppState) {
        state.last_tab_created = None;
        update_state(state, Transition::CreateTab);
    }

    #[test]
    fn test_new_tab_key_binding() {
        let mut bindings = KeyBindings::default();
        
        // Ctrl+N should create new tab
        let event = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::NewTab));
    }

    #[test]
    fn test_close_tab_key_binding() {
        let mut bindings = KeyBindings::default();
        
        // Ctrl+W should close tab
        let event = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::CloseTab));
    }

    #[test]
    fn test_next_tab_key_bindings() {
        let mut bindings = KeyBindings::default();
        
        // Ctrl+Right should switch to next tab
        let event = KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::NextTab));
        
        // Ctrl+PageDown should also switch to next tab
        let event = KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::NextTab));
    }

    #[test]
    fn test_prev_tab_key_bindings() {
        let mut bindings = KeyBindings::default();
        
        // Ctrl+Left should switch to previous tab
        let event = KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::PrevTab));
        
        // Ctrl+PageUp should also switch to previous tab
        let event = KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::PrevTab));
    }

    #[test]
    fn test_tab_selector_key_bindings() {
        let mut bindings = KeyBindings::default();
        
        // Ctrl+T should open tab selector
        let event = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::TabSelector));
        
        // Ctrl+B should also open tab selector
        let event = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL);
        let action = bindings.map_key(&event);
        assert_eq!(action, Some(Action::TabSelector));
    }

    #[test]
    fn test_new_tab_action_creates_transition() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::NewTab);
        
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::CreateTab));
    }

    #[test]
    fn test_close_tab_action_creates_transition() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::CloseTab);
        
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::CloseTab { index: 0 }));
    }

    #[test]
    fn test_next_tab_action_creates_transition() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::NextTab);
        
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::NextTab));
    }

    #[test]
    fn test_prev_tab_action_creates_transition() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::PrevTab);
        
        assert_eq!(transitions.len(), 1);
        assert!(matches!(transitions[0], Transition::PrevTab));
    }

    #[test]
    fn test_tab_selector_action_shows_dialog() {
        let state = create_test_state();
        let transitions = action_to_transitions(&state, &Action::TabSelector);
        
        assert_eq!(transitions.len(), 1);
        
        if let Transition::ShowDialog { dialog } = &transitions[0] {
            assert_eq!(dialog.title, "Select Tab");
            assert!(matches!(dialog.content, DialogContent::TabSelector { .. }));
            
            if let DialogContent::TabSelector { tabs, .. } = &dialog.content {
                // Should have at least one tab (the initial tab)
                assert!(!tabs.is_empty());
            }
        } else {
            panic!("Expected ShowDialog transition");
        }
    }

    #[test]
    fn test_tab_selector_shows_multiple_tabs() {
        let mut state = create_test_state();
        
        // Create additional tabs
        create_tab(&mut state);
        create_tab(&mut state);
        
        let transitions = action_to_transitions(&state, &Action::TabSelector);
        
        if let Transition::ShowDialog { dialog } = &transitions[0] {
            if let DialogContent::TabSelector { tabs, .. } = &dialog.content {
                // Should have 3 tabs now
                assert_eq!(tabs.len(), 3);
                
                // Each tab should have a formatted name
                for (i, tab_name) in tabs.iter().enumerate() {
                    assert!(tab_name.contains(&format!("Tab {}", i + 1)));
                }
            }
        }
    }

    #[test]
    fn test_tab_management_workflow() {
        let mut state = create_test_state();
        
        // Initially should have 1 tab
        assert_eq!(state.tabs.tabs.len(), 1);
        assert_eq!(state.tabs.active_index, 0);
        
        // Create new tab (Ctrl+N)
        let transitions = action_to_transitions(&state, &Action::NewTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }
        
        assert_eq!(state.tabs.tabs.len(), 2);
        assert_eq!(state.tabs.active_index, 1);
        
        // Switch to next tab (Ctrl+Right)
        let transitions = action_to_transitions(&state, &Action::NextTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }
        
        // Should wrap around to first tab
        assert_eq!(state.tabs.active_index, 0);
        
        // Switch to previous tab (Ctrl+Left)
        let transitions = action_to_transitions(&state, &Action::PrevTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }
        
        // Should wrap around to last tab
        assert_eq!(state.tabs.active_index, 1);
        
        // Close current tab (Ctrl+W)
        let transitions = action_to_transitions(&state, &Action::CloseTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }
        
        assert_eq!(state.tabs.tabs.len(), 1);
        assert_eq!(state.tabs.active_index, 0);
    }

    #[test]
    fn test_cannot_close_last_tab() {
        let mut state = create_test_state();
        
        // Should have 1 tab
        assert_eq!(state.tabs.tabs.len(), 1);
        
        // Try to close the last tab
        let transitions = action_to_transitions(&state, &Action::CloseTab);
        for transition in transitions {
            update_state(&mut state, transition);
        }
        
        // Should still have 1 tab (cannot close last tab)
        assert_eq!(state.tabs.tabs.len(), 1);
    }

    #[test]
    fn test_tab_creation_initializes_with_cwd() {
        let mut state = create_test_state();
        
        // Get the current working directory
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
        
        // Create a new tab
        create_tab(&mut state);
        
        // New tab should be initialized with CWD
        let new_tab = &state.tabs.tabs[1];
        assert_eq!(new_tab.left_pane.current_location, crate::model::Location::Local(cwd.clone()));
        assert_eq!(new_tab.right_pane.current_location, crate::model::Location::Local(cwd));
    }

    #[test]
    fn test_tab_closure_adjusts_active_index() {
        let mut state = create_test_state();
        
        // Create 3 tabs
        create_tab(&mut state);
        create_tab(&mut state);
        
        // Should be on tab 2 (index 2)
        assert_eq!(state.tabs.active_index, 2);
        
        // Close the active tab
        update_state(&mut state, Transition::CloseTab { index: 2 });
        
        // Active index should be adjusted to last tab
        assert_eq!(state.tabs.active_index, 1);
        assert_eq!(state.tabs.tabs.len(), 2);
    }

    #[test]
    fn test_tab_switching_wraps_around() {
        let mut state = create_test_state();
        
        // Create 3 tabs
        create_tab(&mut state);
        create_tab(&mut state);
        
        // Go to first tab
        update_state(&mut state, Transition::SwitchTab { index: 0 });
        assert_eq!(state.tabs.active_index, 0);
        
        // Next tab should go to tab 1
        update_state(&mut state, Transition::NextTab);
        assert_eq!(state.tabs.active_index, 1);
        
        // Next tab should go to tab 2
        update_state(&mut state, Transition::NextTab);
        assert_eq!(state.tabs.active_index, 2);
        
        // Next tab should wrap around to tab 0
        update_state(&mut state, Transition::NextTab);
        assert_eq!(state.tabs.active_index, 0);
        
        // Previous tab should wrap around to tab 2
        update_state(&mut state, Transition::PrevTab);
        assert_eq!(state.tabs.active_index, 2);
    }

    #[test]
    fn test_tab_persistence_saves_and_restores() {
        use std::path::PathBuf;
        
        let mut state = create_test_state();
        
        // Create multiple tabs
        create_tab(&mut state);
        create_tab(&mut state);
        
        // Switch to middle tab
        update_state(&mut state, Transition::SwitchTab { index: 1 });
        
        // Mark some files
        let loc1 = crate::model::Location::Local(PathBuf::from("/test1"));
        let loc2 = crate::model::Location::Local(PathBuf::from("/test2"));
        state.current_tab_mut().left_pane.marking.mark(loc1.clone());
        state.current_tab_mut().left_pane.marking.mark(loc2.clone());
        
        // Save session
        let session_path = std::env::temp_dir().join("test_tab_persistence.json");
        let session = crate::session::save_session(
            &state.tabs.tabs,
            state.tabs.active_index,
            state.ui.active_pane,
            &std::collections::HashSet::new(),
            state.ui.layout.show_task_panel,
            state.ui.layout.task_panel_height,
        );
        session.save_to_file(&session_path).unwrap();
        
        // Create new state and restore
        let mut state2 = create_test_state();
        let loaded_session = crate::session::SessionState::load_from_file(&session_path).unwrap();
        state2.tabs.tabs = crate::session::restore_tabs(&loaded_session);
        state2.tabs.active_index = loaded_session.active_tab_index;
        // Marks are per-pane and not persisted across sessions

        // Verify restoration
        assert_eq!(state2.tabs.tabs.len(), 3);
        assert_eq!(state2.tabs.active_index, 1);
        assert_eq!(state2.current_tab_mut().left_pane.marking.count(), 0);
        
        // Cleanup
        let _ = std::fs::remove_file(session_path);
    }

    #[test]
    fn test_tab_selector_dialog_filtering() {
        let mut state = create_test_state();
        
        // Create multiple tabs with different locations
        create_tab(&mut state);
        create_tab(&mut state);
        
        // Show tab selector
        let transitions = action_to_transitions(&state, &Action::TabSelector);
        
        if let Transition::ShowDialog { dialog } = &transitions[0] {
            if let DialogContent::TabSelector { tabs, selected_index } = &dialog.content {
                // Should have 3 tabs
                assert_eq!(tabs.len(), 3);
                assert_eq!(*selected_index, 0);
                
                // Each tab should have a formatted name with locations
                for tab_name in tabs {
                    assert!(tab_name.contains("Tab"));
                    assert!(tab_name.contains("|")); // Should show left | right panes
                }
            } else {
                panic!("Expected TabSelector dialog content");
            }
        } else {
            panic!("Expected ShowDialog transition");
        }
    }

    #[test]
    fn test_tab_independence() {
        let mut state = create_test_state();
        
        // Create second tab
        create_tab(&mut state);
        
        // Go to first tab
        update_state(&mut state, Transition::SwitchTab { index: 0 });
        
        // Move cursor in first tab
        // Add some dummy entries to the pane
        state.current_tab_mut().left_pane.entries = vec![
            crate::model::FileEntry {
                name: "file1.txt".to_string(),
                location: crate::model::Location::Local(std::path::PathBuf::from("/file1.txt")),
                size: 100,
                is_dir: false,
                is_hidden: false,
                modified: std::time::SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
            crate::model::FileEntry {
                name: "file2.txt".to_string(),
                location: crate::model::Location::Local(std::path::PathBuf::from("/file2.txt")),
                size: 200,
                is_dir: false,
                is_hidden: false,
                modified: std::time::SystemTime::now(),
                marked: false,
                calculated_size: None,
            },
        ];
        
        // Move cursor down in first tab
        update_state(&mut state, Transition::CursorMove {
            pane: crate::model::ActivePane::Left,
            delta: 1,
        });
        
        let first_tab_cursor = state.current_tab().left_pane.cursor;
        assert_eq!(first_tab_cursor, 1);
        
        // Switch to second tab
        update_state(&mut state, Transition::NextTab);
        
        // Second tab cursor should be at 0 (independent)
        let second_tab_cursor = state.current_tab().left_pane.cursor;
        assert_eq!(second_tab_cursor, 0);
        
        // Switch back to first tab
        update_state(&mut state, Transition::PrevTab);
        
        // First tab cursor should still be at 1
        let first_tab_cursor_again = state.current_tab().left_pane.cursor;
        assert_eq!(first_tab_cursor_again, 1);
    }

    #[test]
    fn test_close_middle_tab() {
        let mut state = create_test_state();
        
        // Create 3 tabs
        create_tab(&mut state);
        create_tab(&mut state);
        
        // Go to middle tab (index 1)
        update_state(&mut state, Transition::SwitchTab { index: 1 });
        
        // Close middle tab
        update_state(&mut state, Transition::CloseTab { index: 1 });
        
        // Should have 2 tabs left
        assert_eq!(state.tabs.tabs.len(), 2);
        
        // Active index should be adjusted (should be 1, which is the old tab 2)
        assert_eq!(state.tabs.active_index, 1);
    }

    #[test]
    fn test_tab_selector_with_single_tab() {
        let state = create_test_state();
        
        // Show tab selector with only one tab
        let transitions = action_to_transitions(&state, &Action::TabSelector);
        
        if let Transition::ShowDialog { dialog } = &transitions[0] {
            if let DialogContent::TabSelector { tabs, .. } = &dialog.content {
                // Should have 1 tab
                assert_eq!(tabs.len(), 1);
            }
        }
    }

    #[test]
    fn test_multiple_tab_operations_sequence() {
        let mut state = create_test_state();
        
        // Create 4 tabs
        for _ in 0..4 {
            create_tab(&mut state);
        }
        assert_eq!(state.tabs.tabs.len(), 5);
        
        // Close tab 2
        update_state(&mut state, Transition::CloseTab { index: 2 });
        assert_eq!(state.tabs.tabs.len(), 4);
        
        // Close tab 0
        update_state(&mut state, Transition::CloseTab { index: 0 });
        assert_eq!(state.tabs.tabs.len(), 3);
        
        // Create a new tab
        create_tab(&mut state);
        assert_eq!(state.tabs.tabs.len(), 4);
        
        // Close all but one
        while state.tabs.tabs.len() > 1 {
            update_state(&mut state, Transition::CloseTab { index: 0 });
        }
        
        // Should have exactly 1 tab (cannot close last tab)
        assert_eq!(state.tabs.tabs.len(), 1);
    }
}
