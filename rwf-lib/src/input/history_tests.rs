//! Tests for Phase 1.6 — History Dialog

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action, KeyBindings};
    use crate::model::Location;
    use crate::state::{update_state, Transition};
    use crate::test_utils::test_state;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn loc(path: &str) -> Location {
        Location::Local(PathBuf::from(path))
    }

    // ---- Key binding -------------------------------------------------------

    #[test]
    fn test_history_dialog_key_shift_h() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('H'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::ShowHistoryDialog));
    }

    #[test]
    fn test_history_dialog_key_lowercase_h_with_shift_modifier() {
        // Windows crossterm may send lowercase + SHIFT instead of uppercase + NONE
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::SHIFT);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::ShowHistoryDialog));
    }

    #[test]
    fn test_h_key_switches_to_left_pane() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::SwitchToLeftPane));
    }

    // ---- Empty history: dialog still opens (current location shown) --------

    #[test]
    fn test_show_history_dialog_opens_even_with_empty_history() {
        let state = test_state();
        let transitions = action_to_transitions(&state, &Action::ShowHistoryDialog);
        assert!(
            transitions
                .iter()
                .any(|t| matches!(t, Transition::ShowDialog { .. })),
            "Dialog should open even when navigation history is empty"
        );
    }

    // ---- History with entries: dialog opens --------------------------------

    #[test]
    fn test_show_history_dialog_opens_when_history_exists() {
        let mut state = test_state();
        // Seed history by navigating
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/home/user/docs"),
            },
        );
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/home/user/photos"),
            },
        );

        let transitions = action_to_transitions(&state, &Action::ShowHistoryDialog);
        assert!(
            transitions.iter().any(|t| matches!(
                t,
                Transition::ShowDialog { dialog }
                    if matches!(dialog.content, crate::model::dialog::DialogContent::HistoryDialog(_))
            )),
            "ShowHistoryDialog must open a HistoryDialog dialog"
        );
    }

    #[test]
    fn test_history_dialog_title_includes_tab_and_pane() {
        let mut state = test_state();
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/tmp"),
            },
        );

        let transitions = action_to_transitions(&state, &Action::ShowHistoryDialog);
        if let Some(Transition::ShowDialog { dialog }) = transitions.first() {
            assert!(
                dialog.title.contains("Tab 1"),
                "title must include tab number"
            );
            assert!(
                dialog.title.contains("Left"),
                "title must include pane name"
            );
        } else {
            panic!("Expected ShowDialog transition");
        }
    }

    #[test]
    fn test_history_dialog_contains_visited_locations() {
        let mut state = test_state();
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/home/user/docs"),
            },
        );
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/home/user/photos"),
            },
        );

        let transitions = action_to_transitions(&state, &Action::ShowHistoryDialog);
        if let Some(Transition::ShowDialog { dialog }) = transitions.first() {
            if let crate::model::dialog::DialogContent::HistoryDialog(
                crate::model::dialog::HistoryDialogContent { left_entries, .. },
            ) = &dialog.content
            {
                let paths: Vec<String> = left_entries.iter().map(|l| l.display_path()).collect();
                assert!(
                    paths.iter().any(|p| p.contains("docs")),
                    "history must include /docs"
                );
                assert!(
                    paths.iter().any(|p| p.contains("photos")),
                    "history must include /photos"
                );
            } else {
                panic!("Expected HistoryDialog content");
            }
        }
    }

    #[test]
    fn test_history_dialog_selected_index_at_current_pos() {
        let mut state = test_state();
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/a"),
            },
        );
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/b"),
            },
        );
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/c"),
            },
        );

        let transitions = action_to_transitions(&state, &Action::ShowHistoryDialog);
        if let Some(Transition::ShowDialog { dialog }) = transitions.first() {
            if let crate::model::dialog::DialogContent::HistoryDialog(
                crate::model::dialog::HistoryDialogContent {
                    left_selected,
                    left_current_pos,
                    left_entries,
                    ..
                },
            ) = &dialog.content
            {
                assert_eq!(
                    left_selected, left_current_pos,
                    "cursor must start at current history position"
                );
                assert_eq!(
                    *left_current_pos,
                    left_entries.len() - 1,
                    "current_pos should be at newest entry"
                );
            } else {
                panic!("Expected HistoryDialog content");
            }
        }
    }

    // ---- NavigateToHistoryIndex transition ---------------------------------

    #[test]
    fn test_navigate_to_history_index_changes_location() {
        let mut state = test_state();
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/step1"),
            },
        );
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/step2"),
            },
        );
        update_state(
            &mut state,
            Transition::ChangeLocation {
                pane: crate::model::ui::ActivePane::Left,
                location: loc("/step3"),
            },
        );

        // Jump back to index 0 (first entry — whatever the initial empty location was)
        // Actually jump to index 1 (/step1)
        let tab = state.current_tab();
        let (stack, _) = tab
            .history
            .stack_and_pos(crate::model::ui::ActivePane::Left);
        let target_location = stack[1].clone();
        let _ = tab;

        update_state(
            &mut state,
            Transition::NavigateToHistoryIndex {
                pane: crate::model::ui::ActivePane::Left,
                index: 1,
            },
        );

        assert_eq!(
            state.current_tab().left_pane.current_location,
            target_location,
            "pane location must match the history entry at the given index"
        );
        assert_eq!(
            state.current_tab().history.left_pos,
            1,
            "history position must be updated to the jumped index"
        );
    }

    // ---- navigation_history: stack_and_pos / jump_to_index ----------------

    #[test]
    fn test_jump_to_index_out_of_bounds_returns_none() {
        let mut history = crate::model::navigation::NavigationHistory::new();
        history.push(crate::model::ui::ActivePane::Left, loc("/a"));
        assert!(history
            .jump_to_index(crate::model::ui::ActivePane::Left, 99)
            .is_none());
    }

    #[test]
    fn test_jump_to_index_valid() {
        let mut history = crate::model::navigation::NavigationHistory::new();
        history.push(crate::model::ui::ActivePane::Left, loc("/a"));
        history.push(crate::model::ui::ActivePane::Left, loc("/b"));
        history.push(crate::model::ui::ActivePane::Left, loc("/c"));

        let result = history.jump_to_index(crate::model::ui::ActivePane::Left, 1);
        assert_eq!(result, Some(loc("/b")));
        assert_eq!(history.left_pos, 1);
    }
}
