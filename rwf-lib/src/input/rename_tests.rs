//! Tests for Phase 1.4 — Simple Rename Dialog

#[cfg(test)]
mod tests {
    use crate::input::{action_to_transitions, Action, KeyBindings};
    use crate::state::Transition;
    use crate::test_utils::{test_state, FileEntryBuilder};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn make_entry(name: &str, is_dir: bool) -> crate::model::FileEntry {
        FileEntryBuilder::new(name).dir(is_dir).build()
    }

    // ---- Key binding --------------------------------------------------------

    #[test]
    fn test_rename_key_r() {
        let mut bindings = KeyBindings::default();
        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        let action = bindings.map_key(&key);
        assert_eq!(action, Some(Action::Rename));
    }

    // ---- Dialog opening ----------------------------------------------------

    #[test]
    fn test_rename_opens_simple_rename_dialog() {
        let mut state = test_state();
        state.active_pane_mut().entries = vec![make_entry("hello.txt", false)];

        let transitions = action_to_transitions(&state, &Action::Rename);
        assert!(
            transitions.iter().any(|t| matches!(
                t,
                Transition::ShowDialog { dialog }
                    if matches!(dialog.content, crate::model::dialog::DialogContent::SimpleRename { .. })
            )),
            "Rename action must open a SimpleRename dialog"
        );
    }

    #[test]
    fn test_rename_dialog_title() {
        let mut state = test_state();
        state.active_pane_mut().entries = vec![make_entry("hello.txt", false)];

        let transitions = action_to_transitions(&state, &Action::Rename);
        if let Some(Transition::ShowDialog { dialog }) = transitions.first() {
            assert_eq!(dialog.title, "Rename");
        } else {
            panic!("Expected ShowDialog transition");
        }
    }

    #[test]
    fn test_rename_dialog_prefilled_with_current_name() {
        let mut state = test_state();
        state.active_pane_mut().entries = vec![make_entry("hello.txt", false)];

        let transitions = action_to_transitions(&state, &Action::Rename);
        if let Some(Transition::ShowDialog { dialog }) = transitions.first() {
            if let crate::model::dialog::DialogContent::SimpleRename {
                input,
                focused_field,
                ..
            } = &dialog.content
            {
                assert_eq!(input, "hello.txt");
                assert_eq!(*focused_field, 0, "textbox should be focused by default");
            } else {
                panic!("Expected SimpleRename content");
            }
        } else {
            panic!("Expected ShowDialog transition");
        }
    }

    #[test]
    fn test_rename_cursor_at_end_of_prefilled_name() {
        let mut state = test_state();
        state.active_pane_mut().entries = vec![make_entry("hello.txt", false)];

        let transitions = action_to_transitions(&state, &Action::Rename);
        if let Some(Transition::ShowDialog { dialog }) = transitions.first() {
            if let crate::model::dialog::DialogContent::SimpleRename {
                input, cursor_pos, ..
            } = &dialog.content
            {
                assert_eq!(
                    *cursor_pos,
                    input.chars().count(),
                    "cursor should start at end of name"
                );
            } else {
                panic!("Expected SimpleRename content");
            }
        }
    }

    #[test]
    fn test_rename_no_dialog_when_pane_empty() {
        let state = test_state();
        let transitions = action_to_transitions(&state, &Action::Rename);
        assert!(
            transitions.is_empty(),
            "Rename on empty pane should produce no transitions"
        );
    }

    #[test]
    fn test_rename_dialog_works_for_directory() {
        let mut state = test_state();
        state.active_pane_mut().entries = vec![make_entry("src", true)];

        let transitions = action_to_transitions(&state, &Action::Rename);
        assert!(
            transitions.iter().any(|t| matches!(
                t,
                Transition::ShowDialog { dialog }
                    if matches!(&dialog.content, crate::model::dialog::DialogContent::SimpleRename { input, .. } if input == "src")
            )),
            "Rename should work for directories too"
        );
    }
}
