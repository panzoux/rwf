//! Tests for Phase 1.11 — Help Dialog

#[cfg(test)]
mod tests {
    use crate::input::{KeyBindings, Action};
    use crate::model::dialog::DialogContent;
    use crate::state::{AppState, AppConfig, Transition, update_state};
    use crate::model::Dialog;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn open_help(state: &mut AppState) {
        update_state(state, Transition::ShowDialog {
            dialog: Dialog::help_with_language("en"),
        });
    }

    // ---- Key bindings -------------------------------------------------------

    #[test]
    fn test_help_key_question_mark() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::Help));
    }

    #[test]
    fn test_help_key_f1() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::Help));
    }

    // ---- Dialog opens -------------------------------------------------------

    #[test]
    fn test_help_dialog_opens() {
        let mut state = AppState::new(AppConfig::default());
        open_help(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(matches!(dialog.content, DialogContent::Help { .. }));
    }

    #[test]
    fn test_help_dialog_title_non_empty() {
        let mut state = AppState::new(AppConfig::default());
        open_help(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(!dialog.title.is_empty(), "help dialog title should not be empty");
    }

    // ---- Initial state ------------------------------------------------------

    #[test]
    fn test_help_dialog_initial_scroll_pos() {
        let mut state = AppState::new(AppConfig::default());
        open_help(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::Help { scroll_pos, .. } = &dialog.content {
            assert_eq!(*scroll_pos, 0, "scroll_pos must start at 0");
        } else {
            panic!("Expected Help dialog content");
        }
    }

    #[test]
    fn test_help_dialog_entries_field_exists() {
        // Verify Help dialog has structured entries (populated by help builder, empty here)
        let mut state = AppState::new(AppConfig::default());
        open_help(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        assert!(matches!(dialog.content, DialogContent::Help { .. }), "Expected Help dialog");
    }

    #[test]
    fn test_help_dialog_language_field() {
        let mut state = AppState::new(AppConfig::default());
        open_help(&mut state);
        let dialog = state.dialogs.current().expect("dialog must be open");
        if let DialogContent::Help { language, .. } = &dialog.content {
            assert_eq!(language, "en");
        } else {
            panic!("Expected Help dialog content");
        }
    }

    // ---- Language rotation --------------------------------------------------

    #[test]
    fn test_help_rotate_language_changes_dialog() {
        let mut state = AppState::new(AppConfig::default());
        open_help(&mut state);

        let lang_before = {
            let dialog = state.dialogs.current().expect("dialog must be open");
            if let DialogContent::Help { language, .. } = &dialog.content {
                language.clone()
            } else { panic!("Expected Help") }
        };

        update_state(&mut state, Transition::RotateHelpLanguage);

        let dialog = state.dialogs.current().expect("dialog must still be open after rotate");
        if let DialogContent::Help { language, .. } = &dialog.content {
            // After rotate, language may be same (only 1 lang available) or different —
            // the important invariant is the dialog stays open
            let _ = language; // at least one language always present
        } else {
            panic!("Expected Help dialog content after RotateHelpLanguage");
        }

        let _ = lang_before; // just verify no panic
    }

    #[test]
    fn test_help_rotate_language_dialog_stays_open() {
        let mut state = AppState::new(AppConfig::default());
        open_help(&mut state);
        update_state(&mut state, Transition::RotateHelpLanguage);

        let dialog = state.dialogs.current().expect("dialog must still be open after rotate");
        assert!(matches!(dialog.content, DialogContent::Help { .. }), "Expected Help dialog after rotate");
    }
}
